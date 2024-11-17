use anyhow::{anyhow, Result};
use futures::StreamExt;
use iroh::docs::{Author, AuthorId};
use iroh::net::key::PublicKey;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::router::RouterClient;

use super::events::{Event, EventKind, EventObject, HashLink, Tag, NOSTR_ID_TAG};
use super::Repo;

#[derive(Debug, Serialize, Deserialize)]
pub struct Profile {
    name: String,
    description: String,
    picture: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub created_at: i64,
    pub pubkey: PublicKey,
    pub content: HashLink,
    pub blankame: String,
    pub author: Option<Author>,
    pub profile: Option<Profile>,
}

impl EventObject for User {
    async fn from_event(event: Event, router: &RouterClient) -> Result<Self> {
        if event.kind != EventKind::MutateUser {
            return Err(anyhow!("event is not a user mutation"));
        }

        // normalize tags
        let id = event.data_id()?.ok_or_else(|| anyhow!("missing data id"))?;

        // fetch content if necessary
        let mut content = event.content.clone();
        let profile = match content.resolve(router).await {
            Ok(content) => {
                let profile: Profile = serde_json::from_value(content)?;
                Some(profile)
            }
            Err(_) => None,
        };

        let author = AuthorId::from(event.pubkey.as_bytes());
        let author = match router.authors().export(author).await {
            Ok(author) => author,
            Err(_) => None,
        };

        Ok(User {
            id,
            pubkey: event.pubkey,
            created_at: event.created_at,
            content,
            blankame: get_blankname(event.pubkey),
            profile,
            author,
        })
    }

    fn into_mutate_event(&self, author: Author) -> Result<Event> {
        // assert!(author.public_key() == self.author);
        let tags = vec![Tag::new(NOSTR_ID_TAG, self.id.to_string().as_str())];
        Event::create(
            author,
            self.created_at,
            EventKind::MutateUser,
            tags,
            self.content.clone(),
        )
    }
}

impl User {
    async fn from_sql_row(row: &rusqlite::Row<'_>, client: &RouterClient) -> Result<User> {
        let event = Event::from_sql_row(row)?;
        Self::from_event(event, client).await
    }

    pub async fn create(repo: &Repo, profile: Profile) -> Result<User> {
        let id = Uuid::new_v4();
        let author_id = repo.router().authors().create().await?;
        let author = repo
            .router()
            .authors()
            .export(author_id)
            .await?
            .expect("just created author to exist");

        // add profile to store
        let content = serde_json::to_vec(&profile)?;
        let result = repo.router.blobs().add_bytes(content).await?;

        // TODO(b5) - wat. why? you're doing something wrong with types.
        let pubkey = PublicKey::from_bytes(author.public_key().as_bytes())?;

        let user = Self {
            id,
            pubkey,
            created_at: chrono::Utc::now().timestamp(),
            content: HashLink {
                hash: result.hash,
                value: None,
            },
            profile: Some(profile),
            blankame: get_blankname(pubkey),
            author: Some(author.clone()),
        };

        user.into_mutate_event(author)?.write(&repo.db).await?;
        Ok(user)
    }
}

pub struct Users(Repo);

impl Users {
    pub fn new(repo: Repo) -> Self {
        Users(repo)
    }

    pub async fn create(&self, profile: Profile) -> Result<User> {
        User::create(&self.0, profile).await
    }

    pub async fn mutate(&self, mut user: User) -> Result<User> {
        let author = user
            .author
            .clone()
            .ok_or_else(|| anyhow!("missing author"))?;
        user.created_at = chrono::Utc::now().timestamp();
        let event = user.into_mutate_event(author)?;
        event.write(&self.0.db).await?;
        Ok(user)
    }

    pub async fn authors(&self) -> Result<Vec<AuthorId>> {
        let mut author_ids = self.0.router.authors().list().await?;
        let mut authors = Vec::new();
        while let Some(author_id) = author_ids.next().await {
            let author_id = author_id?;
            authors.push(author_id);
        }
        Ok(authors)
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<Vec<User>> {
        let conn = self.0.db.lock().await;
        let mut stmt = conn.prepare("SELECT id, pubkey, created_at, kind, schema, data_id, content, sig FROM events WHERE kind = ?1 LIMIT ?2 OFFSET ?3")?;
        let mut rows = stmt.query(params![EventKind::MutateUser, limit, offset])?;

        let mut users = Vec::new();
        while let Some(row) = rows.next()? {
            let user = User::from_sql_row(row, &self.0.router).await?;
            users.push(user);
        }

        Ok(users)
    }
}

// TODO: have this accept a hash & use the hash to deterministically generate a name
fn get_blankname(key: PublicKey) -> String {
    let bytes = key.as_bytes();
    let adjectives = get_adjectives();
    let colors = get_color_names();
    let animals = get_animal_names();

    let adjective = adjectives[bytes[0] as usize % adjectives.len()];
    let color = colors[bytes[1] as usize % colors.len()];
    let animal = animals[bytes[2] as usize % animals.len()];

    format!("{}_{}_{}", adjective, color, animal)
}

fn get_adjectives() -> Vec<&'static str> {
    vec![
        "quick", "lazy", "sleepy", "noisy", "hungry", "happy", "sad", "angry", "brave", "calm",
        "eager", "fierce", "gentle", "jolly", "kind", "lively", "merry", "nice", "proud", "silly",
        "witty", "zealous", "bright", "dark", "shiny", "dull", "smooth", "rough", "soft", "hard",
    ]
}

fn get_color_names() -> Vec<&'static str> {
    vec![
        "red",
        "blue",
        "green",
        "yellow",
        "purple",
        "orange",
        "pink",
        "brown",
        "black",
        "white",
        "gray",
        "cyan",
        "magenta",
        "lime",
        "indigo",
        "violet",
        "gold",
        "silver",
        "bronze",
        "teal",
        "navy",
        "maroon",
        "olive",
        "coral",
        "peach",
        "mint",
        "lavender",
        "beige",
        "turquoise",
        "salmon",
    ]
}

fn get_animal_names() -> Vec<&'static str> {
    vec![
        "ant", "bat", "cat", "dog", "eel", "fox", "gnu", "hen", "ibis", "jay", "kiwi", "lynx",
        "mole", "newt", "owl", "pig", "quail", "rat", "seal", "toad", "urial", "vole", "wolf",
        "yak", "zebu", "bee", "cow", "duck", "frog", "goat",
    ]
}

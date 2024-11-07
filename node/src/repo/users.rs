use anyhow::Result;
use ed25519_dalek::{SigningKey, VerifyingKey};
use futures::StreamExt;
use iroh::docs::AuthorId;
use rand::prelude::SliceRandom;
use rand::rngs::OsRng;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::db::DB;
use super::Repo;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub pub_key: VerifyingKey,
    pub priv_key: Option<SigningKey>,
    pub name: String,
    pub about: String,
    pub picture: String,
}

impl User {
    pub(crate) async fn create(
        db: &DB,
        name: String,
        about: String,
        picture: String,
    ) -> Result<User> {
        let conn = db.lock().await;

        let mut cspring = OsRng;
        let signing_key = SigningKey::generate(&mut cspring);
        let pub_key = signing_key.verifying_key();
        let priv_key = signing_key.to_bytes();

        conn.execute(
            "INSERT INTO users (pubkey, privkey, name, about, picture) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![pub_key.to_bytes(), &priv_key, &name, &about, &picture],
        )?;
        Ok(User {
            pub_key,
            priv_key: Some(signing_key),
            name,
            about,
            picture,
        })
    }
}

pub struct Users(Repo);

impl Users {
    pub fn new(repo: Repo) -> Self {
        Users(repo)
    }

    // ensure agreement between iroh authors and repo authors
    async fn sync_authors(&self) -> Result<()> {
        let mut authors = self.0.router.authors().list().await?;
        let conn = self.0.db.lock().await;
        while let Some(author_id) = authors.next().await {
            let author_id = author_id?;
            conn.execute(
                "INSERT OR IGNORE INTO users (pubkey, name, about, picture) VALUES (?1, ?2, ?3, ?4)",
                params![author_id.to_bytes(), generate_name(), "", ""],
            )?;
        }
        Ok(())
    }

    pub async fn create(&self, name: String, about: String, picture: String) -> Result<User> {
        User::create(&self.0.db, name, about, picture).await
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

    pub async fn list(&self) -> Result<Vec<User>> {
        let conn = self.0.db.lock().await;
        let mut stmt = conn.prepare("SELECT pubkey, privkey, name, about, picture FROM users")?;
        let rows = stmt.query_map([], |row| {
            let pub_key =
                VerifyingKey::from_bytes(&row.get::<_, [u8; 32]>(0)?).expect("Invalid public key");
            Ok(User {
                pub_key,
                // priv_key: Some(SigningKey::from_bytes(&row.get::<_, Vec<u8>>(2))?),
                priv_key: None,
                name: row.get(2)?,
                about: row.get(3)?,
                picture: row.get(4)?,
            })
        })?;

        let mut users = Vec::new();
        for user in rows {
            users.push(user?);
        }

        Ok(users)
    }
}

// TODO: have this accept a hash & use the hash to deterministically generate a name
pub fn generate_name() -> String {
    let adjectives = get_adjectives();
    let animals = get_animal_names();
    let mut rng = rand::thread_rng();
    format!(
        "{}_{}",
        adjectives.choose(&mut rng).unwrap(),
        animals.choose(&mut rng).unwrap()
    )
}

fn get_adjectives() -> Vec<&'static str> {
    vec![
        "quick", "lazy", "sleepy", "noisy", "hungry", "happy", "sad", "angry", "brave", "calm",
        "eager", "fierce", "gentle", "jolly", "kind", "lively", "merry", "nice", "proud", "silly",
        "witty", "zealous", "bright", "dark", "shiny", "dull", "smooth", "rough", "soft", "hard",
    ]
}

fn get_animal_names() -> Vec<&'static str> {
    vec![
        "ant", "bat", "cat", "dog", "eel", "fox", "gnu", "hen", "ibis", "jay", "kiwi", "lynx",
        "mole", "newt", "owl", "pig", "quail", "rat", "seal", "toad", "urial", "vole", "wolf",
        "yak", "zebu", "bee", "cow", "duck", "frog", "goat",
    ]
}

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use events::{Event, EVENT_SQL_READ_FIELDS};
use iroh::docs::{Author, NamespaceId, NamespaceSecret};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::router::RouterClient;

use self::db::{open_db, setup_db, DB};

pub mod capabilities;
mod db;
pub mod events;
pub mod programs;
pub mod rows;
pub mod schemas;
pub mod space_events;
pub mod tickets;
pub mod users;

#[derive(Debug, Clone)]
pub struct Space {
    pub name: String,
    secret: SpaceSecret,
    router: RouterClient,
    db: DB,
}

impl Space {
    pub async fn open(
        name: String,
        secret: SpaceSecret,
        router: RouterClient,
        repo_base: impl Into<PathBuf>,
    ) -> Result<Self> {
        let path = repo_base.into().join(format!("{}.db", name));
        let db = open_db(&path).await?;
        setup_db(&db).await?;
        Ok(Space {
            name,
            secret,
            router,
            db,
        })
    }

    pub fn db(&self) -> &DB {
        &self.db
    }

    pub fn router(&self) -> &RouterClient {
        &self.router
    }

    pub fn users(&self) -> users::Users {
        users::Users::new(self.clone())
    }

    pub fn programs(&self) -> programs::Programs {
        programs::Programs::new(self.clone())
    }

    pub fn schemas(&self) -> schemas::Schemas {
        schemas::Schemas::new(self.clone())
    }

    pub fn rows(&self) -> rows::Rows {
        rows::Rows::new(self.clone())
    }

    pub async fn search(&self, query: &str, offset: i64, limit: i64) -> Result<Vec<Event>> {
        let conn = self.db.lock().await;
        let mut stmt = conn.prepare(
            format!("SELECT {EVENT_SQL_READ_FIELDS} FROM events WHERE content LIKE '%' || ?1 || '%' COLLATE NOCASE ORDER BY created_at DESC LIMIT ?2 OFFSET ?3").as_str()
        )?;
        let mut rows = stmt.query(params![query, limit, offset])?;
        let mut events = Vec::new();
        while let Some(row) = rows.next()? {
            events.push(Event::from_sql_row(row)?);
        }
        Ok(events)
    }
}

const SPACES_FILENAME: &str = "spaces.json";

#[derive(Debug, Deserialize, Serialize)]
pub struct SpaceDetails {
    name: String,
    secret: SpaceSecret,
}

pub type SpaceSecret = NamespaceSecret;
pub type SpaceId = NamespaceId;

#[derive(Debug, Clone)]
pub struct Spaces {
    path: PathBuf,
    spaces: Arc<RwLock<HashMap<String, Space>>>,
}

impl Spaces {
    pub async fn open_all(router: RouterClient, base_path: impl Into<PathBuf>) -> Result<Self> {
        let path = base_path.into();
        println!("open spaces at path {:?}", path);
        let spaces = Self::read_from_file(&path).await?;
        let mut map = HashMap::new();
        for deets in spaces {
            let space = Space::open(deets.name, deets.secret, router.clone(), path.clone()).await?;
            map.insert(space.name.clone(), space);
        }
        Ok(Self {
            path,
            spaces: Arc::new(RwLock::new(map)),
        })
    }

    pub async fn get_or_create(
        &mut self,
        router: &RouterClient,
        author: Author,
        name: &str,
        description: &str,
    ) -> Result<Space> {
        if let Some(space) = self.get(name).await {
            return Ok(space);
        }
        self.create(router, author, name, description).await
    }

    pub async fn create(
        &mut self,
        router: &RouterClient,
        author: Author,
        name: &str,
        description: &str,
    ) -> Result<Space> {
        let secret = NamespaceSecret::new(&mut rand::thread_rng());
        let new = SpaceDetails {
            name: name.to_string(),
            secret: secret.clone(),
        };
        let space =
            Space::open(name.to_string(), secret, router.clone(), self.path.clone()).await?;
        space_events::SpaceEvents::new(space.clone())
            .create(
                router,
                author,
                space_events::SpaceDetails {
                    title: name.to_string(),
                    description: description.to_string(),
                },
            )
            .await?;
        let mut spaces = self.spaces.write().await;
        spaces.insert(name.to_string().clone(), space.clone());

        let mut details = Spaces::read_from_file(self.path.join(SPACES_FILENAME)).await?;
        details.push(new);
        self.write_to_file(details).await?;

        Ok(space)
    }

    pub async fn get(&self, name: &str) -> Option<Space> {
        self.spaces.read().await.get(name).cloned()
    }

    fn spaces_path(path: impl Into<PathBuf>) -> PathBuf {
        path.into().join(SPACES_FILENAME)
    }

    async fn read_from_file(base_path: impl Into<PathBuf>) -> Result<Vec<SpaceDetails>> {
        let path = Self::spaces_path(base_path);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = tokio::fs::read(&path).await?;
        let spaces: Vec<SpaceDetails> = serde_json::from_slice(&file)?;
        Ok(spaces)
    }

    async fn write_to_file(&self, details: Vec<SpaceDetails>) -> Result<()> {
        let file = serde_json::to_vec(&details)?;
        tokio::fs::write(Self::spaces_path(&self.path), file).await?;
        Ok(())
    }

    pub async fn list(&self, _offset: i64, _limit: i64) -> Result<Vec<SpaceDetails>> {
        let results = Self::read_from_file(&self.path).await?;
        Ok(results)
    }

    // async fn write_all(
    //     base_path: impl Into<PathBuf>,
    //     spaces: HashMap<String, Space>,
    // ) -> Result<()> {
    //     let path = base_path.into();
    //     let spaces = spaces
    //         .into_iter()
    //         .map(|(name, space)| SpaceDetails {
    //             name,
    //             secret: space.secret,
    //         })
    //         .collect::<Vec<_>>();
    //     Spaces::write_to_file(path, spaces).await
    // }
}

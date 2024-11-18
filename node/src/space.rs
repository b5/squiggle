use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use iroh::docs::{NamespaceId, NamespaceSecret};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

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
    db: DB,
}

impl Space {
    pub async fn open(
        name: String,
        secret: SpaceSecret,
        repo_base: impl Into<PathBuf>,
    ) -> Result<Self> {
        let path = repo_base.into().join(format!("{}.db", name));
        let db = open_db(&path).await?;
        setup_db(&db).await?;
        Ok(Space { name, secret, db })
    }

    pub fn db(&self) -> &DB {
        &self.db
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
    pub async fn open_all(base_path: impl Into<PathBuf>) -> Result<Self> {
        let path = base_path.into();
        let spaces = Self::read_from_file(&path.join(SPACES_FILENAME)).await?;
        let mut map = HashMap::new();
        for deets in spaces {
            let space = Space::open(deets.name, deets.secret, path.clone()).await?;
            map.insert(space.name.clone(), space);
        }
        Ok(Self {
            path,
            spaces: Arc::new(RwLock::new(map)),
        })
    }

    pub async fn get_or_create(&mut self, name: &str) -> Result<Space> {
        if let Some(space) = self.get(name).await {
            return Ok(space);
        }
        self.create(name).await
    }

    pub async fn create(&mut self, name: &str) -> Result<Space> {
        let secret = NamespaceSecret::new(&mut rand::thread_rng());
        let new = SpaceDetails {
            name: name.to_string(),
            secret: secret.clone(),
        };
        let space = Space::open(name.to_string(), secret, self.path.clone()).await?;
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

    async fn read_from_file(base_path: impl Into<PathBuf>) -> Result<Vec<SpaceDetails>> {
        let path = base_path.into().join(SPACES_FILENAME);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = tokio::fs::read(&path).await?;
        let spaces: Vec<SpaceDetails> = serde_json::from_slice(&file)?;
        Ok(spaces)
    }

    async fn write_to_file(&self, details: Vec<SpaceDetails>) -> Result<()> {
        let path = self.path.join(SPACES_FILENAME);
        let file = serde_json::to_vec(&details)?;
        tokio::fs::write(&path, file).await?;
        Ok(())
    }

    // async fn write_all(
    //     base_path: impl Into<PathBuf>,
    //     spaces: HashMap<String, Space>,
    // ) -> Result<()> {
    //     let path = base_path.into().join(SPACES_FILENAME);
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

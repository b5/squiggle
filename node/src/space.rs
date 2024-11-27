use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use events::{Event, EVENT_SQL_READ_FIELDS};
use iroh::base::ticket::BlobTicket;
use iroh::blobs::Hash;
use iroh::docs::{NamespaceId, NamespaceSecret};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use space_events::{SpaceEvent, SpaceEvents};
use sync::Sync;
use tokio::sync::RwLock;
use users::User;
use uuid::Uuid;

use crate::router::RouterClient;

use self::db::{open_db, setup_db, DB};

pub mod capabilities;
mod db;
pub mod events;
pub mod programs;
pub mod rows;
pub mod secrets;
pub mod sharing;
pub mod space_events;
pub mod sync;
pub mod tables;
pub mod tickets;
pub mod users;

#[derive(Debug, Clone)]
pub struct Space {
    path: PathBuf,
    pub id: Uuid,
    pub name: String,
    secret: SpaceSecret,

    db: DB,
    router: RouterClient,
    sync: Option<Sync>,
}

impl Space {
    pub async fn open(
        id: Uuid,
        name: String,
        secret: SpaceSecret,
        router: RouterClient,
        repo_base: impl Into<PathBuf>,
    ) -> Result<Self> {
        let path = repo_base.into();
        let db = open_db(&path.join(format!("{}.db", name))).await?;
        setup_db(&db).await?;

        Ok(Space {
            path,
            id,
            name,
            secret,
            router,
            sync: None,
            db,
        })
    }

    pub async fn start_sync(&mut self) -> Result<()> {
        if self.sync.is_some() {
            return Err(anyhow!("sync already started"));
        }

        let sync = Sync::start(&self.db, &self.router, self.secret.id()).await?;
        self.sync = Some(sync);
        Ok(())
    }

    pub fn db(&self) -> &DB {
        &self.db
    }

    pub fn router(&self) -> &RouterClient {
        &self.router
    }

    pub fn details(&self) -> SpaceDetails {
        SpaceDetails {
            id: self.id,
            name: self.name.clone(),
            // TODO: nooooooo
            secret: self.secret.clone(),
        }
    }

    pub fn users(&self) -> users::Users {
        users::Users::new(self.clone())
    }

    pub fn capabilities(&self) -> capabilities::Capabilities {
        capabilities::Capabilities::new(self.clone())
    }

    pub fn programs(&self) -> programs::Programs {
        programs::Programs::new(self.clone())
    }

    pub fn secrets(&self) -> secrets::Secrets {
        secrets::Secrets::new(self.clone())
    }

    pub fn tables(&self) -> tables::Tables {
        tables::Tables::new(self.clone())
    }

    pub fn rows(&self) -> rows::Rows {
        rows::Rows::new(self.clone())
    }

    pub async fn share(&self) -> Result<iroh::base::ticket::BlobTicket> {
        let first = self.users().list(0, 1).await?;
        let first = first.first().ok_or_else(|| anyhow!("no users"))?;
        sharing::export_space(self, first).await
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

    pub async fn info(&self) -> Result<SpaceEvent> {
        SpaceEvents::new(self.clone()).read().await
    }

    async fn merge_db(&self, other_sqlite_db_hash: Hash) -> Result<()> {
        let their_db_path = self.path.join(format!("{}.them.db", self.name));
        self.router
            .blobs()
            .export(
                other_sqlite_db_hash,
                their_db_path.clone(),
                iroh::blobs::store::ExportFormat::Blob,
                iroh::blobs::store::ExportMode::Copy,
            )
            .await?;

        let conn = self.db.lock().await;
        let mut stmt = conn.prepare("ATTACH DATABASE ?1 AS other")?;
        stmt.execute(params![their_db_path.to_string_lossy()])?;

        todo!("finish this");
        // let mut stmt = conn.prepare("SELECT name FROM other.sqlite_master WHERE type='table'")?;
        // let mut tables: Vec<String> = Vec::new();
        // let tables = stmt.query_map(params![], |row| row.get(0))?;

        // for table in tables {
        //     let table = table?;
        //     let mut stmt = conn.prepare(format!("SELECT * FROM other.{}", table).as_str())?;
        //     let mut rows = stmt.query(params![])?;
        //     while let Some(row) = rows.next()? {
        //         let mut stmt =
        //             conn.prepare(format!("INSERT INTO {} VALUES (?)", table).as_str())?;
        //         stmt.execute(params![row])?;
        //     }
        // }

        // // drop external database
        // let mut stmt = conn.prepare("DETACH DATABASE other")?;
        // stmt.execute(params![])?;

        // tokio::fs::remove_file(their_db_path).await?;

        // Ok(())
    }

    fn db_filename(&self) -> String {
        format!("{}.db", self.name)
    }
}

const SPACES_FILENAME: &str = "spaces.json";

#[derive(Debug, Deserialize, Serialize)]
pub struct SpaceDetails {
    pub id: Uuid,
    pub name: String,
    pub secret: SpaceSecret,
}

pub type SpaceSecret = NamespaceSecret;
pub type SpaceId = NamespaceId;

#[derive(Debug, Clone)]
pub struct Spaces {
    path: PathBuf,
    spaces: Arc<RwLock<HashMap<Uuid, Space>>>,
}

impl Spaces {
    pub async fn open_all(router: RouterClient, base_path: impl Into<PathBuf>) -> Result<Self> {
        let path = base_path.into();
        let spaces = Self::read_from_file(&path).await?;
        let mut map = HashMap::new();
        for deets in spaces {
            let space = Space::open(
                deets.id,
                deets.name,
                deets.secret,
                router.clone(),
                path.clone(),
            )
            .await?;
            map.insert(space.id.clone(), space);
        }
        Ok(Self {
            path,
            spaces: Arc::new(RwLock::new(map)),
        })
    }

    pub async fn get_or_create(
        &mut self,
        router: &RouterClient,
        user: &User,
        name: &str,
        description: &str,
    ) -> Result<Space> {
        if let Some(space) = self.get_by_name(name).await {
            return Ok(space);
        }
        self.create(router, user, name, description).await
    }

    pub async fn create(
        &mut self,
        router: &RouterClient,
        user: &User,
        name: &str,
        description: &str,
    ) -> Result<Space> {
        // create the space
        let id = Uuid::new_v4();
        let author = user.author.clone().expect("author to exist");
        let secret = NamespaceSecret::new(&mut rand::thread_rng());
        let new = SpaceDetails {
            id,
            name: name.to_string(),
            secret: secret.clone(),
        };
        let space = Space::open(
            id,
            name.to_string(),
            secret,
            router.clone(),
            self.path.clone(),
        )
        .await?;
        space_events::SpaceEvents::new(space.clone())
            .mutate(
                author,
                id,
                space_events::SpaceDetails {
                    title: name.to_string(),
                    description: description.to_string(),
                },
            )
            .await?;
        let mut spaces = self.spaces.write().await;
        spaces.insert(id.clone(), space.clone());

        // write user details into the space
        user.write(&space).await?;

        let mut details = Spaces::read_from_file(self.path.join(SPACES_FILENAME)).await?;
        details.push(new);
        self.write_to_file(details).await?;

        Ok(space)
    }

    pub async fn get(&self, id: &Uuid) -> Option<Space> {
        self.spaces.read().await.get(id).cloned()
    }

    pub async fn get_by_name(&self, name: &str) -> Option<Space> {
        self.spaces
            .read()
            .await
            .values()
            .find(|space| space.name == name)
            .cloned()
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

    pub async fn add_or_sync_from_collection(
        &self,
        router: &RouterClient,
        ticket: BlobTicket,
    ) -> Result<Space> {
        let blobs = router.blobs();
        blobs
            .download_hash_seq(ticket.hash(), ticket.node_addr().clone())
            .await?
            .finish()
            .await?;

        let collection = blobs.get_collection(ticket.hash()).await?;
        let (_, space_element_hash) = collection
            .clone()
            .into_iter()
            .find(|item| item.0 == crate::space::sharing::SPACE_COLLECTION_FILENAME)
            .ok_or_else(|| anyhow!("space.json not found in collection"))?;

        let (_, db_element_hash) = collection
            .into_iter()
            .find(|item| item.0 == crate::space::sharing::SPACE_COLLECTION_DB_FILENAME)
            .ok_or_else(|| anyhow!("space.db not found in collection"))?;

        let space_data = blobs.read_to_bytes(space_element_hash).await?;
        let space: SpaceDetails = serde_json::from_slice(&space_data)?;
        let space = match self.get(&space.id).await {
            Some(space) => {
                // we've seen this space before, merge the external database with our existing one
                space.merge_db(db_element_hash).await?;
                // TODO - update space details
                space
            }
            None => {
                // we've never seen this space before, create a new one

                // copy database file to space path name
                blobs
                    .export(
                        db_element_hash,
                        self.path.join(format!("{}.db", space.name)),
                        iroh::blobs::store::ExportFormat::Blob,
                        iroh::blobs::store::ExportMode::Copy,
                    )
                    .await?;

                let space = Space::open(
                    space.id,
                    space.name,
                    space.secret,
                    router.clone(),
                    self.path.clone(),
                )
                .await?;
                let mut spaces = self.spaces.write().await;
                spaces.insert(space.id.clone(), space.clone());
                space
            }
        };

        Ok(space)
    }
}

use std::path::PathBuf;

use anyhow::Result;

use self::db::{open_db, setup_db, DB};
use crate::router::RouterClient;

mod db;
pub mod events;
pub mod schemas;
pub mod users;

#[derive(Debug, Clone)]
pub struct Repo {
    db: DB,
    router: RouterClient,
}

impl Repo {
    pub async fn open(router: RouterClient, path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into().join("db.sqlite");
        let db = open_db(&path).await?;
        setup_db(&db).await?;
        Ok(Repo { router, db })
    }

    pub fn router(&self) -> &RouterClient {
        &self.router
    }

    pub fn users(&self) -> users::Users {
        users::Users::new(self.db.clone())
    }

    pub fn schemas(&self) -> schemas::Schemas {
        schemas::Schemas::new(self.clone())
    }

    pub fn events(&self) -> events::Events {
        events::Events::new(self.clone())
    }
}

use std::path::PathBuf;

use anyhow::Result;

use self::db::{open_db, setup_db, DB};

mod db;
mod events;
mod users;

pub struct Repo {
    db: DB,
}

impl Repo {
    pub async fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into().join("db.sqlite");
        let db = open_db(&path).await?;
        setup_db(&db).await?;
        Ok(Repo { db })
    }

    pub async fn list_events(&self) -> Result<Vec<events::Event>> {
        events::list(&self.db).await
    }

    pub fn users(&self) -> users::Users {
        users::Users::new(self.db.clone())
    }
}

use std::path::PathBuf;

use anyhow::Result;

use crate::repo::Repo;
use crate::router::Router;

pub struct Node {
    pub path: PathBuf,
    pub name: String,
    router: Router,
    repo: Repo,
}

impl Node {
    pub async fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let router = crate::router::router(&path).await?;
        let repo = Repo::open(&path).await?;
        Ok(Node {
            path,
            name: "node".to_string(),
            router,
            repo,
        })
    }

    pub fn repo(&self) -> &Repo {
        &self.repo
    }
}

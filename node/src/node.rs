use std::path::PathBuf;

use anyhow::Result;

use crate::repo::Repo;
use crate::router::Router;
use crate::vm::VM;

pub struct Node {
    pub path: PathBuf,
    pub name: String,
    router: Router,
    repo: Repo,
    vm: VM,
}

impl Node {
    pub async fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let router = crate::router::router(&path).await?;
        let repo = Repo::open(&path).await?;
        let vm = VM::new(router.client().clone(), &path).await?;
        Ok(Node {
            path,
            name: "node".to_string(),
            router,
            repo,
            vm,
        })
    }

    pub fn repo(&self) -> &Repo {
        &self.repo
    }
}

use std::path::PathBuf;

use anyhow::Result;
use iroh::util::path::IrohPaths;

use crate::repo::Repo;
use crate::router::Router;
use crate::vm::VM;

pub struct Node {
    path: PathBuf,
    router: Router,
    repo: Repo,
    vm: VM,
}

impl Node {
    pub async fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let router = crate::router::router(&path).await?;

        // add the node key as an author:
        // TODO(b5): this is an anti-pattern, remove.
        let secret_key =
            iroh::util::fs::load_secret_key(IrohPaths::SecretKey.with_root(&path)).await?;
        let author = iroh::docs::Author::from_bytes(&secret_key.to_bytes());
        router.authors().import(author.clone()).await?;

        let repo = Repo::open(&path).await?;
        let vm = VM::new(router.client().clone(), &path).await?;

        Ok(Node {
            path,
            router,
            repo,
            vm,
        })
    }

    pub fn router(&self) -> &Router {
        &self.router
    }

    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    pub fn vm(&self) -> &VM {
        &self.vm
    }
}

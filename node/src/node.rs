use std::env;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use iroh::util::path::IrohPaths;

use crate::repo::Repo;
use crate::router::Router;
use crate::vm::VM;

pub struct Node {
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

        let repo = Repo::open(router.client().clone(), &path).await?;
        let vm = VM::new(repo.clone(), &path).await?;

        Ok(Node { router, repo, vm })
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

/// Name of directory that wraps all datalayer files in a given application directory
const DATALAYER_DIR: &str = "datalayer";

/// Returns the path to the user's iroh data directory.
///
/// If the `DATALAYER_DATA_DIR` environment variable is set it will be used unconditionally.
/// Otherwise the returned value depends on the operating system according to the following
/// table.
///
/// | Platform | Value                                         | Example                                  |
/// | -------- | --------------------------------------------- | ---------------------------------------- |
/// | Linux    | `$XDG_DATA_HOME`/iroh or `$HOME`/.local/share/iroh | /home/alice/.local/share/iroh                 |
/// | macOS    | `$HOME`/Library/Application Support/iroh      | /Users/Alice/Library/Application Support/iroh |
/// | Windows  | `{FOLDERID_RoamingAppData}/iroh`              | C:\Users\Alice\AppData\Roaming\iroh           |
pub fn data_root() -> Result<PathBuf> {
    let path = if let Some(val) = env::var_os("DATALAYER_DATA_DIR") {
        PathBuf::from(val)
    } else {
        let path = dirs_next::data_dir().ok_or_else(|| {
            anyhow!("operating environment provides no directory for application data")
        })?;
        path.join(DATALAYER_DIR)
    };
    let path = if !path.is_absolute() {
        std::env::current_dir()?.join(path)
    } else {
        path
    };
    Ok(path)
}

use std::env;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use futures::StreamExt;
use iroh::docs::AuthorId;
use iroh::util::path::IrohPaths;
use tokio::task::JoinHandle;

use crate::router::Router;
use crate::space::Spaces;
use crate::vm::{VMConfig, VM};

pub struct Node {
    spaces: Spaces,
    router: Router,
    vm: VM,
}

impl Node {
    pub async fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let repo_path = path.into();
        let router = crate::router::router(&repo_path).await?;

        // add the node key as an author:
        // TODO(b5): this is an anti-pattern, remove.
        let secret_key =
            iroh::util::fs::load_secret_key(IrohPaths::SecretKey.with_root(&repo_path)).await?;
        let author = iroh::docs::Author::from_bytes(&secret_key.to_bytes());
        router.authors().import(author.clone()).await?;

        let spaces = Spaces::open_all(repo_path.clone()).await?;
        let vm = VM::create(
            spaces.clone(),
            router.client(),
            VMConfig {
                autofetch: crate::vm::content_routing::AutofetchPolicy::Disabled,
                worker_root: repo_path,
            },
        )
        .await?;

        Ok(Node { router, spaces, vm })
    }

    pub fn spaces(&self) -> &Spaces {
        &self.spaces
    }

    pub fn router(&self) -> &Router {
        &self.router
    }

    pub fn vm(&self) -> &VM {
        &self.vm
    }

    pub async fn accounts(&self) -> Result<Vec<AuthorId>> {
        let mut author_ids = self.router.authors().list().await?;
        let mut authors = Vec::new();
        while let Some(author_id) = author_ids.next().await {
            let author_id = author_id?;
            authors.push(author_id);
        }
        Ok(authors)
    }

    pub async fn gateway(&self, serve_addr: &str) -> Result<JoinHandle<()>> {
        let addr = self.router.net().node_addr().await?;
        let serve_addr = serve_addr.to_string();
        let handle = tokio::spawn(async move {
            crate::gateway::server::run(addr, serve_addr)
                .await
                .expect("gateway failed");
        });

        Ok(handle)
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

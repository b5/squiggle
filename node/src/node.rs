use std::env;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use tokio::task::JoinHandle;

use crate::accounts::Accounts;
use crate::iroh::Protocols;
use crate::space::users::Profile;
use crate::space::Spaces;
use crate::vm::{VMConfig, VM};

pub struct Node {
    accounts: Accounts,
    spaces: Spaces,
    iroh: Protocols,
    vm: VM,
}

impl Node {
    pub async fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let repo_path = path.into();
        let iroh_node = crate::iroh::Protocols::spawn(repo_path.clone())
            .await
            .context("spawing iroh router")?;

        // let node_id = iroh_node.endpoint().node_addr().await?;

        let mut accounts = Accounts::open(&repo_path)
            .await
            .context("opening accounts db")?;

        if accounts.list(0, -1).await?.is_empty() {
            // add the node key as an author:
            // TODO(b5): this is an anti-pattern, remove.
            let author_id = iroh_node.docs().authors().default().await?;
            let author = iroh_node.docs().authors().export(author_id).await?.unwrap();

            // iroh::util::fs::load_secret_key(IrohPaths::SecretKey.with_root(&repo_path)).await?;
            // let author = iroh::docs::Author::from_bytes(&secret_key.to_bytes());
            // iroh_node.authors().import(author.clone()).await?;

            accounts
                .create_account(
                    author,
                    Profile {
                        // TODO: finish:
                        node_ids: vec![],
                        ..Default::default()
                    },
                )
                .await
                .context("creating account")?;
        }

        let spaces = Spaces::open_all(iroh_node.clone(), repo_path.clone())
            .await
            .context("opening spaces db")?;
        let vm = VM::create(
            spaces.clone(),
            &iroh_node,
            VMConfig {
                autofetch: crate::vm::content_routing::AutofetchPolicy::Disabled,
                worker_root: repo_path,
            },
        )
        .await?;

        Ok(Node {
            accounts,
            iroh: iroh_node,
            spaces,
            vm,
        })
    }

    pub fn accounts(&self) -> &Accounts {
        &self.accounts
    }

    pub fn spaces(&self) -> &Spaces {
        &self.spaces
    }

    pub fn iroh(&self) -> &Protocols {
        &self.iroh
    }

    pub fn vm(&self) -> &VM {
        &self.vm
    }

    pub async fn gateway(&self, serve_addr: &str) -> Result<JoinHandle<()>> {
        let addr = self.iroh().endpoint().node_addr().await?;
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
const SQUIGGLE_DATA_DIR: &str = "squiggle";

/// Returns the path to the user's iroh data directory.
///
/// If the `SQUIGGLE_DATA_DIR` environment variable is set it will be used unconditionally.
/// Otherwise the returned value depends on the operating system according to the following
/// table.
///
/// | Platform | Value                                         | Example                                  |
/// | -------- | --------------------------------------------- | ---------------------------------------- |
/// | Linux    | `$XDG_DATA_HOME`/iroh or `$HOME`/.local/share/iroh | /home/alice/.local/share/iroh                 |
/// | macOS    | `$HOME`/Library/Application Support/iroh      | /Users/Alice/Library/Application Support/iroh |
/// | Windows  | `{FOLDERID_RoamingAppData}/iroh`              | C:\Users\Alice\AppData\Roaming\iroh           |
pub fn data_root() -> Result<PathBuf> {
    let path = if let Some(val) = env::var_os(SQUIGGLE_DATA_DIR) {
        PathBuf::from(val)
    } else {
        let path = dirs_next::data_dir().ok_or_else(|| {
            anyhow!("operating environment provides no directory for application data")
        })?;
        path.join(SQUIGGLE_DATA_DIR)
    };
    let path = if !path.is_absolute() {
        std::env::current_dir()?.join(path)
    } else {
        path
    };
    Ok(path)
}

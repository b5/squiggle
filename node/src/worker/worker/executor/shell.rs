use std::path::PathBuf;

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Shell {
    /// Execution root
    root: PathBuf,
}

impl Shell {
    pub async fn new(root: PathBuf) -> Result<Self> {
        tokio::fs::create_dir_all(&root).await?;

        Ok(Self {
            root,
        })
    }
}

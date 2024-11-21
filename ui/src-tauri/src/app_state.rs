use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use squiggle_node::node::Node;
use uuid::Uuid;

const APP_STATE_FILENAME: &str = "app_state.json";

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AppState {
    write_path: PathBuf,
    pub current_space_id: Uuid,
}

impl AppState {
    pub async fn open_or_create(base_path: impl Into<PathBuf>, node: &Node) -> Result<Self> {
        let path = base_path.into().join(APP_STATE_FILENAME);
        if path.exists() {
            Self::open(path)
        } else {
            Self::create(path, node).await
        }
    }

    fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let state = std::fs::read_to_string(&path)?;
        let state: Self = serde_json::from_str(&state)?;
        Ok(Self {
            write_path: path,
            ..state
        })
    }

    async fn create(path: impl Into<PathBuf>, node: &Node) -> Result<Self> {
        let path = path.into();

        // default to the first space we find
        let spaces = node.spaces().list(0, 1).await?;
        let space = spaces.first().expect("no spaces found");

        let state = Self {
            write_path: path.clone(),
            current_space_id: space.id,
        };
        state.write_to_file().await?;
        Ok(state)
    }

    async fn write_to_file(&self) -> Result<()> {
        let state = serde_json::to_string(self)?;
        tokio::fs::write(&self.write_path, state).await?;
        Ok(())
    }
}

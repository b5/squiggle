use anyhow::Result;
use std::path::PathBuf;

use crate::router::RouterClient;

mod api;
mod blobs;
mod config;
mod content_routing;
mod doc;
mod docker;
mod flow;
mod job;
mod metrics;
mod scheduler;
mod worker;
mod workspace;

#[cfg(test)]
mod test_utils;

pub struct VM {
    workspaces: workspace::Workspaces,
}

impl VM {
    pub async fn new(router: RouterClient, path: impl Into<PathBuf>) -> Result<Self> {
        let cfg = config::NodeConfig::default();
        let workspaces = workspace::Workspaces::load_or_create(router, path, cfg).await?;
        Ok(VM { workspaces })
    }
}

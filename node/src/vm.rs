use anyhow::Result;
use std::path::PathBuf;

use crate::router::RouterClient;
use crate::vm::flow::Flow;

mod api;
mod blobs;
mod config;
mod content_routing;
mod doc;
mod docker;
pub mod flow;
mod job;
mod metrics;
mod scheduler;
mod worker;
mod workspace;

#[cfg(test)]
mod test_utils;

pub const DEFAULT_WORKSPACE: &str = "default";

pub struct VM {
    router: RouterClient,
    workspaces: workspace::Workspaces,
}

impl VM {
    pub async fn new(router: RouterClient, path: impl Into<PathBuf>) -> Result<Self> {
        // TODO(b5): move configuration up a level
        let cfg = config::NodeConfig::default();
        let workspaces = workspace::Workspaces::load_or_create(router.clone(), path, cfg).await?;
        if !workspaces.contains(DEFAULT_WORKSPACE).await {
            workspaces.create(DEFAULT_WORKSPACE).await?;
        }
        Ok(VM { router, workspaces })
    }

    // path is the path to a flow.toml to run
    pub async fn run(&self, ws: &str, flow: Flow) -> Result<()> {
        let workspace = self
            .workspaces
            .get(ws)
            .await
            .expect(format!("unknown workspace: {}", ws).as_str());
        let res = flow.run(&self.router, &workspace).await?;
        println!("flow result: {:?}", res);
        Ok(())
    }
}

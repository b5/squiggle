use anyhow::{Context, Result};
use iroh::docs::Author;
use job::{Artifacts, JobDescription, DEFAULT_TIMEOUT};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use flow::{Flow, FlowOutput, Task, TaskOutput};

use crate::repo::Repo;

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
    repo: Repo,
    workspaces: workspace::Workspaces,
}

impl VM {
    pub async fn new(repo: Repo, path: impl Into<PathBuf>) -> Result<Self> {
        // TODO(b5): move configuration up a level
        let cfg = config::NodeConfig::default();
        let workspaces = workspace::Workspaces::load_or_create(repo.clone(), path, cfg).await?;
        if !workspaces.contains(DEFAULT_WORKSPACE).await {
            workspaces.create(DEFAULT_WORKSPACE).await?;
        }
        Ok(VM { repo, workspaces })
    }

    // path is the path to a flow.toml to run
    pub async fn run_flow(&self, ws: &str, flow: Flow) -> Result<FlowOutput> {
        let workspace = self.workspaces.get(ws).await.context("unknown workspace")?;
        let res = flow.run(&self.repo, &workspace).await?;
        Ok(res)
    }

    pub async fn run_program(
        &self,
        workspace: &str,
        author: Author,
        id: Uuid,
        environment: HashMap<String, String>,
    ) -> Result<TaskOutput> {
        let workspace = self
            .workspaces
            .get(workspace)
            .await
            .context("getting workspace")?;
        let program = self.repo.programs().get_by_id(id).await?;

        let program_entry_hash = program.program_entry.context("program has no main entry")?;
        // construct a task so we can schedule it with the VM
        let task = Task {
            tasks: vec![],
            description: JobDescription {
                name: program.manifest.name.clone(),
                author: author.id().to_string(),
                environment,
                details: job::JobDetails::Wasm {
                    module: job::Source::LocalBlob(program_entry_hash),
                },
                artifacts: Artifacts::default(),
                timeout: DEFAULT_TIMEOUT,
            },
        };
        let result = Flow {
            name: program.manifest.name,
            tasks: vec![task],
            uploads: Default::default(),
            downloads: Default::default(),
        }
        .run(&self.repo, &workspace)
        .await?;
        let output = result.tasks.first().expect("single task").clone();
        Ok(output)
    }
}

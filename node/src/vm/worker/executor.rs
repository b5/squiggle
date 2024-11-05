use std::path::Path;

use anyhow::{bail, Result};
use tracing::{debug, warn};

use crate::repo::Repo;
use crate::vm::{
    blobs::Blobs,
    job::{JobContext, JobType},
};

use self::{docker::Docker, wasm::WasmExecutor};

pub mod docker;
pub mod wasm;

/// Defines the ability to execute work.
pub trait Executor {
    /// Executor specifc job details.
    type Job;
    /// Executor specific
    type Report;

    async fn execute(&self, ctx: &JobContext, job: Self::Job) -> Result<Self::Report>;
}

#[derive(Debug, Clone)]
pub struct Executors {
    docker: Option<Docker>,
    wasm: WasmExecutor,
}

impl Executors {
    pub async fn new(repo: Repo, blobs: Blobs, root: impl AsRef<Path>) -> Result<Self> {
        let docker_root = root.as_ref().join("docker");
        let docker = match Docker::new(repo.clone(), blobs.clone(), docker_root).await {
            Ok(docker) => Some(docker),
            Err(err) => {
                debug!("docker error: {:?}", err);
                warn!("Docker is not available, worker capability will not be started");
                None
            }
        };
        let wasm_root = root.as_ref().join("wasm");
        let wasm = WasmExecutor::new(repo, blobs, wasm_root).await?;

        Ok(Self { docker, wasm })
    }

    pub fn supports_job_type(&self, t: &JobType) -> bool {
        match t {
            JobType::Docker => self.docker.is_some(),
            JobType::Wasm => true,
        }
    }

    pub async fn execute_docker(
        &self,
        ctx: &JobContext,
        job: docker::Job,
    ) -> Result<docker::Report> {
        let Some(ref docker) = self.docker else {
            bail!("no docker executor available");
        };

        docker.execute(ctx, job).await
    }

    pub async fn execute_wasm(&self, ctx: &JobContext, job: wasm::Job) -> Result<wasm::Report> {
        self.wasm.execute(ctx, job).await
    }
}

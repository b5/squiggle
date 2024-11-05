#![allow(clippy::too_many_arguments)]

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use futures::StreamExt;
use futures_buffered::try_join_all;
use iroh::blobs::Hash;
use iroh::client::docs::Entry;
use iroh::client::Doc;
use iroh::docs::AuthorId;
use iroh::net::NodeId;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::repo::Repo;

use super::blobs::Blobs;
use super::doc::{DocEventHandler, Event, EventData};
use super::job::{
    JobContext, JobDetails, JobNameContext, JobOutput, JobResult, JobResultStatus, JobStatus,
    JobType, ScheduledJob, JOBS_PREFIX,
};
use super::metrics::Metrics;
use super::scheduler::{parse_status, SchedulerEvent};

use self::executor::Executors;

pub(crate) const WORKER_PREFIX: &str = "worker";

mod executor;

#[derive(Clone, Debug)]
pub struct Worker {
    author_id: AuthorId,
    executors: Executors,
    doc: Doc,
    blobs: Blobs,
    repo: Repo,
    current_jobs: Arc<Mutex<HashSet<Uuid>>>,
    /// If this worker will accept work.
    enabled: Arc<AtomicBool>,
}

impl Worker {
    pub async fn new(
        author_id: AuthorId,
        doc: Doc,
        blobs: Blobs,
        repo: Repo,
        root: impl AsRef<Path>,
    ) -> Result<Self> {
        let executors = Executors::new(repo.clone(), blobs.clone(), root).await?;
        let w = Self {
            author_id,
            executors,
            doc,
            blobs,
            repo,
            current_jobs: Default::default(),
            enabled: Arc::new(AtomicBool::new(true)),
        };
        Ok(w)
    }

    /// Enable this worker to accept work.
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
    }

    /// Disable this worker to accept work.
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }

    /// Is this worker accepting work?
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Get the current scheduling status of a job on this node by id.
    pub async fn read_job_status(&self, job_id: Uuid) -> Result<JobStatus> {
        let job_id = job_id.as_u128();
        let mut status: Option<JobStatus> = None;

        // query from all authors
        let q = iroh::docs::store::Query::all()
            .key_prefix(format!("{}/status/{}/", JOBS_PREFIX, job_id));
        let mut entries = self.doc.get_many(q).await?;

        while let Some(entry) = entries.next().await {
            let entry = entry?;
            let key = std::str::from_utf8(entry.key())?;

            debug!("checking status: {}", key);
            let (_, read_status) = parse_status(key)?;

            match status {
                Some(ref mut s) => {
                    s.merge(read_status);
                }
                None => {
                    status.replace(read_status);
                }
            }
        }

        let status = status.ok_or_else(|| anyhow!("job not found: {}", job_id))?;
        Ok(status)
    }

    async fn request_job(&self, job_id: Uuid, job_hash: Hash, job_hash_len: u64) -> Result<()> {
        debug!("requesting job {}", job_id);
        iroh_metrics::inc!(Metrics, worker_jobs_requested);
        self.set_execution_state(job_id, ExecutionStatus::Requested, job_hash, job_hash_len)
            .await
    }

    async fn skip_job(&self, job_id: Uuid, job_hash: Hash, job_hash_len: u64) -> Result<()> {
        debug!("skipping job {}", job_id);
        iroh_metrics::inc!(Metrics, worker_jobs_skipped);
        self.set_execution_state(job_id, ExecutionStatus::Skipped, job_hash, job_hash_len)
            .await
    }

    async fn execute_job(&self, job_id: Uuid, scheduled_job: ScheduledJob) -> Result<JobOutput> {
        info!("executing job {}", job_id);

        let author = self
            .repo
            .router()
            .authors()
            .export(scheduled_job.author)
            .await?
            .ok_or_else(|| anyhow!("author not found: {}", scheduled_job.author))?;

        let job_ctx = JobContext {
            author,
            id: job_id,
            environment: scheduled_job.description.environment.clone(),
            name: scheduled_job.description.name.clone(),
            name_context: JobNameContext {
                scope: scheduled_job.scope,
            },
            artifacts: scheduled_job.description.artifacts.clone(),
        };

        self.ensure_artifact_downloads(&job_ctx).await?;

        match &scheduled_job.description.details {
            JobDetails::Docker { image, command } => {
                let job = executor::docker::Job {
                    image: image.clone(),
                    command: command.clone(),
                };
                let res = self.executors.execute_docker(&job_ctx, job).await?;
                Ok(JobOutput::Docker {
                    code: res.code,
                    stderr: res.stderr,
                    stdout: res.stdout,
                })
            }
            JobDetails::Wasm { module } => {
                let job = executor::wasm::Job {
                    module: module.clone(),
                };
                let res = self.executors.execute_wasm(&job_ctx, job).await?;
                Ok(JobOutput::Wasm { output: res.output })
            }
        }
    }

    /// Ensures all required download artifcats are available locally.
    async fn ensure_artifact_downloads(&self, ctx: &JobContext) -> Result<()> {
        // Fetch required downloads

        let mut futures = Vec::new();
        for artifact in &ctx.artifacts.downloads {
            futures.push(async move {
                debug!("fetching {:?}", artifact);
                let name = ctx.name_context.render(&artifact.name)?;
                self.blobs.fetch_object(&name).await?;
                anyhow::Ok(())
            });
        }

        try_join_all(futures).await?;

        Ok(())
    }

    async fn mark_job_completed(
        &self,
        job_id: Uuid,
        job_hash: Hash,
        job_hash_len: u64,
    ) -> Result<()> {
        info!("job {} completed", job_id);
        iroh_metrics::inc!(Metrics, scheduler_jobs_completed);
        self.set_execution_state(job_id, ExecutionStatus::Completed, job_hash, job_hash_len)
            .await
    }

    async fn set_execution_state(
        &self,
        job_id: Uuid,
        status: ExecutionStatus,
        hash: Hash,
        len: u64,
    ) -> Result<()> {
        let key = Self::execution_status_key(job_id, status);
        self.set_hash_iff_new(key, hash, len).await?;
        Ok(())
    }

    pub async fn get_execution_status(&self, job_id: Uuid) -> Result<ExecutionStatus> {
        let mut status = ExecutionStatus::Unknown;
        let q = iroh::docs::store::Query::author(self.author_id)
            .key_prefix(Self::execution_status_prefix(job_id));
        let mut entries = self.doc.get_many(q).await?;
        while let Some(entry) = entries.next().await {
            let entry = entry?;
            let key = String::from_utf8(entry.key().to_vec())
                .map_err(|_| anyhow::anyhow!("Invalid UTF-8"))?;

            let read_status = Self::parse_execution_status(key)?;
            status = match (status, read_status) {
                (ExecutionStatus::Unknown, _) => read_status,
                (ExecutionStatus::Requested, ExecutionStatus::Running) => read_status,
                (ExecutionStatus::Requested, ExecutionStatus::Skipped) => read_status,
                (ExecutionStatus::Running, ExecutionStatus::Completed) => read_status,
                _ => status,
            }
        }

        Ok(status)
    }

    fn supports_job_type(&self, t: &JobType) -> bool {
        self.executors.supports_job_type(t)
    }

    fn execution_status_prefix(id: Uuid) -> String {
        format!("{}/status/{}/", WORKER_PREFIX, id.as_u128())
    }

    fn execution_status_key(id: Uuid, status: ExecutionStatus) -> String {
        format!("{}/status/{}/{}", WORKER_PREFIX, id.as_u128(), status,)
    }

    fn parse_execution_status(key: String) -> Result<ExecutionStatus> {
        let mut parts = key.splitn(4, '/').skip(3);

        let status = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing status component"))?;
        let status: ExecutionStatus = status.parse()?;

        Ok(status)
    }

    async fn handle_job_status_change(
        &self,
        job_hash: Hash,
        job_id: Uuid,
        job_len: u64,
    ) -> Result<()> {
        let scheduled_job = self.get_scheduled_job(job_hash).await?;
        debug!("{} job: {:?}", self.author_id.fmt_short(), scheduled_job);

        if self.is_enabled() && self.supports_job_type(&scheduled_job.job_type()) {
            self.request_job(job_id, job_hash, job_len).await?;
        }
        Ok(())
    }

    async fn get_scheduled_job(&self, job_hash: Hash) -> Result<ScheduledJob> {
        self.blobs.fetch_blob(job_hash).await?;
        let data = self.repo.router().blobs().read_to_bytes(job_hash).await?;
        let jd = ScheduledJob::try_from(data)?;
        Ok(jd)
    }

    async fn set_scheduled_job_result(
        &self,
        job_id: Uuid,
        job_hash: Hash,
        res: JobResult,
    ) -> Result<()> {
        // update job details
        let mut scheduled_job = self.get_scheduled_job(job_hash).await?;
        scheduled_job.result = res;

        let data = scheduled_job.to_bytes()?;
        let key = format!("{}/{}.json", JOBS_PREFIX, job_id.as_u128());
        let (new_hash, new_size) = self.blobs.put_bytes(key.as_str(), data).await?;

        self.mark_job_completed(job_id, new_hash, new_size).await?;

        Ok(())
    }

    async fn handle_job_assignment(
        &self,
        job_hash: Hash,
        job_id: Uuid,
        job_len: u64,
        from: AuthorId,
        worker: AuthorId,
    ) -> Result<()> {
        debug!(
            "scheduler {} job {} assigned to worker {}",
            from, job_id, worker,
        );

        let is_our_job = worker == self.author_id;
        let status = self.get_execution_status(job_id).await?;

        if !is_our_job {
            debug!("skipping job {}, not assigned to us", job_id);
            if status == ExecutionStatus::Requested {
                // no work for us :(
                self.skip_job(job_id, job_hash, job_len).await?;
            }
            return Ok(());
        }

        if !self.current_jobs.lock().await.insert(job_id) {
            debug!("skipping double event for {}", job_id);
            return Ok(());
        }
        struct Guard(Arc<Mutex<HashSet<Uuid>>>, Uuid);
        impl Drop for Guard {
            fn drop(&mut self) {
                let jobs = self.0.clone();
                let job_id = self.1;
                tokio::task::spawn(async move {
                    jobs.lock().await.remove(&job_id);
                    debug!("job guard: {} dropped", job_id);
                });
            }
        }

        let _guard = Guard(self.current_jobs.clone(), job_id);
        debug!("job guard: {} locked", job_id);

        // only execute job if we're in the requesting phase
        if is_our_job && status == ExecutionStatus::Requested {
            let self2 = self.clone();
            let node2 = self.repo.clone();

            iroh_metrics::inc!(Metrics, worker_jobs_running);
            let res = async {
                self.set_execution_state(job_id, ExecutionStatus::Running, job_hash, job_len)
                    .await?;

                let data = node2.router().blobs().read_to_bytes(job_hash).await?;
                let scheduled_job = ScheduledJob::try_from(data)?;
                let timeout: std::time::Duration = scheduled_job
                    .description
                    .timeout
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("invalid timeout"))?;

                let res =
                    tokio::time::timeout(timeout, self2.execute_job(job_id, scheduled_job)).await;

                match res {
                    Ok(Ok(output)) => anyhow::Ok(JobResultStatus::Ok(output)),
                    Ok(Err(err)) => {
                        error!("failed to execute job: {}", err);
                        Ok(JobResultStatus::Err(format!("{:#?}", err)))
                    }
                    Err(_) => {
                        error!("faile to execute job: timeout");
                        Ok(JobResultStatus::ErrTimeout)
                    }
                }
            };
            let res = match res.await {
                Ok(res) => res,
                Err(err) => {
                    error!("failed to execute job: {}", err);
                    JobResultStatus::Err(err.to_string())
                }
            };

            if let Err(err) = self2
                .set_scheduled_job_result(
                    job_id,
                    job_hash,
                    JobResult {
                        worker: Some(self2.author_id),
                        status: res,
                    },
                )
                .await
            {
                error!("unable to update job result: {:?}: {}", err, job_hash);
            }
        } else if is_our_job {
            error!(
                "worker {} ignoring job {} assigned to worker {}. we're in the {:?} phase, need to be in the requesting phase",
                self.author_id, job_id, worker, status,
            );
        }

        Ok(())
    }

    /// Returns `true` if an actual update has occured.
    async fn set_hash_iff_new(&self, key: impl Into<Bytes>, hash: Hash, size: u64) -> Result<bool> {
        let key: Bytes = key.into();
        match self.doc.get_exact(self.author_id, &key, true).await? {
            Some(entry) => {
                if entry.content_hash() == hash {
                    return Ok(false);
                }
            }
            None => {
                // No entry, lets set it
            }
        }
        self.doc.set_hash(self.author_id, key, hash, size).await?;
        Ok(true)
    }
}

impl DocEventHandler for Worker {
    async fn handle_event(&self, event: Event) -> Result<()> {
        if let EventData::Scheduler(se) = event.data {
            debug!("{} worker event: {:?}", self.author_id.fmt_short(), se);
            match se {
                SchedulerEvent::JobStatusChanged {
                    from,
                    job_id,
                    status,
                    job_hash,
                    job_len,
                } => {
                    debug!(
                        "scheduler {} job {} status changed to {}",
                        from, job_id, status,
                    );

                    let status = self.get_execution_status(job_id).await?;
                    if status == ExecutionStatus::Unknown {
                        let self2 = self.clone();
                        tokio::task::spawn(async move {
                            if let Err(err) = self2
                                .handle_job_status_change(job_hash, job_id, job_len)
                                .await
                            {
                                warn!("failed job handling: {:?}", err);
                            }
                        });
                    }
                }
                SchedulerEvent::JobAssigned {
                    from,
                    job_id,
                    worker,
                    job_hash,
                    job_len,
                } => {
                    let self2 = self.clone();
                    tokio::task::spawn(async move {
                        if let Err(err) = self2
                            .handle_job_assignment(job_hash, job_id, job_len, from, worker)
                            .await
                        {
                            warn!("failed job assignment: {:?}", err);
                        }
                    });
                }
            }
        }

        Ok(())
    }
}

#[derive(
    Copy,
    Debug,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    derive_more::Display,
    derive_more::FromStr,
)]
pub enum ExecutionStatus {
    Unknown,
    Requested,
    Skipped,
    Running,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkerEvent {
    ExecutionStatusChanged {
        worker: AuthorId,
        job_id: Uuid,
        status: ExecutionStatus,
        job_description_hash: Hash,
        job_description_length: u64,
    },
}

pub(crate) fn parse_worker_event(key: &str, from: &NodeId, entry: &Entry) -> Option<EventData> {
    match event_components(key) {
        Ok((job_id, status)) => Some(EventData::Worker(WorkerEvent::ExecutionStatusChanged {
            worker: AuthorId::from(from.as_bytes()),
            job_id,
            status,
            job_description_hash: entry.content_hash(),
            job_description_length: entry.content_len(),
        })),
        Err(e) => {
            error!("failed to parse worker event: {}", e);
            None
        }
    }
}

fn event_components(key: &str) -> Result<(Uuid, ExecutionStatus)> {
    let mut parts = key.splitn(4, '/').skip(2);

    let job_id = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing job_id component"))?;
    let job_id = job_id.parse().context("invalid job_id component")?;
    let job_id = Uuid::from_u128(job_id);

    let status = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing status component"))?;
    let status = status.parse()?;

    Ok((job_id, status))
}

use std::str::FromStr;

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use futures::StreamExt;
use iroh::blobs::Hash;
use iroh::client::docs::Entry;
use iroh::docs::AuthorId;
use iroh::net::NodeId;
use tracing::{debug, info, trace};
use uuid::Uuid;

use crate::router::RouterClient;

use super::blobs::Blobs;
use super::doc::{Doc, DocEventHandler, Event, EventData};
use super::job::{
    JobDescription, JobResult, JobResultStatus, JobStatus, ScheduledJob, JOBS_PREFIX,
};
use super::metrics::Metrics;
use super::worker::{ExecutionStatus, WorkerEvent};
use super::workspace::node_author_id;

#[derive(Clone, Debug)]
pub struct Scheduler {
    author_id: AuthorId, // author_id must be matched to the node_id doing the scheduling
    blobs: Blobs,
    node: RouterClient,
    doc: Doc,
    job_subscriptions: async_broadcast::Sender<(Uuid, JobStatus)>,
    job_r: async_broadcast::InactiveReceiver<(Uuid, JobStatus)>,
}

type ScheduledJobRef = (Hash, u64);

impl Scheduler {
    pub async fn new(
        author_id: AuthorId,
        doc: Doc,
        blobs: Blobs,
        node: RouterClient,
    ) -> Result<Self> {
        let (mut s, r) = async_broadcast::broadcast(128);
        s.set_await_active(false);

        let s = Self {
            author_id,
            doc,
            node,
            blobs,
            job_subscriptions: s,
            job_r: r.deactivate(),
        };
        Ok(s)
    }

    pub async fn run_job(
        &self,
        scope: Uuid,
        id: Uuid,
        job_description: JobDescription,
    ) -> Result<Uuid> {
        info!(
            "scheduling job: {} ({}) with scope {} by {}",
            job_description.name, id, scope, job_description.author
        );

        let author = AuthorId::from_str(&job_description.author.as_str())?;

        let scheduled_job = ScheduledJob {
            author,
            description: job_description,
            scope,
            result: JobResult::default(),
        };

        // phase 1 of 2 phase commit: write the job to the doc
        self.set_job_state(id, JobStatus::Scheduling, &scheduled_job)
            .await?;

        Ok(id)
    }

    pub async fn run_job_and_wait(
        &self,
        scope: Uuid,
        job_id: Uuid,
        job_description: JobDescription,
    ) -> Result<JobResult> {
        // subscribe before running, to make sure not events are missed
        let mut recv = self.subscribe_job_status_change();
        self.run_job(scope, job_id, job_description).await?;

        let mut worker_id = None;
        loop {
            let msg = recv.recv_direct().await;
            match msg {
                Err(async_broadcast::RecvError::Closed) => {
                    break;
                }
                Err(async_broadcast::RecvError::Overflowed(_)) => {}
                Ok((i, status)) => {
                    if job_id == i {
                        match status {
                            JobStatus::Scheduling => {}
                            JobStatus::Assigned(id) => {
                                worker_id.replace(id);
                            }
                            JobStatus::Canceled(id) => {
                                return Ok(JobResult {
                                    worker: worker_id,
                                    status: JobResultStatus::Err(format!("canceled: {:?}", id)),
                                });
                            }
                            JobStatus::Completed(id) => {
                                if let Some(worker_id) = worker_id {
                                    if id == worker_id {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let worker_id = worker_id.ok_or_else(|| anyhow::anyhow!("missing assigned worker id"))?;
        match self.get_job_result(job_id).await? {
            Some((JobStatus::Completed(id), result)) => {
                if id == worker_id {
                    iroh_metrics::inc!(Metrics, task_run_completed);
                    return Ok(result);
                }
                bail!(
                    "unexpected worker completed the job {}: {} != {}",
                    job_id,
                    worker_id,
                    id
                );
            }
            result => {
                bail!("failed to complete job: {:?}", result);
            }
        }
    }

    /// Cancel the given job.
    pub async fn cancel_job(&self, id: Uuid) -> Result<()> {
        info!("canceling job {}", id);

        let job = self.get_job(id).await?;
        match job {
            Some((JobStatus::Completed(_), _)) => {
                bail!("already completed");
            }
            Some((JobStatus::Canceled(_), _)) => {
                bail!("already canceled");
            }
            Some((JobStatus::Scheduling, job)) => {
                self.set_job_state(id, JobStatus::Canceled(None), &job)
                    .await?;
            }
            Some((JobStatus::Assigned(worker_id), job)) => {
                self.set_job_state(id, JobStatus::Canceled(Some(worker_id)), &job)
                    .await?;
            }
            None => {
                bail!("unknown job {}", id);
            }
        }
        Ok(())
    }

    async fn set_job_state(&self, id: Uuid, status: JobStatus, job: &ScheduledJob) -> Result<()> {
        let data = job.to_bytes()?;
        let key = format!("{}/{}.json", JOBS_PREFIX, id.as_u128());
        let (hash, size) = self.blobs.put_bytes(key.as_str(), data).await?;

        self.set_job_state_ref(id, status, (hash, size)).await?;
        Ok(())
    }

    async fn set_job_state_ref(
        &self,
        id: Uuid,
        status: JobStatus,
        (hash, size): ScheduledJobRef,
    ) -> Result<()> {
        match status {
            JobStatus::Scheduling => {
                iroh_metrics::inc!(Metrics, scheduler_jobs_requested);
            }
            JobStatus::Assigned(_) => {
                iroh_metrics::inc!(Metrics, scheduler_jobs_assigned);
            }
            JobStatus::Completed(_) => {
                iroh_metrics::inc!(Metrics, scheduler_jobs_completed);
            }
            JobStatus::Canceled(_) => {
                iroh_metrics::inc!(Metrics, scheduler_jobs_canceled);
            }
        }
        let key = job_status_key(id, status);
        self.set_hash_iff_new(key, hash, size).await?;

        Ok(())
    }

    // phase 2 of 2 phase commit
    async fn assign_job(
        &self,
        job_id: Uuid,
        worker_id: AuthorId,
        job_ref: ScheduledJobRef,
    ) -> Result<()> {
        info!("assigning job {} to {}", job_id, worker_id);
        let key = job_assignment_key(job_id, worker_id);
        // write the key that awards the job to worker_id
        self.set_hash_iff_new(key, job_ref.0, job_ref.1).await?;

        // advance job state (notifying any candidate workers)
        self.set_job_state_ref(job_id, JobStatus::Assigned(worker_id), job_ref)
            .await
    }

    async fn mark_job_completed(
        &self,
        job_id: Uuid,
        worker_id: AuthorId,
        job_ref: ScheduledJobRef,
    ) -> Result<()> {
        info!("job {} completed by {}", job_id, worker_id);
        self.set_job_state_ref(job_id, JobStatus::Completed(worker_id), job_ref)
            .await
    }

    /// Get the current scheduling status of a job on this node by id.
    ///
    /// If the job is not found, return `None`.
    pub async fn get_job_status(&self, job_id: Uuid) -> Result<Option<JobStatus>> {
        let res = self.get_job_result(job_id).await?;
        Ok(res.map(|(s, _)| s))
    }

    /// Get the current scheduling status and result of a job on this node by id.
    /// If the job is not found, return `None`.
    pub async fn get_job_result(&self, job_id: Uuid) -> Result<Option<(JobStatus, JobResult)>> {
        let res = self.get_job(job_id).await?;
        Ok(res.map(|(s, j)| (s, j.result)))
    }

    /// Get the current scheduling status and result of a job on this node by id.
    ///
    /// If the job is not found, return `None`.
    pub async fn get_job(&self, job_id: Uuid) -> Result<Option<(JobStatus, ScheduledJob)>> {
        let job_id = job_id.as_u128();
        let mut status: Option<(JobStatus, Hash)> = None;
        let q = iroh::docs::store::Query::author(self.author_id)
            .key_prefix(format!("{}/status/{}/", JOBS_PREFIX, job_id));
        let mut entries = self.doc.get_many(q).await?;

        while let Some(entry) = entries.next().await {
            let entry = entry?;
            let key = std::str::from_utf8(entry.key())?;

            trace!("checking status: {}", key);
            let (_, read_status) = parse_status(key)?;

            match status {
                Some((ref mut s, ref mut r)) => {
                    if s.merge(read_status) {
                        *r = entry.content_hash();
                    }
                }
                None => {
                    status.replace((read_status, entry.content_hash()));
                }
            }
        }

        match status {
            Some((status, job_hash)) => {
                let job = self.get_scheduled_job(job_hash).await?;
                Ok(Some((status, job)))
            }
            None => Ok(None),
        }
    }

    async fn get_scheduled_job(&self, job_hash: Hash) -> Result<ScheduledJob> {
        self.blobs.fetch_blob(job_hash).await?;
        let data = self.node.blobs().read_to_bytes(job_hash).await?;
        let jd = ScheduledJob::try_from(data)?;
        Ok(jd)
    }

    pub fn subscribe_job_status_change(&self) -> async_broadcast::Receiver<(Uuid, JobStatus)> {
        self.job_r.activate_cloned()
    }

    async fn handle_worker_execution_status_change(
        &self,
        job_id: Uuid,
        worker: AuthorId,
        status: ExecutionStatus,
        job_ref: ScheduledJobRef,
    ) -> Result<()> {
        match self.get_job_status(job_id).await? {
            Some(JobStatus::Scheduling) => {
                if status == ExecutionStatus::Requested {
                    self.assign_job(job_id, worker, job_ref).await?;
                }
            }
            Some(JobStatus::Assigned(worker_id)) => {
                if status == ExecutionStatus::Completed && worker == worker_id {
                    self.mark_job_completed(job_id, worker, job_ref).await?;
                }
            }
            _ => {}
        };

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

fn job_status_key(id: Uuid, status: JobStatus) -> String {
    format!("{}/status/{}/{}", JOBS_PREFIX, id.as_u128(), status)
}

fn job_assignment_key(id: Uuid, author_id: AuthorId) -> String {
    format!("{}/assign/{}/{}", JOBS_PREFIX, id.as_u128(), author_id)
}

impl DocEventHandler for Scheduler {
    async fn handle_event(&self, event: Event) -> Result<()> {
        debug!(
            "{} scheduler event: {:?}",
            self.author_id.fmt_short(),
            event
        );
        match event.data {
            EventData::Worker(WorkerEvent::ExecutionStatusChanged {
                worker,
                job_id,
                status,
                job_description_hash,
                job_description_length,
            }) => {
                self.handle_worker_execution_status_change(
                    job_id,
                    worker,
                    status,
                    (job_description_hash, job_description_length),
                )
                .await?;
                Ok(())
            }
            EventData::Scheduler(SchedulerEvent::JobStatusChanged { job_id, status, .. }) => {
                let res = self
                    .job_subscriptions
                    .broadcast_direct((job_id, status))
                    .await?;
                debug!("sending {}: {}: {:?}", job_id, status, res);
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerEvent {
    JobStatusChanged {
        from: AuthorId,    // node doing the scheduling
        job_id: Uuid,      // unique id of the job
        status: JobStatus, // updated status of the job
        job_hash: Hash,    // hash of the job description
        job_len: u64,      // length of the job description
    },
    JobAssigned {
        from: AuthorId,   // node doing the scheduling
        job_id: Uuid,     // unique id of the job
        worker: AuthorId, // node assigned the work
        job_hash: Hash,   // hash of the job description
        job_len: u64,     // length of the job description
    },
}

pub(crate) fn parse_scheduler_event(key: &str, from: &NodeId, entry: &Entry) -> Option<EventData> {
    parse_event(key, from, entry)
}

fn parse_event(key: &str, from: &NodeId, entry: &Entry) -> Option<EventData> {
    if key.starts_with(&format!("{}/status", JOBS_PREFIX)) {
        match parse_status(key) {
            Ok((job_id, status)) => Some(EventData::Scheduler(SchedulerEvent::JobStatusChanged {
                from: node_author_id(from),
                job_id,
                status,
                job_hash: entry.content_hash(),
                job_len: entry.content_len(),
            })),
            Err(e) => {
                tracing::error!("failed to parse scheduler event: {}", e);
                None
            }
        }
    } else if key.starts_with(&format!("{}/assign", JOBS_PREFIX)) {
        match parse_assignment_event(key) {
            Ok((job_id, author_id)) => Some(EventData::Scheduler(SchedulerEvent::JobAssigned {
                from: node_author_id(from),
                job_id,
                worker: author_id,
                job_hash: entry.content_hash(),
                job_len: entry.content_len(),
            })),
            Err(e) => {
                tracing::error!("failed to parse scheduler event: {}", e);
                None
            }
        }
    } else {
        None
    }
}

pub(crate) fn parse_status(key: &str) -> Result<(Uuid, JobStatus)> {
    let mut parts = key.splitn(4, '/').skip(2); // lop off JOBS_PREFIX, status

    let job_id = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing job_id component"))?;
    let job_id = job_id.parse()?;
    let job_id = Uuid::from_u128(job_id);

    let status_str = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing status component"))?;
    let status = status_str.parse()?;

    Ok((job_id, status))
}

fn parse_assignment_event(key: &str) -> Result<(Uuid, AuthorId)> {
    let mut parts = key.splitn(4, '/').skip(2);

    let job_id = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing job_id component"))?;
    let job_id = job_id.parse().context("invalid job_id component")?;
    let job_id = Uuid::from_u128(job_id);

    let author_id = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing author_id component"))?;
    let author_id = author_id.parse().context("invalid author_id component")?;

    Ok((job_id, author_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::{Context, Result};

    use crate::vm::job::{Artifact, Artifacts, JobDetails, JobOutput, DEFAULT_TIMEOUT};
    use crate::vm::test_utils::{create_nodes, setup_logging};

    #[tokio::test]
    async fn test_work_schedule_assign() -> Result<()> {
        setup_logging();

        let dir = tempfile::tempdir().context("tempdir")?;

        let nodes = create_nodes(&dir, 2).await.unwrap();

        let scope = Uuid::new_v4();
        let job_id = Uuid::new_v4();

        // manually add the `min.wat` file
        let file = tokio::fs::read("tests/min.wat").await?;
        let name = format!("{}/min.wat", scope.as_simple());
        let res = nodes[0].0.blobs().add_bytes(file).await?;
        nodes[0]
            .1
            .blobs()
            .put_object(&name, res.hash, res.size)
            .await?;

        // Disable the worker on the scheduler to force the second node to grab the work
        nodes[0].1.worker().disable();

        let job_result = nodes[0]
            .1
            .scheduler()
            .run_job_and_wait(
                scope,
                job_id,
                JobDescription {
                    name: String::from("sleep for 10 milliseconds"),
                    details: JobDetails::Wasm {
                        module: "min.wat".into(),
                    },
                    artifacts: Artifacts {
                        downloads: [Artifact {
                            name: "{scope}/min.wat".into(),
                            path: "min.wat".into(),
                            executable: false,
                        }]
                        .into_iter()
                        .collect(),
                        uploads: Default::default(),
                    },
                    timeout: DEFAULT_TIMEOUT,
                },
            )
            .await?;
        assert!(
            matches!(job_result.status, JobResultStatus::Ok(_)),
            "{:#?}",
            job_result
        );
        let status = nodes[0].1.scheduler().get_job_status(job_id).await?;
        assert_eq!(
            status.unwrap(),
            JobStatus::Completed(node_author_id(&nodes[1].0.node_id()))
        );

        let status = nodes[1].1.worker().get_execution_status(job_id).await?;
        assert_eq!(status, ExecutionStatus::Completed);

        Ok(())
    }

    #[tokio::test]
    async fn test_1_scheduler_5_workers() -> Result<()> {
        setup_logging();

        let temp_dir = tempfile::tempdir().context("tempdir")?;
        let nodes = create_nodes(&temp_dir, 5).await?;

        let (node, ws) = &nodes[0];

        let scope = Uuid::new_v4();

        // manually add the `min.wat` file
        let file = tokio::fs::read("tests/min.wat").await?;
        let name = format!("{}/min.wat", scope.as_simple());
        let res = node.blobs().add_bytes(file).await?;
        ws.blobs().put_object(&name, res.hash, res.size).await?;

        // Disable the worker on the scheduler to force the another node to grab the work
        nodes[0].1.worker().disable();

        let job_id = Uuid::new_v4();
        let job_result = ws
            .scheduler()
            .run_job_and_wait(
                scope,
                job_id,
                JobDescription {
                    name: "hello".into(),
                    details: JobDetails::Wasm {
                        module: "min.wat".into(),
                    },
                    artifacts: Artifacts {
                        downloads: [Artifact {
                            name: "{scope}/min.wat".into(),
                            path: "min.wat".into(),
                            executable: false,
                        }]
                        .into_iter()
                        .collect(),
                        uploads: Default::default(),
                    },
                    timeout: DEFAULT_TIMEOUT,
                },
            )
            .await?;

        assert_eq!(
            job_result.status,
            JobResultStatus::Ok(JobOutput::Wasm {
                output: "hello world\n".into(),
            })
        );

        let job_status = ws.scheduler().get_job_status(job_id).await?;
        let Some(JobStatus::Completed(worker_id)) = job_status else {
            panic!("unexpected job status: {:?}", job_status);
        };

        // Checking workers
        for (i, (node, ws)) in nodes.iter().skip(1).enumerate() {
            info!(
                "checking workspace: {}: {}/{}",
                node.node_id(),
                i + 1,
                nodes.len() - 1
            );
            let status = ws.worker().get_execution_status(job_id).await?;
            let id = node_author_id(&node.node_id());
            if id == worker_id {
                assert_eq!(status, ExecutionStatus::Completed);
            } else {
                assert!(matches!(
                    status,
                    ExecutionStatus::Skipped | ExecutionStatus::Requested
                ));
            }
        }

        Ok(())
    }
}

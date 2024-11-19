use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use flow::{Flow, Task, TaskOutput};
use futures::StreamExt;
use iroh::base::node_addr::AddrInfoOptions;
use iroh::client::docs::ShareMode;
use iroh::docs::{Author, AuthorId, DocTicket, NamespaceId};
use iroh::net::NodeId;
use job::{Artifacts, DEFAULT_TIMEOUT};
use tokio::task::JoinHandle;
use tracing::{debug, info, info_span, warn, Instrument};
use uuid::Uuid;

use crate::router::RouterClient;

use crate::space::{Space, Spaces};
use crate::vm::blobs::Blobs;
use crate::vm::content_routing::AutofetchPolicy;
use crate::vm::doc::{create_doc, join_doc, subscribe, Doc, DocEventHandler};
use crate::vm::job::JobDescription;
use crate::vm::metrics::Metrics;
use crate::vm::scheduler::Scheduler;
use crate::vm::worker::Worker;

mod blobs;
mod config;
pub mod content_routing;
mod doc;
mod docker;
pub mod flow;
mod job;
mod metrics;
mod scheduler;
mod worker;

#[derive(Debug)]
pub struct VM {
    router: RouterClient,
    doc: Doc,
    blobs: Blobs,
    scheduler: Scheduler,
    worker: Worker,
    /// Tracks the subscription task, canceling it when the vm gets dropped.
    _doc_subscription_handle: JoinHandle<()>,
}

impl VM {
    pub async fn create(spaces: Spaces, router: &RouterClient, cfg: VMConfig) -> Result<Self> {
        let doc = create_doc(&router.clone()).await?;
        Self::open(spaces, router, doc, cfg).await
    }

    pub async fn join(
        spaces: Spaces,
        router: &RouterClient,
        ticket: DocTicket,
        cfg: VMConfig,
    ) -> Result<Self> {
        debug!("joining {}", ticket);
        let doc = join_doc(router, ticket).await?;
        Self::open(spaces, router, doc, cfg).await
    }

    pub async fn open(
        spaces: Spaces,
        router: &RouterClient,
        doc: Doc,
        cfg: VMConfig,
    ) -> Result<Self> {
        let node_id = router.net().node_id().await?;
        let blobs = Blobs::new(node_id, doc.clone(), router.clone(), cfg.autofetch);
        let author_id = node_author_id(&node_id);
        let scheduler =
            Scheduler::new(author_id, doc.clone(), blobs.clone(), router.clone()).await?;
        let worker = Worker::new(
            spaces,
            router.clone(),
            author_id,
            doc.clone(),
            blobs.clone(),
            &cfg.worker_root,
        )
        .await?;

        let events = subscribe(&doc, node_id).await?;
        let scheduler2 = scheduler.clone();
        let worker2 = worker.clone();
        let blobs2 = blobs.clone();

        let handle = tokio::task::spawn(
            async move {
                let mut events = std::pin::pin!(events);
                while let Some(event) = events.next().await {
                    if let Err(err) = scheduler2.handle_event(event.clone()).await {
                        warn!("scheduler failed to handle event: {:?}", err);
                    }
                    if let Err(err) = worker2.handle_event(event.clone()).await {
                        warn!("worker failed to handle event: {:?}", err);
                    }
                    if let Err(err) = blobs2.handle_event(event).await {
                        warn!("blobs failed to handle event: {:?}", err);
                    }
                }

                debug!("exiting event handling");
            }
            .instrument(info_span!("workspace_eventsub", %node_id)),
        );

        let ws = Self {
            router: router.clone(),
            doc,
            blobs,
            scheduler,
            worker,
            _doc_subscription_handle: handle.into(),
        };

        iroh_metrics::inc!(Metrics, workspaces);
        info!(
            "opened workspace. write ticket: {}",
            ws.get_write_ticket(Default::default()).await?.to_string()
        );
        Ok(ws)
    }

    pub fn id(&self) -> NamespaceId {
        self.doc.id()
    }

    pub async fn get_write_ticket(&self, opts: AddrInfoOptions) -> Result<DocTicket> {
        self.doc.share(ShareMode::Write, opts).await
    }

    pub fn blobs(&self) -> &Blobs {
        &self.blobs
    }

    pub fn scheduler(&self) -> &Scheduler {
        &self.scheduler
    }

    pub fn worker(&self) -> &Worker {
        &self.worker
    }

    // pub async fn run_job(&self, scope: Uuid, id: Uuid, jd: JobDescription) -> Result<Uuid> {
    //     let id = self.scheduler.run_job(scope, id, jd).await?;
    //     Ok(id)
    // }

    // pub async fn run_job_and_wait(
    //     &self,
    //     scope: Uuid,
    //     id: Uuid,
    //     jd: JobDescription,
    // ) -> Result<JobResult> {
    //     let result = self.scheduler.run_job_and_wait(scope, id, jd).await?;
    //     Ok(result)
    // }

    pub async fn run_program(
        &self,
        space: &Space,
        author: Author,
        id: Uuid,
        environment: HashMap<String, String>,
    ) -> Result<TaskOutput> {
        let program = space.programs().get_by_id(id).await?;
        let program_entry_hash = program.program_entry.context("program has no main entry")?;
        // construct a task so we can schedule it with the VM
        let result = Flow {
            name: program.manifest.name.clone(),
            tasks: vec![Task {
                tasks: vec![],
                description: JobDescription {
                    space: space.name.clone(),
                    name: program.manifest.name.clone(),
                    author: author.id().to_string(),
                    environment,
                    details: job::JobDetails::Wasm {
                        module: job::Source::LocalBlob(program_entry_hash),
                    },
                    artifacts: Artifacts::default(),
                    timeout: DEFAULT_TIMEOUT,
                },
            }],
            uploads: Default::default(),
            downloads: Default::default(),
        }
        .run(&self)
        .await?;
        let output = result.tasks.first().expect("single task").clone();
        Ok(output)
    }
}

pub struct VMConfig {
    pub autofetch: AutofetchPolicy,
    pub worker_root: PathBuf,
}

pub(crate) fn node_author_id(node_id: &NodeId) -> AuthorId {
    AuthorId::from(node_id.as_bytes())
}

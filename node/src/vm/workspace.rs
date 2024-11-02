use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use futures::StreamExt;
use iroh::base::node_addr::AddrInfoOptions;
use iroh::client::docs::ShareMode;
use iroh::docs::{store::Query, AuthorId, DocTicket, NamespaceId};
use iroh::net::NodeId;
use tokio::sync::{RwLock, RwLockMappedWriteGuard, RwLockReadGuard, RwLockWriteGuard};
use tokio::task::JoinHandle;
use tracing::{debug, info, info_span, warn, Instrument};
use uuid::Uuid;

use crate::router::{Router, RouterClient};

use super::blobs::Blobs;
use super::config::NodeConfig;
use super::content_routing::AutofetchPolicy;
use super::doc::{create_doc, join_doc, open_doc, subscribe, Doc, DocEventHandler};
use super::job::{JobDescription, JobResult};
use super::metrics::Metrics;
use super::scheduler::Scheduler;
use super::worker::Worker;

const WORKSPACES_FILE_NAME: &str = "workspaces.json";
const WORKSPACE_NAME_KEY: &str = "workspace";

#[derive(Debug)]
pub struct Workspace {
    name: String,
    doc: Doc,
    blobs: Blobs,
    scheduler: Scheduler,
    worker: Worker,
    /// Tracks the subscription task, canceling it when the workspace gets dropped.
    _doc_subscription_handle: JoinHandle<()>,
}

impl Workspace {
    pub async fn create(
        name: String,
        node_id: NodeId,
        node: &RouterClient,
        cfg: WorkspaceConfig,
    ) -> Result<Self> {
        let doc = create_doc(node).await?;
        let author_id = node_author_id(&node_id);

        // TODO: use values again when they are inlined
        let key = format!("{}/{}", WORKSPACE_NAME_KEY, name);
        doc.set_bytes(author_id, key, name.clone()).await?;
        load_name(&doc).await.context("just set?")?;

        Self::open(name, node_id, node, doc, cfg).await
    }

    pub async fn join(
        node_id: NodeId,
        node: &RouterClient,
        ticket: DocTicket,
        cfg: WorkspaceConfig,
    ) -> Result<Self> {
        debug!("joining {}", ticket);
        let doc = join_doc(node, ticket).await?;
        let name = load_name(&doc).await?;
        Self::open(name, node_id, node, doc, cfg).await
    }

    pub async fn open(
        name: String,
        node_id: NodeId,
        node: &RouterClient,
        doc: Doc,
        cfg: WorkspaceConfig,
    ) -> Result<Self> {
        let blobs = Blobs::new(node_id, doc.clone(), node.clone(), cfg.autofetch);
        let author_id = node_author_id(&node_id);
        let scheduler = Scheduler::new(author_id, doc.clone(), blobs.clone(), node.clone()).await?;
        let worker = Worker::new(
            author_id,
            doc.clone(),
            blobs.clone(),
            node.clone(),
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
            name: name.clone(),
            doc,
            blobs,
            scheduler,
            worker,
            _doc_subscription_handle: handle.into(),
        };

        iroh_metrics::inc!(Metrics, workspaces);
        info!(
            "opened workspace {:?} write ticket: {}",
            &name,
            ws.get_write_ticket(Default::default()).await?.to_string()
        );
        Ok(ws)
    }

    pub fn id(&self) -> NamespaceId {
        self.doc.id()
    }

    pub fn name(&self) -> &str {
        &self.name
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

    pub async fn run_job(&self, scope: Uuid, id: Uuid, jd: JobDescription) -> Result<Uuid> {
        let id = self.scheduler.run_job(scope, id, jd).await?;

        Ok(id)
    }

    pub async fn run_job_and_wait(
        &self,
        scope: Uuid,
        id: Uuid,
        jd: JobDescription,
    ) -> Result<JobResult> {
        let result = self.scheduler.run_job_and_wait(scope, id, jd).await?;

        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub struct Workspaces {
    node: RouterClient,
    path: PathBuf,
    inner: Arc<RwLock<HashMap<String, Workspace>>>,
    cfg: Arc<NodeConfig>,
}

impl Workspaces {
    pub fn node(&self) -> &RouterClient {
        &self.node
    }

    pub async fn list(&self) -> Vec<String> {
        self.inner.read().await.keys().cloned().collect()
    }

    pub async fn get(&self, name: &str) -> Option<RwLockReadGuard<'_, Workspace>> {
        let guard = RwLockReadGuard::try_map(self.inner.read().await, |l| l.get(name)).ok()?;

        Some(guard)
    }

    pub async fn get_mut(&self, name: &str) -> Option<RwLockMappedWriteGuard<'_, Workspace>> {
        let guard =
            RwLockWriteGuard::try_map(self.inner.write().await, |l| l.get_mut(name)).ok()?;

        Some(guard)
    }

    pub async fn contains(&self, name: &str) -> bool {
        self.inner.read().await.contains_key(name)
    }

    pub async fn load_or_create(
        node: RouterClient,
        data_dir: impl Into<PathBuf>,
        cfg: NodeConfig,
    ) -> Result<Self> {
        let path = data_dir.into().join(WORKSPACES_FILE_NAME);
        if !path.exists() {
            return Ok(Self {
                node,
                path: path.clone(),
                inner: Default::default(),
                cfg: Arc::new(cfg),
            });
        }

        let file = std::fs::File::open(&path)?;
        let workspace_list: Vec<(String, NamespaceId)> = serde_json::from_reader(file)?;
        let workspaces = workspace_list.into_iter().map(|(_, id)| id).collect();
        let workspaces = open_workspaces(node.clone(), workspaces, &cfg).await?;

        Ok(Self {
            node,
            path,
            inner: Arc::new(RwLock::new(workspaces)),
            cfg: Arc::new(cfg),
        })
    }

    pub async fn create(&self, name: &str) -> Result<()> {
        let router_id = self.node.net().node_id().await?;
        let ws = Workspace::create(
            name.to_string(),
            router_id,
            &self.node.clone(),
            self.cfg.workspace_config(),
        )
        .await?;
        self.inner.write().await.insert(name.to_string(), ws);
        self.write_to_file().await?;
        Ok(())
    }

    pub async fn join_workspace(&self, ticket: DocTicket) -> Result<()> {
        debug!("joining workspace, ticket: {:?}", ticket);

        let doc = join_doc(&self.node, ticket).await?;
        let name = load_name(&doc).await?;
        let router_id = self.node.net().node_id().await?;
        if !self.contains(&name).await {
            let ws = Workspace::open(
                name,
                router_id,
                &self.node,
                doc,
                self.cfg.workspace_config(),
            )
            .await?;
            self.inner.write().await.insert(ws.name.clone(), ws);
            self.write_to_file().await?;
        } else {
            debug!("workspace already open: {}", name);
        }
        Ok(())
    }

    async fn write_to_file(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let workspaces = inner
            .iter()
            .map(|(k, v)| (k, v.doc.id()))
            .collect::<Vec<_>>();

        let serialized = serde_json::to_string(&workspaces)?;
        tokio::fs::write(&self.path, serialized.as_bytes()).await?;
        Ok(())
    }
}

async fn open_workspaces(
    node: RouterClient,
    workspaces: Vec<NamespaceId>,
    cfg: &NodeConfig,
) -> Result<HashMap<String, Workspace>> {
    let router_id = node.net().node_id().await?;
    let mut ans = HashMap::new();
    for namespace_id in workspaces {
        let ws = open_workspace(
            namespace_id,
            router_id,
            &node.clone(),
            cfg.workspace_config(),
        )
        .await;
        let ws = match ws {
            Ok(doc) => doc,
            Err(err) => {
                warn!("workspace {:?} not found: {:?}", namespace_id, err);
                continue;
            }
        };
        ans.insert(ws.name.clone(), ws);
    }
    Ok(ans)
}

/// Gets the name from the underlying document.
pub async fn load_name(doc: &Doc) -> Result<String> {
    let entry = doc
        .get_one(
            Query::single_latest_per_key()
                .key_prefix(format!("{}/", WORKSPACE_NAME_KEY))
                .build(),
        )
        .await?
        .ok_or_else(|| anyhow::anyhow!("invalid document, missing name"))?;
    let key = std::str::from_utf8(entry.key())?;
    let name = parse_name(key)?;
    Ok(name.to_string())
}

pub(crate) fn parse_name(key: &str) -> Result<&str> {
    let parts: Vec<_> = key.split('/').collect();
    ensure!(parts.len() == 2, "invalid key");
    let prefix = parts[0];
    ensure!(prefix == WORKSPACE_NAME_KEY, "invalid prefix: {}", prefix);

    let name = parts[1];
    Ok(name)
}

async fn open_workspace(
    namespace_id: NamespaceId,
    node_id: NodeId,
    node: &RouterClient,
    cfg: WorkspaceConfig,
) -> Result<Workspace> {
    let doc = open_doc(node, namespace_id).await?;
    let workspace_name = load_name(&doc).await?;
    Workspace::open(workspace_name, node_id, node, doc, cfg).await
}

pub struct WorkspaceConfig {
    pub autofetch: AutofetchPolicy,
    pub worker_root: PathBuf,
}

pub(crate) fn node_author_id(node_id: &NodeId) -> AuthorId {
    AuthorId::from(node_id.as_bytes())
}

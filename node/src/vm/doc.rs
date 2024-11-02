use anyhow::{anyhow, bail, Result};
use futures::stream::{Stream, StreamExt};
use iroh::client::docs::Entry;
use iroh::docs::{DocTicket, NamespaceId};
use iroh::net::NodeId;
use tracing::{trace, warn};

use super::blobs::{parse_blobs_event, BlobsEvent, BLOBS_DOC_PREFIX};
use super::content_routing::{
    parse_content_routing_event, ContentRoutingEvent, CONTENT_ROUTING_PREFIX,
};
use super::job::JOBS_PREFIX;
use super::node::IrohNodeClient;
use super::scheduler::{parse_scheduler_event, SchedulerEvent};
use super::worker::{parse_worker_event, WorkerEvent, WORKER_PREFIX};

pub use iroh::client::Doc;

// A sentinel value for when the value of a key is not important. use it with doc.set_bytes
// Can't use the empty hash because it stands for deletion
pub(crate) const EMPTY_OK_VALUE: &[u8] = b"ok";

const DEFAULT_POLICY: iroh::docs::store::DownloadPolicy =
    iroh::docs::store::DownloadPolicy::NothingExcept(vec![]);

pub async fn create_doc(node: &IrohNodeClient) -> Result<Doc> {
    let doc = node.docs().create().await?;
    configure_doc(&doc).await?;
    Ok(doc)
}

pub async fn join_doc(node: &IrohNodeClient, ticket: DocTicket) -> Result<Doc> {
    let doc = node.docs().import(ticket).await?;
    wait_for_sync_finished(&doc).await?;
    configure_doc(&doc).await?;
    Ok(doc)
}

async fn wait_for_sync_finished(doc: &Doc) -> Result<()> {
    trace!("wait for sync to finish");
    let mut stream = doc.subscribe().await?;
    while let Some(event) = stream.next().await {
        trace!("doc event: {:?}", event);
        let event = event.unwrap();
        if let iroh::client::docs::LiveEvent::SyncFinished(event) = event {
            event
                .result
                .map_err(|e| anyhow::anyhow!("sync error: {}", e))?;
            return Ok(());
        }
    }
    bail!("sync finished event not found")
}

pub async fn open_doc(node: &IrohNodeClient, namespace_id: NamespaceId) -> Result<Doc> {
    let doc = node
        .docs()
        .open(namespace_id)
        .await?
        .ok_or_else(|| anyhow!("doc is not found"))?;

    Ok(doc)
}

async fn configure_doc(doc: &Doc) -> Result<()> {
    doc.set_download_policy(DEFAULT_POLICY).await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct Event {
    #[allow(dead_code)]
    pub(crate) entry: Entry,
    pub(crate) data: EventData,
}

#[derive(Debug, Clone)]
pub(crate) enum EventData {
    Blobs(BlobsEvent),
    ContentRouting(ContentRoutingEvent),
    Scheduler(SchedulerEvent),
    Worker(WorkerEvent),
}

pub(crate) trait DocEventHandler {
    async fn handle_event(&self, event: Event) -> Result<()>;
}

fn parse_key(key: &[u8]) -> Option<(&str, &str)> {
    let key = std::str::from_utf8(key).ok()?;
    let demux = key.split('/').next()?;
    Some((key, demux))
}

pub(crate) async fn subscribe(doc: &Doc, node_id: NodeId) -> Result<impl Stream<Item = Event>> {
    let stream = doc.subscribe().await?;
    let stream = stream.filter_map(move |event| async move {
        tracing::info!("doc event ({}): {:?}", node_id, event);
        match event {
            Ok(event) => {
                let (from, entry) = match event {
                    iroh::client::docs::LiveEvent::InsertRemote {
                        ref entry, from, ..
                    } => (from, entry),
                    iroh::client::docs::LiveEvent::InsertLocal { ref entry } => (node_id, entry),
                    _ => return None,
                };

                parse_key(entry.key())
                    .and_then(|(key, demux)| match demux {
                        JOBS_PREFIX => parse_scheduler_event(key, &from, entry),
                        WORKER_PREFIX => parse_worker_event(key, &from, entry),
                        BLOBS_DOC_PREFIX => parse_blobs_event(key),
                        CONTENT_ROUTING_PREFIX => parse_content_routing_event(key),
                        _ => None,
                    })
                    .map(|data| Event {
                        entry: entry.clone(),
                        data,
                    })
            }
            Err(err) => {
                warn!("error: {:?}", err);
                None
            }
        }
    });

    Ok(stream)
}

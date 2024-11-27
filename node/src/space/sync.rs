use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use futures::{Sink, SinkExt, StreamExt};
use iroh::docs::NamespaceId;
use iroh::gossip::net::Command;
use tokio::sync::Mutex;

use crate::router::RouterClient;

use super::events::Event;
use super::users::all_user_node_ids;
use super::DB;

struct Inner {
    sink: Pin<Box<dyn Sink<Command, Error = anyhow::Error> + Send>>,
    sink_task: tokio::task::JoinHandle<()>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.sink_task.abort();
        tracing::info!("dropping broadcast");
    }
}

impl Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Broadcast").finish()
    }
}

/// Sync ensures events and blobs are propagated to nodes that have at least some read capability
/// in the target space. Sync follows two main strategies:
/// 1. Reconcile: whenever a node comes online, it performs a full reconciliation with it's
///    bootstrap nodes. This "catches up" the node with the latest state of the space.
/// 2. Update: Nodes writing new events broadcast to all nodes that have matching credentials to
///    read the event. This ensures granular liveness
#[derive(Debug, Clone)]
pub struct Sync {
    inner: Arc<Mutex<Inner>>,
}

impl Sync {
    pub async fn start(db: &DB, router: &RouterClient, topic: NamespaceId) -> Result<Self> {
        let bootstrap = all_user_node_ids(db, router).await?;
        let (sync, mut stream) = router.gossip().subscribe(topic, bootstrap.clone()).await?;

        let sink_task = tokio::task::spawn(async move {
            while let Some(event) = stream.next().await {
                let event = event
                    .map_err(|e| tracing::error!("gossip error: {:?}", e))
                    .ok();
                if let Some(event) = event {
                    match event {
                        iroh::gossip::net::Event::Gossip(event) => match event {
                            iroh::gossip::net::GossipEvent::NeighborUp(peer) => {
                                tracing::info!("joined {:?}", peer)
                            }
                            iroh::gossip::net::GossipEvent::NeighborDown(peer) => {
                                tracing::info!("left {:?}", peer)
                            }
                            iroh::gossip::net::GossipEvent::Received(message) => {
                                tracing::info!("message {:?}", message)
                            }
                            iroh::gossip::net::GossipEvent::Joined(peers) => {
                                tracing::info!("joined {:?}", peers)
                            }
                        },
                        iroh::gossip::net::Event::Lagged => {
                            tracing::warn!("gossip lagged")
                        }
                    }
                }
            }
        });

        let inner = Inner {
            sink: Box::pin(sync),
            sink_task,
        };

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    pub async fn broadcast_event_update(&self, event: Event) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let bytes = serde_json::to_vec(&event)?;
        let command = Command::BroadcastNeighbors(bytes.into());
        inner.sink.send(command);
        Ok(())
    }
}

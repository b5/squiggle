use std::str::FromStr;

use anyhow::{Context, Result};
use futures::StreamExt;
use iroh::{NodeAddr, NodeId};
use iroh_blobs::Hash;
use iroh_docs::store::Query;
use iroh_docs::AuthorId;
use serde::{Deserialize, Serialize};
use tracing::trace;

use crate::iroh::Protocols;

use super::doc::{Doc, Event, EventData, EMPTY_OK_VALUE};
use super::metrics::Metrics;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutofetchPolicy {
    /// don't fetch the data from the remote source unless explicitly asked to via API calls
    Disabled,
    /// fetch all content from the document store it
    All,
}

impl Default for AutofetchPolicy {
    fn default() -> Self {
        Self::Disabled
    }
}

pub(crate) const CONTENT_ROUTING_PREFIX: &str = "providers";

#[derive(Debug, Clone)]
pub(crate) struct ContentRouter {
    author_id: AuthorId,
    node_id: NodeId,
    doc: Doc,
    node: Protocols,
    autofetch: AutofetchPolicy,
}

impl ContentRouter {
    pub(crate) fn new(
        author_id: AuthorId,
        node_id: NodeId,
        doc: Doc,
        node: Protocols,
        autofetch: AutofetchPolicy,
    ) -> Self {
        Self {
            author_id,
            node_id,
            doc,
            node,
            autofetch,
        }
    }

    pub(crate) async fn fetch_blob(&self, hash: Hash) -> Result<()> {
        let provs = self.find_providers(hash).await?;
        if provs.contains(&self.node_id) {
            // Nothing to do, we have it ourselves
            trace!("local provider found for {}", hash);
            return Ok(());
        }

        if provs.is_empty() {
            return Err(anyhow::anyhow!("No providers found for hash {}", hash));
        }

        trace!(
            "Found {} providers for hash {}: {:?}",
            provs.len(),
            hash,
            provs
        );

        for prov in provs {
            match fetch_blob_from_provider(&self.node, hash, prov).await {
                Ok(_) => {
                    iroh_metrics::inc!(Metrics, content_routing_blobs_fetched);
                    return Ok(());
                }
                Err(err) => {
                    trace!("failed to fetch from provider: {:?}", err);
                    continue;
                }
            }
        }

        Err(anyhow::anyhow!("Failed to fetch blob from any provider"))
    }

    pub(crate) async fn announce_provide(
        &self,
        author_id: AuthorId,
        hash: Hash,
        node_id: NodeId,
    ) -> Result<()> {
        let key = provider_key(hash, node_id);
        iroh_metrics::inc!(Metrics, content_routing_blobs_announced);
        // can't use the empty hash here, going with a dummy value for now
        self.doc.set_bytes(author_id, key, EMPTY_OK_VALUE).await?;
        Ok(())
    }

    pub(crate) async fn find_providers(&self, hash: Hash) -> Result<Vec<NodeId>> {
        let prefix = providers_key(hash);

        let mut results: Vec<NodeId> = Vec::new();
        let mut entries = self.doc.get_many(Query::key_prefix(&prefix)).await?;
        while let Some(entry) = entries.next().await {
            let entry = entry?;
            let prov_key = entry.key();
            let prov_key = String::from_utf8(prov_key.to_vec())
                .map_err(|_| anyhow::anyhow!("Invalid UTF-8"))?;
            let node_id = node_key_component(prov_key.as_str())?;
            results.push(node_id);
        }
        Ok(results)
    }

    pub(crate) async fn handle_event(&self, event: Event) -> Result<()> {
        // we listen for provider addition instead of blob creation because blobs are useless
        // unless they can be fetched
        if self.autofetch == AutofetchPolicy::All {
            if let EventData::ContentRouting(e) = event.data {
                match e {
                    ContentRoutingEvent::ProviderAdded { hash, provider } => {
                        // TODO - we run the risk of overwhelming initial new providers if
                        // there are many nodes that request here. I think the right approach
                        // is dial backoffs on the provider side, ideally with a TTL that clients
                        // should honor before re-requesting
                        let self2 = self.clone();
                        tokio::task::spawn(async move {
                            if fetch_blob_from_provider(&self2.node, hash, provider)
                                .await
                                .is_ok()
                            {
                                self2
                                    .announce_provide(self2.author_id, hash, self2.node_id)
                                    .await
                                    .unwrap();
                                trace!(
                                    "AutoFetched & Provoded blob {} from provider: {}",
                                    hash,
                                    provider
                                );
                            }
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

async fn fetch_blob_from_provider(node: &Protocols, hash: Hash, provider: NodeId) -> Result<()> {
    trace!(
        hash = %hash,
        provider = %provider,
        "fetch_blob_from_provider");

    let addr = NodeAddr::new(provider);
    let outcome = node.blobs().download(hash, addr).await?.await?;
    trace!("Downloaded blob: {:?}", outcome);
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) enum ContentRoutingEvent {
    ProviderAdded { provider: NodeId, hash: Hash },
    // ProviderRemoved { provider: NodeId, hash: Hash },
}

pub(crate) fn parse_content_routing_event(key: &str) -> Option<EventData> {
    match event_components(key) {
        Ok((hash, provider)) => Some(EventData::ContentRouting(
            ContentRoutingEvent::ProviderAdded { hash, provider },
        )),
        Err(e) => {
            tracing::error!("failed to parse content routing event: {}", e);
            None
        }
    }
    // TODO - when we support deletes, we'll need to check for null hash values
}

fn event_components(key: &str) -> Result<(Hash, NodeId)> {
    let mut parts = key.splitn(3, '/').skip(1); // lop off CONTENT_ROUTING_PREFIX
    let hash = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing hash component"))?;
    let hash = Hash::from_str(hash).context("invalid hash component")?;
    let node_id = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing node_id component"))?;
    let node_id = NodeId::from_str(node_id).context("invalid node_id component")?;

    Ok((hash, node_id))
}

fn node_key_component(key: &str) -> Result<NodeId> {
    let mut parts = key.splitn(3, '/').skip(2); // lop off CONTENT_ROUTING_PREFIX, hash
    let node_id = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing node_id component"))?;
    let node_id = NodeId::from_str(node_id).context("invalid nod_id component")?;
    Ok(node_id)
}

fn providers_key(hash: Hash) -> String {
    format!("{}/{}/", CONTENT_ROUTING_PREFIX, hash)
}

fn provider_key(hash: Hash, node_id: NodeId) -> String {
    format!("{}{}", providers_key(hash), node_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::test_utils::create_router;
    use crate::vm::workspace::VM;
    use crate::vm::{config::NodeConfig, test_utils::setup_logging};
    use anyhow::{Context, Result};
    use iroh::base::node_addr::AddrInfoOptions;
    use std::time::Duration;
    use tokio::time;

    #[tokio::test]
    async fn autofetch_basic() -> Result<()> {
        setup_logging();

        let ws_name = "test_workspace";
        let temp_dir = tempfile::tempdir().context("tempdir")?;

        let repo_1_path = temp_dir.path().join("repo_1");
        let cfg1 = &NodeConfig {
            iroh_port: 5467,
            ..Default::default()
        };

        let node_1 = create_router(&repo_1_path, cfg1).await?;
        let ws1 = VM::create(
            String::from(ws_name),
            node_1.node_id(),
            &node_1,
            cfg1.workspace_config(),
        )
        .await?;

        let ticket = ws1
            .get_write_ticket(AddrInfoOptions::RelayAndAddresses)
            .await?;

        let repo_2_path = temp_dir.path().join("repo_2");
        let cfg2 = &NodeConfig {
            iroh_port: 5468,
            autofetch_default: AutofetchPolicy::All,
            ..Default::default()
        };
        let node_2 = create_router(&repo_2_path, cfg2).await?;
        let ws2 = VM::join(node_2.node_id(), &node_2, ticket, cfg2.workspace_config()).await?;

        let (hash, _) = ws1
            .blobs()
            .put_bytes(
                "hello/world.json",
                bytes::Bytes::from_static(b"{\"hello\": \"world\"}"),
            )
            .await?;

        time::timeout(Duration::from_secs(6), async {
            loop {
                time::sleep(Duration::from_millis(100)).await;
                let provs = ws1.blobs().router().find_providers(hash).await?;
                if provs.len() == 2 {
                    return anyhow::Ok(());
                }
            }
        })
        .await??;

        drop(ws1);
        drop(ws2);

        Ok(())
    }
}

use anyhow::{anyhow, Result};
use bytes::Bytes;
use futures::TryStreamExt;
use iroh::blobs::Hash;
use iroh::client::docs::Entry;
use iroh::docs::store::Query;
use iroh::docs::AuthorId;
use iroh::net::NodeId;

use tracing::{debug, warn};

use crate::content_routing::{AutofetchPolicy, ContentRouter};
use crate::doc::{Doc, Event, EventData};
use crate::node::IrohNodeClient;

/// prefix used for blobs in the doc
pub(crate) const BLOBS_DOC_PREFIX: &str = "blobs";

#[derive(Debug, Clone)]
pub struct Blobs {
    // nodeID doubles as the author ID for this replica when writing to the doc
    node_id: NodeId,
    node: IrohNodeClient,
    doc: Doc,
    content_router: ContentRouter,
}

impl Blobs {
    pub fn new(
        node_id: NodeId,
        doc: Doc,
        node: IrohNodeClient,
        autofetch: AutofetchPolicy,
    ) -> Self {
        let author_id = iroh::docs::AuthorId::from(node_id.as_bytes());
        let content_router =
            ContentRouter::new(author_id, node_id, doc.clone(), node.clone(), autofetch);
        Self {
            node_id,
            doc,
            node,
            content_router,
        }
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    pub(crate) fn router(&self) -> &ContentRouter {
        &self.content_router
    }

    pub async fn fetch_blob(&self, hash: Hash) -> Result<()> {
        self.content_router.fetch_blob(hash).await
    }

    fn author_id(&self) -> AuthorId {
        self.node_id.as_bytes().into()
    }

    pub async fn list_objects(&self) -> Result<Vec<Entry>> {
        let query = Query::key_prefix(BLOBS_DOC_PREFIX);
        let entries = self
            .doc
            .get_many(query)
            .await?
            .map_ok(|e| {
                debug!("entry: {:?}", e);
                e
            })
            .try_collect()
            .await?;
        Ok(entries)
    }

    pub async fn put_bytes(&self, key: &str, data: impl Into<bytes::Bytes>) -> Result<(Hash, u64)> {
        let res = self.node.blobs().add_bytes(data.into()).await?;
        self.put_object(key, res.hash, res.size).await?;
        Ok((res.hash, res.size))
    }

    pub async fn put_object(&self, key: &str, hash: Hash, size: u64) -> Result<()> {
        let key = object_key(key);
        let author_id = self.author_id();
        self.doc.set_hash(author_id, key, hash, size).await?;
        self.router()
            .announce_provide(author_id, hash, self.node_id)
            .await
    }

    pub async fn fetch_object(&self, key: &str) -> Result<()> {
        let info = self.get_object_info(key).await?;
        self.fetch_blob(info.content_hash()).await?;
        Ok(())
    }

    pub async fn get_object(&self, key: &str) -> Result<Bytes> {
        let info = self.get_object_info(key).await?;
        self.fetch_blob(info.content_hash()).await?;
        let data = self.node.blobs().read_to_bytes(info.content_hash()).await?;
        Ok(data)
    }

    pub async fn get_object_info(&self, key: &str) -> Result<Entry> {
        let key = object_key(key);
        let query = Query::key_exact(key.clone());
        match self.doc.get_one(query).await? {
            Some(entry) => Ok(entry),
            None => Err(anyhow!("object not found: {}", key)),
        }
    }

    pub async fn has_object(&self, key: &str) -> Result<bool> {
        let key = object_key(key);
        let query = Query::key_exact(key);
        let res = self.doc.get_one(query).await?;
        Ok(res.is_some())
    }

    pub async fn delete_object(&self, _key: &str) -> Result<()> {
        todo!();
    }

    pub(crate) async fn handle_event(&self, event: Event) -> Result<()> {
        self.content_router.handle_event(event).await
    }
}

fn object_key(key: &str) -> String {
    format!("{}/{}", BLOBS_DOC_PREFIX, key)
}

impl std::hash::Hash for Blobs {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.doc.id().hash(state);
    }
}

#[derive(Debug, Clone)]
pub(crate) enum BlobsEvent {
    // ObjectPut { key: String, hash: Hash, size: u64 },
    BlobPut,
    // ObjectDeleted { key: String, hash: Hash, size: u64 },
}

pub(crate) fn parse_blobs_event(key: &str) -> Option<EventData> {
    match event_components(key) {
        // matched key here has the BLOBS_DOC_PREFIX removed
        Ok(_key) => {
            // let hash = entry.content_hash();
            // let size = entry.content_len();
            Some(EventData::Blobs(BlobsEvent::BlobPut))
        }
        Err(e) => {
            warn!("parse_blobs_event: {:?}", e);
            None
        }
    }
}

fn event_components(key: &str) -> Result<&str> {
    let object_name = key
        .strip_prefix(&format!("{}/", BLOBS_DOC_PREFIX))
        .ok_or_else(|| anyhow!("invalid object key"))?;

    Ok(object_name)
}

#[cfg(test)]
mod tests {
    use crate::test_utils::create_nodes;
    use anyhow::{Context, Result};

    #[tokio::test]
    async fn two_node_blob_replication() -> Result<()> {
        let temp_dir = tempfile::tempdir().context("tempdir")?;
        let nodes = create_nodes(&temp_dir, 2).await?;

        let (_node1, ws1) = &nodes[0];
        let (hash, _) = ws1.blobs().put_bytes("hello.txt", "hello").await?;

        // silly wait for replication
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let (_, ws2) = &nodes[1];
        let info = ws2.blobs().get_object_info("hello.txt").await?;
        assert_eq!(info.content_hash(), hash);

        Ok(())
    }
}

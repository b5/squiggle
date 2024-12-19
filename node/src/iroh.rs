use std::path::PathBuf;

use anyhow::Result;
use futures::StreamExt;
use iroh::{protocol::Router, Endpoint};
use iroh_blobs::{net_protocol::Blobs, util::local_pool::LocalPool, ALPN as BLOBS_ALPN};
use iroh_docs::protocol::Docs;
use iroh_docs::{AuthorId, ALPN as DOCS_ALPN};
use iroh_gossip::{net::Gossip, ALPN as GOSSIP_ALPN};

pub(crate) type DocsClient = iroh_docs::rpc::client::docs::Client<
    quic_rpc::client::FlumeConnector<iroh_docs::rpc::proto::RpcService>,
>;

/// An in-memory RPC client for a single document
pub type Doc = iroh_docs::rpc::client::docs::Doc<
    quic_rpc::client::FlumeConnector<iroh_docs::rpc::proto::RpcService>,
>;

pub(crate) type BlobsClient = iroh_blobs::rpc::client::blobs::Client<
    quic_rpc::client::FlumeConnector<iroh_blobs::rpc::proto::RpcService>,
>;

#[derive(Debug, Clone)]
pub struct Protocols {
    endpoint: Endpoint,
    router: Router,
    gossip: Gossip,
    blobs: Blobs<iroh_blobs::store::mem::Store>,
    docs: Docs<iroh_blobs::store::mem::Store>,
    pub(crate) node_id: iroh::NodeId,
}

impl Protocols {
    pub(crate) async fn spawn(_path: PathBuf) -> Result<Self> {
        // create an iroh endpoint that includes the standard discovery mechanisms
        // we've built at number0
        let endpoint = Endpoint::builder().discovery_n0().bind().await?;

        let node_id = endpoint.node_id();

        // create a router builder, we will add the protocols to this
        // builder and then spawn the router
        let builder = Router::builder(endpoint.clone());

        // build the blobs protocol
        let local_pool = LocalPool::default();
        let blobs = Blobs::memory().build(local_pool.handle(), builder.endpoint());

        // build the gossip protocol
        let gossip = Gossip::builder().spawn(builder.endpoint().clone()).await?;

        // build the docs protocol
        let docs = Docs::memory().spawn(&blobs, &gossip).await?;

        // setup router
        let router = builder
            .accept(BLOBS_ALPN, blobs.clone())
            .accept(GOSSIP_ALPN, gossip.clone())
            .accept(DOCS_ALPN, docs.clone())
            .spawn()
            .await
            .unwrap();

        Ok(Self {
            endpoint,
            router,
            node_id,
            gossip,
            blobs,
            docs,
        })
    }

    pub(crate) async fn get_author(&self) -> Result<AuthorId> {
        let mut stream = self.docs.client().authors().list().await?;
        if let Some(author_id) = stream.next().await {
            let author_id = author_id?;
            return Ok(author_id);
        }
        let author_id = self.docs.client().authors().create().await?;
        Ok(author_id)
    }

    pub(crate) fn endpoint(&self) -> Endpoint {
        self.endpoint.clone()
    }

    pub(crate) fn docs(&self) -> &DocsClient {
        &self.docs.client()
    }

    pub(crate) fn blobs(&self) -> &BlobsClient {
        &self.blobs.client()
    }

    pub(crate) fn gossip(&self) -> &Gossip {
        &self.gossip
    }
}

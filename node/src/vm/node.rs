use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::{Path, PathBuf};

use anyhow::Result;
use iroh::docs::AuthorId;
use iroh::net::{discovery::dns::DnsDiscovery, relay::RelayMode, NodeId};
use iroh::util::path::IrohPaths;
use tracing::{debug, info};

use super::config::NodeConfig;

pub type IrohNode = iroh::node::FsNode;
pub type IrohNodeClient = iroh::client::Iroh;

pub(crate) async fn create_iroh(data_dir: &PathBuf, cfg: &NodeConfig) -> Result<IrohNode> {
    debug!("using data directory: {}", data_dir.display());
    let secret_key =
        iroh::util::fs::load_secret_key(IrohPaths::SecretKey.with_root(data_dir)).await?;

    tokio::fs::create_dir_all(&data_dir).await?;

    let dns_discovery = DnsDiscovery::n0_dns();

    let relay_mode = match cfg.relay_map()? {
        Some(dm) => RelayMode::Custom(dm),
        None => RelayMode::Default,
    };

    let author = iroh::docs::Author::from_bytes(&secret_key.to_bytes());
    let node = iroh::node::Node::persistent(data_dir)
        .await?
        .secret_key(secret_key)
        .relay_mode(relay_mode)
        .enable_docs()
        .bind_addr_v4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, cfg.iroh_port))
        .node_discovery(iroh::node::DiscoveryConfig::Custom(Box::new(dns_discovery)))
        .gc_policy(cfg.gc_policy)
        .spawn()
        .await?;

    node.authors().import(author).await?;

    let addr: iroh::net::NodeAddr = node.net().node_addr().await?;
    info!(
        id = %node.node_id().to_string(),
        port = %cfg.iroh_port,
        relay_url = ?addr.info.relay_url,
        "iroh node is running",
    );

    Ok(node)
}

pub(crate) async fn read_node_id_from_file(data_dir: &Path) -> Result<NodeId> {
    let path = IrohPaths::SecretKey.with_root(data_dir);
    let secret_key = iroh::util::fs::load_secret_key(path).await?;
    Ok(NodeId::from_bytes(secret_key.public().as_bytes()).unwrap())
}

pub(crate) fn node_author_id(node_id: &NodeId) -> AuthorId {
    AuthorId::from(node_id.as_bytes())
}

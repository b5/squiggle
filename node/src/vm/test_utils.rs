use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::PathBuf;

use anyhow::Result;
use iroh::base::node_addr::AddrInfoOptions;
use iroh::net::{discovery::dns::DnsDiscovery, relay::RelayMode};
use iroh::util::path::IrohPaths;
use tempfile::TempDir;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;
use tracing_subscriber::{fmt, EnvFilter};

use crate::router::Router;

use super::config::NodeConfig;
use super::workspace::Workspace;

pub(crate) async fn create_router(data_dir: &PathBuf, cfg: &NodeConfig) -> Result<Router> {
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

pub fn setup_logging() {
    let subscriber = fmt::layer().with_filter(EnvFilter::from_default_env());
    tracing_subscriber::registry()
        .with(subscriber)
        .with(tracing_error::ErrorLayer::default())
        .try_init()
        .ok();
}

/// Creates a given number of iroh nodes all subscribed to the same workspace
pub async fn create_nodes(td: &TempDir, num: usize) -> Result<Vec<(Router, Workspace)>> {
    let mut nodes = Vec::new();
    let mut ticket = None;
    let name = "test_workspace".to_string();

    for i in 0..num {
        let repo_path = td.path().join(format!("repo_{}", i));
        let cfg = &NodeConfig {
            iroh_port: 5467 + i as u16,
            ..Default::default()
        };
        let node = create_router(&repo_path, cfg).await?;

        match ticket {
            None => {
                let ws =
                    Workspace::create(name.clone(), node.node_id(), &node, cfg.workspace_config())
                        .await?;
                ticket = Some(
                    ws.get_write_ticket(AddrInfoOptions::RelayAndAddresses)
                        .await?,
                );
                nodes.push((node, ws));
            }
            Some(ref ticket) => {
                let ws = Workspace::join(
                    node.node_id(),
                    &node,
                    ticket.clone(),
                    cfg.workspace_config(),
                )
                .await?;
                nodes.push((node, ws));
            }
        }
    }
    Ok(nodes)
}

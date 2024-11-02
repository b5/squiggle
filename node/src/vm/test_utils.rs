use anyhow::Result;
use iroh::base::node_addr::AddrInfoOptions;
use tempfile::TempDir;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;
use tracing_subscriber::{fmt, EnvFilter};

use super::config::NodeConfig;
use super::node::create_iroh;
use super::node::IrohNode;
use super::workspace::Workspace;

pub fn setup_logging() {
    let subscriber = fmt::layer().with_filter(EnvFilter::from_default_env());
    tracing_subscriber::registry()
        .with(subscriber)
        .with(tracing_error::ErrorLayer::default())
        .try_init()
        .ok();
}

/// Creates a given number of iroh nodes all subscribed to the same workspace
pub async fn create_nodes(td: &TempDir, num: usize) -> Result<Vec<(IrohNode, Workspace)>> {
    let mut nodes = Vec::new();
    let mut ticket = None;
    let name = "test_workspace".to_string();

    for i in 0..num {
        let repo_path = td.path().join(format!("repo_{}", i));
        let cfg = &NodeConfig {
            iroh_port: 5467 + i as u16,
            ..Default::default()
        };
        let node = create_iroh(&repo_path, cfg).await?;

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

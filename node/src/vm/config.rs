use std::path::PathBuf;

use iroh::defaults::prod::{default_eu_relay_node, default_na_relay_node};
use iroh::RelayNode;
use serde::{Deserialize, Serialize};

use super::content_routing::AutofetchPolicy;

/// The configuration for an iroh node.
#[derive(PartialEq, Eq, Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct NodeConfig {
    /// Control automatic content fetching within a workspace
    pub autofetch_default: AutofetchPolicy,
    /// Port number for the main iroh fog HTTP API to listen on.
    pub api_port: u16,
    /// Bind address on which to serve Prometheus metrics
    pub metrics_port: Option<u16>,

    /// Port for iroh to listen on for direct connections. Defaults to 0 for random available
    /// port assignement.
    pub iroh_port: u16,
    /// The set of iroh relay nodes to use.
    pub relay_nodes: Vec<RelayNode>,
    /// Address of the tracing collector.
    /// eg: set to http://localhost:4317 for a locally running Jaeger instance.
    pub tracing_endpoint: Option<String>,

    /// Root folder used for storing and retrieving assets shared with the worker.
    pub worker_root: PathBuf,
}

impl Default for NodeConfig {
    fn default() -> Self {
        let worker_root =
            tempfile::TempDir::with_prefix("fog-worker").expect("unable to create tempdir");
        let worker_root = worker_root.into_path();
        Self {
            api_port: 8015,
            metrics_port: Some(8016),
            iroh_port: 0,
            relay_nodes: [default_na_relay_node(), default_eu_relay_node()].into(),
            autofetch_default: AutofetchPolicy::Disabled,
            tracing_endpoint: None,
            worker_root,
        }
    }
}

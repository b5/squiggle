//! Metrics for fog
use std::net::SocketAddr;

use iroh_metrics::core::{Counter, Metric};
use iroh_metrics::struct_iterable::Iterable;
use tracing::{debug, info};

/// Enum of metrics for the module
#[allow(missing_docs)]
#[derive(Debug, Clone, Iterable)]
pub struct Metrics {
    pub workspaces: Counter,

    pub flow_run_started: Counter,
    pub flow_run_completed: Counter,
    pub task_run_started: Counter,
    pub task_run_completed: Counter,

    pub scheduler_jobs_requested: Counter,
    pub scheduler_jobs_assigned: Counter,
    pub scheduler_jobs_completed: Counter,
    pub scheduler_jobs_canceled: Counter,

    pub worker_jobs_requested: Counter,
    pub worker_jobs_skipped: Counter,
    pub worker_jobs_running: Counter,
    pub worker_jobs_completed: Counter,

    pub content_routing_blobs_announced: Counter,
    pub content_routing_blobs_fetched: Counter,
}

impl Default for Metrics {
    #[rustfmt::skip]
    fn default() -> Self {
        Self {
            workspaces: Counter::new("Count of workspaces"),

            flow_run_started: Counter::new("Count of flow runs started"),
            flow_run_completed: Counter::new("Count of flow runs completed"),
            task_run_started: Counter::new("Count of task runs started"),
            task_run_completed: Counter::new("Count of task runs completed"),

            scheduler_jobs_requested: Counter::new("Count of jobs requested by the scheduler"),
            scheduler_jobs_assigned: Counter::new("Count of jobs assigned by the scheduler"),
            scheduler_jobs_completed: Counter::new("Count of jobs completed by the scheduler"),
            scheduler_jobs_canceled: Counter::new("Count of jobs canceled by the scheduler"),

            worker_jobs_requested: Counter::new("Count of jobs requested by the worker"),
            worker_jobs_skipped: Counter::new("Count of jobs skipped by the worker"),
            worker_jobs_running: Counter::new("Count of jobs ever started by the worker"),
            worker_jobs_completed: Counter::new("Count of jobs completed by the worker"),

            content_routing_blobs_announced: Counter::new("Count of blobs announced by the content router"),
            content_routing_blobs_fetched: Counter::new("Count of blobs fetched by the content router"),
        }
    }
}

impl Metric for Metrics {
    fn name() -> &'static str {
        "fog"
    }
}

pub fn try_init_metrics_collection() -> std::io::Result<()> {
    iroh_metrics::core::Core::try_init(|reg, metrics| {
        metrics.insert(crate::vm::metrics::Metrics::new(reg));
        metrics.insert(iroh::docs::metrics::Metrics::new(reg));
        metrics.insert(iroh::net::metrics::MagicsockMetrics::new(reg));
        metrics.insert(iroh::net::metrics::NetcheckMetrics::new(reg));
        metrics.insert(iroh::net::metrics::PortmapMetrics::new(reg));
    })
}

pub fn start_metrics_server(metrics_port: Option<u16>) -> Option<tokio::task::JoinHandle<()>> {
    // doesn't start the server if the address is None
    if let Some(metrics_port) = metrics_port {
        let metrics_addr = SocketAddr::from(([0, 0, 0, 0], metrics_port));
        // metrics are initilaized with try_init_metrics_collection
        // here we only start the server
        info!("Starting metrics server at {}", metrics_addr);
        return Some(tokio::task::spawn(async move {
            if let Err(e) = iroh_metrics::metrics::start_metrics_server(metrics_addr).await {
                eprintln!("Failed to start metrics server: {e}");
            }
        }));
    }
    debug!("Metrics server not started, no address provided");
    None
}

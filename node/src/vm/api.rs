use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;

use anyhow::Result;
use axum::response::IntoResponse;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use tokio::net::TcpListener;
use tracing::{debug, error, info};
use uuid::Uuid;

use super::flow::Flow;
use super::job::JobDescription;
use super::node::IrohNode;
use super::workspace::Workspaces;

#[derive(Debug, Clone)]
pub struct FogApi(Inner);

impl Deref for FogApi {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct Inner {
    pub(crate) node: IrohNode,
    pub(crate) workspaces: Arc<Workspaces>,
}

impl FogApi {
    pub fn new(node: IrohNode, workspaces: Workspaces) -> Self {
        let workspaces = Arc::new(workspaces);
        Self(Inner { node, workspaces })
    }

    pub async fn serve(&self, port: u16) -> Result<()> {
        let app = Router::new()
            .route("/status", post(|| async { (StatusCode::OK, "ok") }))
            .route("/:workspace/jobs", post(run_job_handler))
            .route("/:workspace/flows", post(run_flow_handler))
            .with_state(self.clone());

        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        info!("worker api listening at http://{}", addr);

        tokio::task::spawn(async move {
            let listener = TcpListener::bind(addr).await.unwrap();
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });

        Ok(())
    }
}

async fn run_job_handler(
    State(app): State<FogApi>,
    Path(workspace): Path<String>,
    Json(payload): Json<JobDescription>,
) -> impl IntoResponse {
    debug!("Received job for workspace {}: {:?}", workspace, payload);
    let ws = app.workspaces.get(&workspace).await.unwrap();
    let scope = Uuid::new_v4();
    let id = Uuid::new_v4();
    match ws.run_job(scope, id, payload).await {
        Ok(id) => (StatusCode::OK, id.to_string()),
        Err(e) => {
            error!("failed to create job: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                String::from("failed to create job"),
            )
        }
    }
}

async fn run_flow_handler(
    State(app): State<FogApi>,
    Path(workspace): Path<String>,
    Json(flow): Json<Flow>,
) -> impl IntoResponse {
    debug!("Received flow for workspace {}: {:?}", workspace, flow);
    let ws = app.workspaces.get(&workspace).await.unwrap();
    match flow.run(&app.node, &ws).await {
        Ok(result) => {
            let data = serde_json::to_string(&result).unwrap();
            (StatusCode::OK, data)
        }
        Err(e) => {
            error!("failed to run flow: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                String::from("failed to run flow"),
            )
        }
    }
}

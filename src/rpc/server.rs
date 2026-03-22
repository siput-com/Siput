//! RPC Server
//!
//! Main RPC server implementation with modular handlers,
//! interface-based design, and API versioning.

use crate::rpc::handlers::*;
use crate::rpc::services::*;
use crate::rpc::versioning::*;
use crate::observability::{create_rpc_span, metrics};
use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// RPC server state with service interfaces
#[derive(Clone)]
pub struct RpcState {
    pub service: Arc<RpcService>,
}

/// Create the RPC server router with versioned endpoints
pub fn create_router(state: RpcState) -> Router {
    let blockchain_routes = Router::new()
        .route("/send_tx", post(submit_transaction))
        .route("/balance/:address", get(get_balance))
        .route("/block/:hash", get(get_block))
        .route("/tx/:hash", get(get_transaction))
        .route("/dag", get(get_dag_info));

    let contract_routes = Router::new()
        .route("/deploy", post(deploy_contract))
        .route("/call", post(call_contract))
        .route("/:address", get(get_contract_info));

    let node_routes = Router::new()
        .route("/info", get(get_node_info))
        .route("/mempool", get(get_mempool_info));

    Router::new()
        .nest(&versioned_path("blockchain"), blockchain_routes)
        .nest(&versioned_path("contracts"), contract_routes)
        .nest(&versioned_path("node"), node_routes)
        .route(&versioned_path("version"), get(get_api_version))
        .layer(axum::middleware::from_fn(version_middleware))
        .with_state(state)
}

/// GET /v1/version - Get API version information
pub async fn get_api_version() -> Json<VersionInfo> {
    Json(get_version_info())
}

/// Start the RPC server
pub async fn start_server(
    addr: &str,
    rpc_service: Arc<RpcService>,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = RpcState {
        service: rpc_service,
    };

    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("RPC server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

//! RPC Server
//!
//! Main RPC server implementation with modular handlers,
//! interface-based design, and API versioning.

use crate::contracts::contract_registry::ContractRegistry;
use crate::core::{Block, Transaction};
use crate::dag::blockdag::BlockDAG;
use crate::mempool::tx_dag_mempool::TxDagMempool;
use crate::network::p2p_node::P2PNode;
use crate::rpc::handlers::*;
use crate::rpc::services::*;
use crate::rpc::versioning::*;
use crate::state::state_manager::StateManager;
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
        .route("/send_tx", post(send_transaction))
        .route("/balance/:address", get(get_balance))
        .route("/block/:hash", get(get_block))
        .route("/tx/:hash", get(get_transaction))
        .route("/tx/status/:hash", get(get_transaction_status))
        .route("/dag", get(get_dag_info));

    let contract_routes = Router::new()
        .route("/deploy", post(deploy_contract))
        .route("/call", post(call_contract))
        .route("/:address", get(get_contract_info))
        .route("/list", get(get_contract_list));

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
    dag: Arc<RwLock<BlockDAG>>,
    mempool: Arc<TxDagMempool>,
    state_manager: Arc<Mutex<StateManager>>,
    contract_registry: Arc<Mutex<ContractRegistry>>,
    p2p_node: Option<Arc<tokio::sync::RwLock<P2PNode>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create service implementations
    let blockchain_service = Arc::new(BlockchainServiceImpl::new(
        Arc::clone(&dag),
        Arc::clone(&mempool),
        Arc::clone(&state_manager),
    ));

    let contract_service = Arc::new(ContractServiceImpl::new(
        Arc::clone(&contract_registry),
        Arc::clone(&state_manager),
    ));

    let node_service = Arc::new(NodeServiceImpl::new(
        p2p_node.as_ref().map(Arc::clone),
        Arc::clone(&mempool),
        Arc::clone(&dag),
    ));

    let rpc_service = Arc::new(RpcService {
        blockchain: blockchain_service,
        contracts: contract_service,
        node: node_service,
    });

    let state = RpcState {
        service: rpc_service,
    };

    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("RPC server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

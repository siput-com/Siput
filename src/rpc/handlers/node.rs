//! Network and node-related RPC handlers
//!
//! This module contains handlers for network operations and node information.
//!
//! API Version: v1
//! Base path: /v1/node

use crate::rpc::interfaces::*;
use crate::rpc::services::*;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};

/// Node info response
#[derive(Serialize)]
pub struct NodeInfoResponse {
    pub peer_id: String,
    pub connected_peers: Vec<String>,
    pub mempool_size: usize,
    pub dag_height: usize,
    pub version: String,
    pub uptime: u64,
}

/// Network stats response
#[derive(Serialize)]
pub struct NetworkStatsResponse {
    pub connected_peers: usize,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
}

/// API version response
#[derive(Serialize)]
pub struct ApiVersionResponse {
    pub version: String,
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub features: Vec<String>,
}

/// Health check response
#[derive(Serialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub timestamp: u64,
    pub services: serde_json::Value,
}

/// GET /v1/node/info - Get node information
pub async fn get_node_info(
    State(state): State<super::RpcHandlerState>,
) -> Result<Json<RpcResponse<NodeInfoResponse>>, StatusCode> {
    let node_info = state.service.blockchain().get_node_info()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = NodeInfoResponse {
        peer_id: node_info.peer_id,
        connected_peers: node_info.connected_peers,
        mempool_size: node_info.mempool_size,
        dag_height: node_info.dag_height,
        version: node_info.version,
        uptime: node_info.uptime,
    };

    Ok(Json(RpcResponse::new(response)))
}

/// GET /v1/network/stats - Get network statistics
pub async fn get_network_stats(
    State(state): State<super::RpcHandlerState>,
) -> Result<Json<RpcResponse<NetworkStatsResponse>>, StatusCode> {
    let network_stats = state.service.network().get_network_stats()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = NetworkStatsResponse {
        connected_peers: network_stats.connected_peers,
        bytes_sent: network_stats.bytes_sent,
        bytes_received: network_stats.bytes_received,
        messages_sent: network_stats.messages_sent,
        messages_received: network_stats.messages_received,
    };

    Ok(Json(RpcResponse::new(response)))
}

/// GET /v1/network/peers - Get connected peers
pub async fn get_connected_peers(
    State(state): State<super::RpcHandlerState>,
) -> Result<Json<RpcResponse<Vec<String>>>, StatusCode> {
    let peers = state.service.network().get_connected_peers()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(RpcResponse::new(peers)))
}

/// GET /v1/version - Get API version information
pub async fn get_api_version() -> Json<RpcResponse<ApiVersionResponse>> {
    let version = ApiVersion::current();
    let response = ApiVersionResponse {
        version: version.version,
        major: version.major,
        minor: version.minor,
        patch: version.patch,
        features: version.features,
    };

    Json(RpcResponse::new(response))
}

/// GET /v1/health - Health check endpoint
pub async fn health_check(
    State(state): State<super::RpcHandlerState>,
) -> Result<Json<RpcResponse<HealthCheckResponse>>, StatusCode> {
    // Check various services
    let blockchain_ok = state.service.blockchain().get_dag_info().await.is_ok();
    let network_ok = state.service.network().get_connected_peers().await.is_ok();

    let services = serde_json::json!({
        "blockchain": if blockchain_ok { "healthy" } else { "unhealthy" },
        "network": if network_ok { "healthy" } else { "unhealthy" },
        "contracts": "unknown" // TODO: implement contract health check
    });

    let status = if blockchain_ok && network_ok { "healthy" } else { "degraded" };

    let response = HealthCheckResponse {
        status: status.to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        services,
    };

    Ok(Json(RpcResponse::new(response)))
}
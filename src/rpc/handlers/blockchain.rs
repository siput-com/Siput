//! Blockchain-related RPC handlers
//!
//! This module contains handlers for blockchain operations like
//! transactions, blocks, balances, and DAG information.
//!
//! API Version: v1
//! Base path: /v1/blockchain

use crate::rpc::interfaces::*;
use crate::rpc::services::*;
use crate::observability::{create_rpc_span, metrics};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Shared RPC state
#[derive(Clone)]
pub struct RpcHandlerState {
    pub service: Arc<RpcService>,
}

/// Transaction submission request
#[derive(Deserialize)]
pub struct SendTxRequest {
    pub transaction: crate::core::Transaction,
}

/// Balance response
#[derive(Serialize)]
pub struct BalanceResponse {
    pub address: String,
    pub balance: u64,
    pub nonce: u64,
}

/// Block response
#[derive(Serialize)]
pub struct BlockResponse {
    pub hash: String,
    pub block: Option<crate::core::Block>,
}

/// Transaction response
#[derive(Serialize)]
pub struct TransactionResponse {
    pub hash: String,
    pub transaction: Option<crate::core::Transaction>,
    pub status: TransactionStatus,
}

/// DAG info response
#[derive(Serialize)]
pub struct DagResponse {
    pub tips: Vec<String>,
    pub total_blocks: usize,
    pub height: u64,
    pub stats: serde_json::Value,
}

/// Mempool response
#[derive(Serialize)]
pub struct MempoolResponse {
    pub tx_count: usize,
    pub tx_hashes: Vec<String>,
    pub total_gas: u64,
}

/// POST /v1/blockchain/transactions - Submit transaction
pub async fn submit_transaction(
    State(state): State<RpcHandlerState>,
    Json(request): Json<SendTxRequest>,
) -> Result<Json<RpcResponse<serde_json::Value>>, StatusCode> {
    let _span = create_rpc_span("submit_transaction", None);
    trace_performance!(async {
        let tx_hash = hex::encode(&request.transaction.hash);

        tracing::info!(
            transaction_hash = %tx_hash,
            "Submitting transaction via RPC"
        );

        let start_time = std::time::Instant::now();
        let result = state.service.blockchain().submit_transaction(request.transaction).await;
        let duration = start_time.elapsed();

        metrics::record_latency("rpc_submit_transaction", duration);
        metrics::increment_counter("rpc_requests_total", &[("method", "submit_transaction")]);

        match result {
            Ok(_) => {
                metrics::increment_counter("rpc_requests_success", &[("method", "submit_transaction")]);
                tracing::info!(
                    transaction_hash = %tx_hash,
                    duration_ms = duration.as_millis(),
                    "Transaction submitted successfully"
                );

                let response = RpcResponse::new(serde_json::json!({
                    "status": "success",
                    "message": "Transaction submitted to mempool",
                    "transaction_hash": tx_hash
                }));
                Ok(Json(response))
            }
            Err(e) => {
                metrics::increment_counter("rpc_requests_error", &[("method", "submit_transaction")]);
                tracing::error!(
                    transaction_hash = %tx_hash,
                    error = %e,
                    duration_ms = duration.as_millis(),
                    "Failed to submit transaction"
                );
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }).await
}

/// GET /v1/blockchain/balance/{address} - Get account balance
pub async fn get_balance(
    State(state): State<RpcHandlerState>,
    Path(address_str): Path<String>,
) -> Result<Json<RpcResponse<BalanceResponse>>, StatusCode> {
    // Parse address
    let address = parse_address(&address_str)?;

    let (balance, nonce) = tokio::try_join!(
        state.service.blockchain().get_balance(address),
        state.service.blockchain().get_nonce(address)
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = BalanceResponse {
        address: address_str,
        balance,
        nonce,
    };

    Ok(Json(RpcResponse::new(response)))
}

/// GET /v1/blockchain/blocks/{hash} - Get block by hash
pub async fn get_block(
    State(state): State<RpcHandlerState>,
    Path(hash_str): Path<String>,
) -> Result<Json<RpcResponse<BlockResponse>>, StatusCode> {
    let hash = parse_block_hash(&hash_str)?;
    let block = state.service.blockchain().get_block(hash)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = BlockResponse {
        hash: hash_str,
        block,
    };

    Ok(Json(RpcResponse::new(response)))
}

/// GET /v1/blockchain/transactions/{hash} - Get transaction by hash
pub async fn get_transaction(
    State(state): State<RpcHandlerState>,
    Path(hash_str): Path<String>,
) -> Result<Json<RpcResponse<TransactionResponse>>, StatusCode> {
    let hash = parse_tx_hash(&hash_str)?;

    let (transaction, status) = tokio::try_join!(
        state.service.blockchain().get_transaction(hash),
        state.service.blockchain().get_transaction_status(hash)
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = TransactionResponse {
        hash: hash_str,
        transaction,
        status,
    };

    Ok(Json(RpcResponse::new(response)))
}

/// GET /v1/blockchain/dag - Get DAG information
pub async fn get_dag_info(
    State(state): State<RpcHandlerState>,
) -> Result<Json<RpcResponse<DagResponse>>, StatusCode> {
    let dag_info = state.service.blockchain().get_dag_info()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = DagResponse {
        tips: dag_info.tips,
        total_blocks: dag_info.total_blocks,
        height: dag_info.height,
        stats: dag_info.stats,
    };

    Ok(Json(RpcResponse::new(response)))
}

/// GET /v1/blockchain/mempool - Get mempool information
pub async fn get_mempool_info(
    State(state): State<RpcHandlerState>,
) -> Result<Json<RpcResponse<MempoolResponse>>, StatusCode> {
    let mempool_info = state.service.blockchain().get_mempool_info()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = MempoolResponse {
        tx_count: mempool_info.tx_count,
        tx_hashes: mempool_info.tx_hashes,
        total_gas: mempool_info.total_gas,
    };

    Ok(Json(RpcResponse::new(response)))
}

/// Helper function to parse address
pub(crate) fn parse_address(addr_str: &str) -> Result<[u8; 20], StatusCode> {
    if !addr_str.starts_with("spt") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let bytes = hex::decode(&addr_str[3..]).map_err(|_| StatusCode::BAD_REQUEST)?;
    if bytes.len() != 20 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

/// Helper function to parse block hash
fn parse_block_hash(hash_str: &str) -> Result<[u8; 32], StatusCode> {
    let bytes = hex::decode(hash_str).map_err(|_| StatusCode::BAD_REQUEST)?;
    if bytes.len() != 32 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

/// Helper function to parse transaction hash
fn parse_tx_hash(hash_str: &str) -> Result<[u8; 32], StatusCode> {
    parse_block_hash(hash_str) // Same format
}
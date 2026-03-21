//! Contract-related RPC handlers
//!
//! This module contains handlers for smart contract operations like
//! deployment, calling, and querying contract information.
//!
//! API Version: v1
//! Base path: /v1/contracts

use crate::rpc::interfaces::*;
use crate::rpc::services::*;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};

/// Contract deployment request
#[derive(Deserialize)]
pub struct DeployContractRequest {
    pub bytecode: String, // hex-encoded
    pub constructor_args: String, // hex-encoded
    pub sender: String, // spt address
}

/// Contract call request
#[derive(Deserialize)]
pub struct CallContractRequest {
    pub address: String, // spt address
    pub method: String,
    pub args: String, // hex-encoded
    pub sender: String, // spt address
}

/// Contract info response
#[derive(Serialize)]
pub struct ContractInfoResponse {
    pub address: String,
    pub bytecode: String,
    pub metadata: serde_json::Value,
    pub deployed_at: u64,
}

/// Contract list response
#[derive(Serialize)]
pub struct ContractListResponse {
    pub contracts: Vec<ContractInfoResponse>,
}

/// Contract call response
#[derive(Serialize)]
pub struct ContractCallResponse {
    pub result: String, // hex-encoded
    pub gas_used: u64,
}

/// POST /v1/contracts/deploy - Deploy contract
pub async fn deploy_contract(
    State(state): State<super::RpcHandlerState>,
    Json(request): Json<DeployContractRequest>,
) -> Result<Json<RpcResponse<serde_json::Value>>, StatusCode> {
    // Parse inputs
    let bytecode = hex::decode(&request.bytecode)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let constructor_args = hex::decode(&request.constructor_args)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let sender = super::blockchain::parse_address(&request.sender)?;

    let contract_address = state.service.contracts().deploy_contract(
        bytecode,
        constructor_args,
        sender,
    ).await.map_err(|e| {
        tracing::error!("Failed to deploy contract: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let response = RpcResponse::new(serde_json::json!({
        "status": "success",
        "contract_address": format!("spt{}", hex::encode(contract_address)),
        "message": "Contract deployed successfully"
    }));

    Ok(Json(response))
}

/// POST /v1/contracts/call - Call contract method
pub async fn call_contract(
    State(state): State<super::RpcHandlerState>,
    Json(request): Json<CallContractRequest>,
) -> Result<Json<RpcResponse<ContractCallResponse>>, StatusCode> {
    // Parse inputs
    let address = super::blockchain::parse_address(&request.address)?;
    let args = hex::decode(&request.args)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let sender = super::blockchain::parse_address(&request.sender)?;

    let result = state.service.contracts().call_contract(
        address,
        request.method,
        args,
        sender,
    ).await.map_err(|e| {
        tracing::error!("Failed to call contract: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let response = ContractCallResponse {
        result: hex::encode(result),
        gas_used: 0, // TODO: track gas usage
    };

    Ok(Json(RpcResponse::new(response)))
}

/// GET /v1/contracts/{address} - Get contract information
pub async fn get_contract_info(
    State(state): State<super::RpcHandlerState>,
    Path(address_str): Path<String>,
) -> Result<Json<RpcResponse<Option<ContractInfoResponse>>>, StatusCode> {
    let address = super::blockchain::parse_address(&address_str)?;

    let contract_info = state.service.contracts().get_contract_info(address)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = contract_info.map(|info| ContractInfoResponse {
        address: address_str,
        bytecode: info.bytecode,
        metadata: info.metadata,
        deployed_at: info.deployed_at,
    });

    Ok(Json(RpcResponse::new(response)))
}

/// GET /v1/contracts - List all contracts
pub async fn list_contracts(
    State(state): State<super::RpcHandlerState>,
) -> Result<Json<RpcResponse<ContractListResponse>>, StatusCode> {
    let contracts = state.service.contracts().list_contracts()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = ContractListResponse {
        contracts: contracts.into_iter().map(|info| ContractInfoResponse {
            address: format!("spt{}", hex::encode(info.address.as_bytes())),
            bytecode: info.bytecode,
            metadata: info.metadata,
            deployed_at: info.deployed_at,
        }).collect(),
    };

    Ok(Json(RpcResponse::new(response)))
}
//! RPC Interface definitions
//!
//! This module defines the interfaces that RPC handlers use to interact
//! with the blockchain core, ensuring loose coupling and API stability.

use crate::core::{Block, BlockHash, Transaction, Address};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Core blockchain interface for RPC operations
#[async_trait]
pub trait BlockchainInterface: Send + Sync {
    /// Submit transaction to mempool
    async fn submit_transaction(&self, tx: Transaction) -> Result<(), RpcError>;

    /// Get account balance
    async fn get_balance(&self, address: Address) -> Result<u64, RpcError>;

    /// Get account nonce
    async fn get_nonce(&self, address: Address) -> Result<u64, RpcError>;

    /// Get block by hash
    async fn get_block(&self, hash: BlockHash) -> Result<Option<Block>, RpcError>;

    /// Get transaction by hash
    async fn get_transaction(&self, hash: [u8; 32]) -> Result<Option<Transaction>, RpcError>;

    /// Get transaction status
    async fn get_transaction_status(&self, hash: [u8; 32]) -> Result<TransactionStatus, RpcError>;

    /// Get DAG information
    async fn get_dag_info(&self) -> Result<DagInfo, RpcError>;

    /// Get node information
    async fn get_node_info(&self) -> Result<NodeInfo, RpcError>;

    /// Get mempool information
    async fn get_mempool_info(&self) -> Result<MempoolInfo, RpcError>;
}

/// Contract interface for RPC operations
#[async_trait]
pub trait ContractInterface: Send + Sync {
    /// Get contract information
    async fn get_contract_info(&self, address: Address) -> Result<Option<ContractInfo>, RpcError>;

    /// List all contracts
    async fn list_contracts(&self) -> Result<Vec<ContractInfo>, RpcError>;

    /// Deploy contract
    async fn deploy_contract(&self, bytecode: Vec<u8>, constructor_args: Vec<u8>, sender: Address) -> Result<Address, RpcError>;

    /// Call contract
    async fn call_contract(&self, address: Address, method: String, args: Vec<u8>, sender: Address) -> Result<Vec<u8>, RpcError>;
}

/// Network interface for RPC operations
#[async_trait]
pub trait NetworkInterface: Send + Sync {
    /// Get connected peers
    async fn get_connected_peers(&self) -> Result<Vec<String>, RpcError>;

    /// Get network stats
    async fn get_network_stats(&self) -> Result<NetworkStats, RpcError>;
}

/// Combined RPC interface
pub trait RpcInterface: BlockchainInterface + ContractInterface + NetworkInterface {}

/// RPC Error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcError {
    InvalidRequest(String),
    NotFound(String),
    InternalError(String),
    ValidationError(String),
    NetworkError(String),
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RpcError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            RpcError::NotFound(msg) => write!(f, "Not found: {}", msg),
            RpcError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            RpcError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            RpcError::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl std::error::Error for RpcError {}

/// Transaction status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Confirmed { block_hash: BlockHash, block_height: u64 },
    Failed(String),
}

/// DAG information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagInfo {
    pub tips: Vec<String>,
    pub total_blocks: usize,
    pub height: u64,
    pub stats: serde_json::Value,
}

/// Node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub peer_id: String,
    pub connected_peers: Vec<String>,
    pub mempool_size: usize,
    pub dag_height: usize,
    pub version: String,
    pub uptime: u64,
}

/// Mempool information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolInfo {
    pub tx_count: usize,
    pub tx_hashes: Vec<String>,
    pub total_gas: u64,
}

/// Contract information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractInfo {
    pub address: String,
    pub bytecode: String,
    pub metadata: serde_json::Value,
    pub deployed_at: u64,
}

/// Network statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub connected_peers: usize,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
}

/// API Version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiVersion {
    pub version: String,
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub features: Vec<String>,
}

impl ApiVersion {
    pub fn current() -> Self {
        ApiVersion {
            version: "v1.0.0".to_string(),
            major: 1,
            minor: 0,
            patch: 0,
            features: vec![
                "blockchain".to_string(),
                "contracts".to_string(),
                "network".to_string(),
                "events".to_string(),
            ],
        }
    }
}

/// RPC Response wrapper with versioning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse<T> {
    pub api_version: String,
    pub timestamp: u64,
    pub request_id: Option<String>,
    pub data: T,
}

impl<T> RpcResponse<T> {
    pub fn new(data: T) -> Self {
        RpcResponse {
            api_version: ApiVersion::current().version,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            request_id: None,
            data,
        }
    }

    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
}
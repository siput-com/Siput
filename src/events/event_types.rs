use crate::core::{Address, Block, BlockHash, Transaction};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Event types that can be subscribed to in the Siput ecosystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    /// New block added to the blockchain
    NewBlock { block: Block, block_height: u64 },
    /// New transaction added to the mempool or confirmed
    NewTransaction {
        transaction: Transaction,
        status: TransactionStatus,
    },
    /// Smart contract emitted an event
    ContractEvent {
        contract_address: Address,
        event_name: String,
        event_data: Vec<u8>,
        block_hash: BlockHash,
        transaction_hash: String,
    },
    /// New peer connected to the network
    PeerConnected { peer_id: String, address: String },
    /// Peer disconnected from the network
    PeerDisconnected {
        peer_id: String,
        reason: Option<String>,
    },
    /// Node status changed
    NodeStatusChanged { status: NodeStatus },
    /// Block created (emitted when block is produced)
    BlockCreated { block: Block, block_height: u64 },
    /// Transaction confirmed in a block
    TransactionConfirmed {
        transaction: Transaction,
        block_hash: BlockHash,
        block_height: u64,
    },
    /// Contract executed successfully
    ContractExecuted {
        contract_address: Address,
        method: String,
        result: Vec<u8>,
        gas_used: u64,
    },
    /// Custom event for extensibility
    Custom {
        event_name: String,
        data: serde_json::Value,
    },
}

/// Transaction status for events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatus {
    /// Transaction added to mempool
    Pending,
    /// Transaction confirmed in block
    Confirmed,
    /// Transaction failed
    Failed,
}

/// Node status for events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Node is starting up
    Starting,
    /// Node is running normally
    Running,
    /// Node is syncing with network
    Syncing,
    /// Node encountered an error
    Error,
    /// Node is shutting down
    ShuttingDown,
}

/// Event wrapper with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event ID
    pub id: String,
    /// Event type
    pub event_type: EventType,
    /// Timestamp when event occurred
    pub timestamp: u64,
    /// Source node ID
    pub source_node: String,
}

impl Event {
    /// Create a new event
    pub fn new(event_type: EventType, source_node: String) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let id = format!("{}_{}", timestamp, uuid::Uuid::new_v4().simple());

        Event {
            id,
            event_type,
            timestamp,
            source_node,
        }
    }

    /// Create a new block event
    pub fn new_block(block: Block, block_height: u64, source_node: String) -> Self {
        Self::new(
            EventType::NewBlock {
                block,
                block_height,
            },
            source_node,
        )
    }

    /// Create a new transaction event
    pub fn new_transaction(
        transaction: Transaction,
        status: TransactionStatus,
        source_node: String,
    ) -> Self {
        Self::new(
            EventType::NewTransaction {
                transaction,
                status,
            },
            source_node,
        )
    }

    /// Create a contract event
    pub fn contract_event(
        contract_address: Address,
        event_name: String,
        event_data: Vec<u8>,
        block_hash: BlockHash,
        transaction_hash: String,
        source_node: String,
    ) -> Self {
        Self::new(
            EventType::ContractEvent {
                contract_address,
                event_name,
                event_data,
                block_hash,
                transaction_hash,
            },
            source_node,
        )
    }

    /// Create a peer connected event
    pub fn peer_connected(peer_id: String, address: String, source_node: String) -> Self {
        Self::new(EventType::PeerConnected { peer_id, address }, source_node)
    }

    /// Create a peer disconnected event
    pub fn peer_disconnected(peer_id: String, reason: Option<String>, source_node: String) -> Self {
        Self::new(EventType::PeerDisconnected { peer_id, reason }, source_node)
    }

    /// Create a node status changed event
    pub fn node_status_changed(status: NodeStatus, source_node: String) -> Self {
        Self::new(EventType::NodeStatusChanged { status }, source_node)
    }

    /// Create a block created event
    pub fn new_block_created(block: Block, block_height: u64, source_node: String) -> Self {
        Self::new(EventType::BlockCreated { block, block_height }, source_node)
    }

    /// Create a transaction confirmed event
    pub fn new_transaction_confirmed(transaction: Transaction, block_hash: BlockHash, source_node: String) -> Self {
        Self::new(EventType::TransactionConfirmed {
            transaction,
            block_hash,
            block_height: 0, // Will be set by caller
        }, source_node)
    }

    /// Create a contract executed event
    pub fn new_contract_executed(contract_address: Address, method: String, result: Vec<u8>, gas_used: u64, source_node: String) -> Self {
        Self::new(EventType::ContractExecuted {
            contract_address,
            method,
            result,
            gas_used,
        }, source_node)
    }

    /// Create a custom event
    pub fn new_custom(event_name: String, data: serde_json::Value, source_node: String) -> Self {
        Self::new(EventType::Custom { event_name, data }, source_node)
    }

    /// Get string representation of event type for routing
    pub fn event_type_string(&self) -> String {
        match &self.event_type {
            EventType::NewBlock { .. } => "new_block".to_string(),
            EventType::NewTransaction { .. } => "new_transaction".to_string(),
            EventType::ContractEvent { .. } => "contract_event".to_string(),
            EventType::PeerConnected { .. } => "peer_connected".to_string(),
            EventType::PeerDisconnected { .. } => "peer_disconnected".to_string(),
            EventType::NodeStatusChanged { .. } => "node_status".to_string(),
            EventType::BlockCreated { .. } => "block_created".to_string(),
            EventType::TransactionConfirmed { .. } => "transaction_confirmed".to_string(),
            EventType::ContractExecuted { .. } => "contract_executed".to_string(),
            EventType::Custom { event_name, .. } => event_name.clone(),
        }
    }
}

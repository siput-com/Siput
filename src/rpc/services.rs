//! RPC Interface implementations
//!
//! This module provides concrete implementations of the RPC interfaces
//! that wrap the core blockchain components.

use crate::contracts::contract_registry::ContractRegistry;
use crate::core::{Block, Transaction, Address};
use crate::dag::blockdag::BlockDAG;
use crate::mempool::tx_dag_mempool::TxDagMempool;
use crate::network::p2p_node::P2PNode;
use crate::state::state_manager::StateManager;
use crate::rpc::interfaces::*;
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

/// Concrete implementation of BlockchainInterface
pub struct BlockchainService {
    dag: Arc<RwLock<BlockDAG>>,
    mempool: Arc<TxDagMempool>,
    state_manager: Arc<Mutex<StateManager>>,
}

impl BlockchainService {
    pub fn new(
        dag: Arc<RwLock<BlockDAG>>,
        mempool: Arc<TxDagMempool>,
        state_manager: Arc<Mutex<StateManager>>,
    ) -> Self {
        BlockchainService {
            dag,
            mempool,
            state_manager,
        }
    }
}

#[async_trait::async_trait]
impl BlockchainInterface for BlockchainService {
    async fn submit_transaction(&self, tx: Transaction) -> Result<(), RpcError> {
        // Validate transaction
        tx.validate_basic().map_err(|e| RpcError::ValidationError(e))?;

        // Add to mempool
        self.mempool.add_transaction(tx, vec![], None)
            .map_err(|e| RpcError::InternalError(format!("Failed to add transaction: {}", e)))
    }

    async fn get_balance(&self, address: Address) -> Result<u64, RpcError> {
        let state = self.state_manager.lock();
        Ok(state.get_balance(address))
    }

    async fn get_nonce(&self, address: Address) -> Result<u64, RpcError> {
        let state = self.state_manager.lock();
        Ok(state.get_nonce(address))
    }

    async fn get_block(&self, hash: [u8; 32]) -> Result<Option<Block>, RpcError> {
        let dag = self.dag.read();
        Ok(dag.get_block(&hash))
    }

    async fn get_transaction(&self, hash: [u8; 32]) -> Result<Option<Transaction>, RpcError> {
        // First search in mempool
        if let Some(mempool_tx) = self.mempool.get_transaction(&hash) {
            return Ok(Some(mempool_tx.transaction.clone()));
        }

        // Search in blocks
        let dag = self.dag.read();
        for block in dag.get_all_blocks() {
            for tx in &block.transactions {
                if tx.hash() == hash {
                    return Ok(Some(tx.clone()));
                }
            }
        }

        Ok(None)
    }

    async fn get_transaction_status(&self, hash: [u8; 32]) -> Result<TransactionStatus, RpcError> {
        // Check mempool first
        if self.mempool.get_transaction(&hash).is_some() {
            return Ok(TransactionStatus::Pending);
        }

        // Search in blocks
        let dag = self.dag.read();
        for block in dag.get_all_blocks() {
            for tx in &block.transactions {
                if tx.hash() == hash {
                    return Ok(TransactionStatus::Confirmed {
                        block_hash: block.hash(),
                        block_height: block.height(),
                    });
                }
            }
        }

        Err(RpcError::NotFound("Transaction not found".to_string()))
    }

    async fn get_dag_info(&self) -> Result<DagInfo, RpcError> {
        let dag = self.dag.read();
        let tips: Vec<String> = dag.get_tips().into_iter().map(|h| hex::encode(h)).collect();
        let total_blocks = dag.get_all_blocks().len();
        let height = dag.get_height();

        let stats = serde_json::json!({
            "tips_count": tips.len(),
            "total_blocks": total_blocks,
            "orphans": 0, // TODO: implement orphan tracking
            "avg_block_time": 5000 // TODO: calculate from actual data
        });

        Ok(DagInfo {
            tips,
            total_blocks,
            height,
            stats,
        })
    }

    async fn get_node_info(&self) -> Result<NodeInfo, RpcError> {
        let mempool_size = self.mempool.tx_count();
        let dag = self.dag.read();
        let dag_height = dag.block_count();

        Ok(NodeInfo {
            peer_id: "unknown".to_string(), // TODO: get from network layer
            connected_peers: vec![], // TODO: get from network layer
            mempool_size,
            dag_height,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime: 0, // TODO: track node uptime
        })
    }

    async fn get_mempool_info(&self) -> Result<MempoolInfo, RpcError> {
        let tx_count = self.mempool.tx_count();
        let tx_hashes: Vec<String> = self.mempool.get_all_transactions()
            .into_iter()
            .map(|tx| hex::encode(tx.hash()))
            .collect();

        // Calculate total gas (simplified)
        let total_gas = self.mempool.get_all_transactions()
            .into_iter()
            .map(|tx| tx.gas_limit)
            .sum();

        Ok(MempoolInfo {
            tx_count,
            tx_hashes,
            total_gas,
        })
    }
}

/// Concrete implementation of ContractInterface
pub struct ContractService {
    contract_registry: Arc<Mutex<ContractRegistry>>,
    blockchain_service: Arc<dyn BlockchainInterface>,
}

impl ContractService {
    pub fn new(
        contract_registry: Arc<Mutex<ContractRegistry>>,
        blockchain_service: Arc<dyn BlockchainInterface>,
    ) -> Self {
        ContractService {
            contract_registry,
            blockchain_service,
        }
    }
}

#[async_trait::async_trait]
impl ContractInterface for ContractService {
    async fn get_contract_info(&self, address: Address) -> Result<Option<ContractInfo>, RpcError> {
        let registry = self.contract_registry.lock();

        // TODO: Implement contract info retrieval
        // For now, return None
        Ok(None)
    }

    async fn list_contracts(&self) -> Result<Vec<ContractInfo>, RpcError> {
        // TODO: Implement contract listing
        Ok(vec![])
    }

    async fn deploy_contract(&self, bytecode: Vec<u8>, constructor_args: Vec<u8>, sender: Address) -> Result<Address, RpcError> {
        // TODO: Implement contract deployment
        Err(RpcError::InternalError("Contract deployment not implemented".to_string()))
    }

    async fn call_contract(&self, address: Address, method: String, args: Vec<u8>, sender: Address) -> Result<Vec<u8>, RpcError> {
        // TODO: Implement contract calling
        Err(RpcError::InternalError("Contract calling not implemented".to_string()))
    }
}

/// Concrete implementation of NetworkInterface
pub struct NetworkService {
    p2p_node: Option<Arc<tokio::sync::RwLock<P2PNode>>>,
}

impl NetworkService {
    pub fn new(p2p_node: Option<Arc<tokio::sync::RwLock<P2PNode>>>) -> Self {
        NetworkService { p2p_node }
    }
}

#[async_trait::async_trait]
impl NetworkInterface for NetworkService {
    async fn get_connected_peers(&self) -> Result<Vec<String>, RpcError> {
        if let Some(node_arc) = &self.p2p_node {
            let node = node_arc.read().await;
            let peers: Vec<String> = node
                .get_connected_peers()
                .into_iter()
                .map(|p| p.to_string())
                .collect();
            Ok(peers)
        } else {
            Ok(vec![])
        }
    }

    async fn get_network_stats(&self) -> Result<NetworkStats, RpcError> {
        // TODO: Implement network statistics
        Ok(NetworkStats {
            connected_peers: 0,
            bytes_sent: 0,
            bytes_received: 0,
            messages_sent: 0,
            messages_received: 0,
        })
    }
}

/// Combined RPC service implementation
pub struct RpcService {
    blockchain: Arc<dyn BlockchainInterface>,
    contracts: Arc<dyn ContractInterface>,
    network: Arc<dyn NetworkInterface>,
}

impl RpcService {
    pub fn new(
        blockchain: Arc<dyn BlockchainInterface>,
        contracts: Arc<dyn ContractInterface>,
        network: Arc<dyn NetworkInterface>,
    ) -> Self {
        RpcService {
            blockchain,
            contracts,
            network,
        }
    }

    pub fn blockchain(&self) -> Arc<dyn BlockchainInterface> {
        self.blockchain.clone()
    }

    pub fn contracts(&self) -> Arc<dyn ContractInterface> {
        self.contracts.clone()
    }

    pub fn network(&self) -> Arc<dyn NetworkInterface> {
        self.network.clone()
    }
}

impl RpcInterface for RpcService {}
use std::sync::Arc;
use parking_lot::{Mutex, RwLock};
use tokio::time::{interval, Duration};

use crate::pipeline::{TransactionPipelineManager, ValidationStage, MempoolStage, ExecutionStage, StateUpdateStage, FinalityStage};
use crate::state::state_manager::StateManager;
use crate::execution::transaction_executor::TransactionExecutor;
use crate::dag::BlockDAG;
use crate::mempool::TxDagMempool;
use crate::core::{Block, Transaction, BlockHash};
use crate::finality::FinalityEngine;

use crate::node::{PeerInfo, SystemMetrics};

use super::{ExecutionService, ConsensusService, NetworkService};

/// Service manager untuk mengelola semua services
pub struct ServiceManager {
    /// Execution service
    pub execution_service: ExecutionService,
    /// Consensus service
    pub consensus_service: ConsensusService,
    /// Network service
    pub network_service: NetworkService,
    /// BlockDAG
    pub blockdag: Arc<RwLock<BlockDAG>>,
    /// Transaction mempool
    pub mempool: Arc<TxDagMempool>,
    /// Block producer
    pub block_producer: Arc<crate::block::BlockProducer>,
    /// Indexers
    pub block_indexer: Arc<crate::indexer::BlockIndexerImpl>,
    pub tx_indexer: Arc<crate::indexer::TransactionIndexerImpl>,
    pub address_indexer: Arc<crate::indexer::AddressIndexerImpl>,
    /// Chain storage
    pub chain_storage: Arc<crate::storage::ChainStorage>,
    /// Transaction pipeline
    pub pipeline: TransactionPipelineManager,
}

impl ServiceManager {
    /// Buat service manager baru
    pub fn new(
        execution_service: ExecutionService,
        consensus_service: ConsensusService,
        network_service: NetworkService,
        blockdag: Arc<RwLock<BlockDAG>>,
        mempool: Arc<TxDagMempool>,
        block_producer: Arc<crate::block::BlockProducer>,
        block_indexer: Arc<crate::indexer::BlockIndexerImpl>,
        tx_indexer: Arc<crate::indexer::TransactionIndexerImpl>,
        address_indexer: Arc<crate::indexer::AddressIndexerImpl>,
        chain_storage: Arc<crate::storage::ChainStorage>,
        state_manager: Arc<parking_lot::Mutex<StateManager>>,
        executor: Arc<parking_lot::Mutex<TransactionExecutor>>,
        finality_engine: Arc<FinalityEngine>,
    ) -> Self {
        let mut pipeline = TransactionPipelineManager::new();

        // Setup default pipeline stages
        pipeline.add_stage(Box::new(ValidationStage::new(state_manager.clone())));
        pipeline.add_stage(Box::new(MempoolStage::new(mempool.clone())));
        pipeline.add_stage(Box::new(ExecutionStage::new(executor.clone())));
        pipeline.add_stage(Box::new(StateUpdateStage::new(state_manager.clone())));
        pipeline.add_stage(Box::new(FinalityStage::new(finality_engine)));

        Self {
            execution_service,
            consensus_service,
            network_service,
            blockdag,
            mempool,
            block_producer,
            block_indexer,
            tx_indexer,
            address_indexer,
            chain_storage,
            pipeline,
        }
    }

    /// Execute block
    pub fn execute_block(&self, block: &Block) -> Result<(), String> {
        self.execution_service.execute_block(block)
    }

    /// Add transaction to mempool
    pub async fn add_transaction(&self, tx: Transaction) -> Result<(), String> {
        // Process through pipeline
        let result = self.pipeline.process_transaction(tx).await?;

        if !result.success {
            return Err(result.error_message.unwrap_or_else(|| "Pipeline processing failed".to_string()));
        }

        tracing::info!("Transaction processed successfully: {:?}", result.transaction_hash);
        Ok(())
    }

    /// Produce block
    pub async fn produce_block(&self) -> Result<(), String> {
        // Simplified block production
        // In full implementation, this would coordinate with consensus service
        Ok(())
    }

    /// Check finality
    pub fn check_finality(&self) -> Result<(), String> {
        let dag = self.blockdag.read().clone();
        self.consensus_service.check_finality(&dag)
    }

    /// Get state root
    pub fn get_state_root(&self) -> [u8; 32] {
        self.execution_service.get_state_root()
    }

    /// Get tips
    pub fn get_tips(&self) -> Vec<BlockHash> {
        self.blockdag.read().get_tips().to_vec()
    }

    /// Get finality height
    pub fn get_finality_height(&self) -> Option<u64> {
        self.consensus_service.get_finality_height()
    }

    /// Get mempool size
    pub fn get_mempool_size(&self) -> usize {
        self.mempool.size()
    }

    /// Get connected peers
    pub fn get_connected_peers(&self) -> Vec<String> {
        self.network_service.get_connected_peers()
    }

    /// Get peer count
    pub fn get_peer_count(&self) -> usize {
        self.network_service.get_peer_count()
    }

    /// Get current height
    pub fn get_current_height(&self) -> u64 {
        self.blockdag
            .read()
            .get_tip_blocks()
            .iter()
            .map(|b| b.header.chain_height)
            .max()
            .unwrap_or(0)
    }

    /// Get hash rate
    pub fn get_hash_rate(&self) -> f64 {
        self.consensus_service.get_hash_rate()
    }

    /// Get balance
    pub fn get_balance(&self, address: &crate::core::transaction::Address) -> u64 {
        self.execution_service.get_balance(address)
    }

    /// Get nonce
    pub fn get_nonce(&self, address: &crate::core::transaction::Address) -> u64 {
        self.execution_service.get_nonce(address)
    }

    /// Start mining
    pub async fn start_mining(&self) -> Result<(), String> {
        self.consensus_service.start_mining(4);
        Ok(())
    }

    /// Stop mining
    pub async fn stop_mining(&self) -> Result<(), String> {
        self.consensus_service.stop_mining();
        Ok(())
    }

    /// Get metrics
    pub async fn get_metrics(&self) -> Result<SystemMetrics, String> {
        let mempool_size = self.get_mempool_size();
        let block_count = self.blockdag.read().get_all_blocks().len();
        let finality_height = self.get_finality_height().unwrap_or(0);

        Ok(SystemMetrics {
            tps: 0.0,
            network_latency_ms: 0.0,
            memory_mb: 0,
            storage_mb: 0,
            mempool_size,
            block_count,
            finality_height,
        })
    }

    /// Validate transaction
    pub async fn validate_transaction(&self, tx_hash: &[u8; 32]) -> Result<bool, String> {
        // Check if transaction exists in mempool or blocks
        if self.mempool.get_transaction(tx_hash).is_some() {
            return Ok(true);
        }

        // Check in block history
        let blocks = self.blockdag.read().get_all_blocks();
        for block in blocks {
            for tx in &block.transactions {
                if &tx.hash() == tx_hash {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    pub async fn get_detailed_peers(&self) -> Result<Vec<PeerInfo>, String> {
        self.network_service.get_detailed_peers().await
    }

    /// Prune storage
    pub async fn prune_storage(&self, _window: usize) -> Result<usize, String> {
        // Simplified
        Ok(_window)
    }
}
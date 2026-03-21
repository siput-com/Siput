use parking_lot::RwLock;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::time::{interval, Duration};

use crate::block::BlockProducer;
use crate::consensus::mining::Consensus;
use crate::consensus::{FeeMarket, FeeConfig, DaaManager, DaaConfig, BlueSetManager, BlueSetConfig, GhostDagManager, GhostDagConfig};
use crate::consensus::mining::MiningController;
use crate::contracts::ContractRegistry;
use crate::core::{BlockHash, Transaction};
use crate::dag::blockdag::BlockDAG;
use crate::execution::transaction_executor::TransactionExecutor;
use crate::finality::FinalityEngine;
use crate::mempool::TxDagMempool;
use crate::network::DiscoveryManager;
use crate::state::state_manager::StateManager;
use crate::storage::{BlockStore, ChainStorage};

use crate::node::services::{ExecutionService, ConsensusService, NetworkService, ServiceManager};

/// Snapshot of DAG state that can be safely used across async boundaries.
///
/// This is intentionally lightweight and only contains primitive values and
/// block hashes so it can be moved into async tasks without carrying locks.
#[derive(Clone, Debug)]
pub struct DagSnapshot {
    pub tip_count: usize,
    pub height: u64,
    pub last_block_timestamp: Option<u64>,
    pub genesis_hash: Option<BlockHash>,
    pub tip_hashes: Vec<BlockHash>,
}

/// Node runtime yang menjalankan blockchain node
///
/// Sekarang hanya sebagai orchestrator yang menggunakan ServiceManager
#[derive(Clone)]
pub struct NodeRuntime {
    /// Service manager
    service_manager: Arc<ServiceManager>,
    /// BlockDAG (still needed for some operations)
    blockdag: Arc<RwLock<BlockDAG>>,
    /// Node ID
    pub node_id: [u8; 20],
    /// Block production interval (ms)
    pub block_interval: u64,
    /// Whether mining is enabled
    pub mining_enabled: Arc<AtomicBool>,
    /// Whether empty blocks are allowed
    pub allow_empty_blocks: bool,
}

impl NodeRuntime {
    /// Buat node runtime baru dengan default settings
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let node_id = [0u8; 20]; // Default node ID
        Self::new_with_config(node_id, 5000, 10, 100, 1000)
    }

    /// Buat node runtime baru dengan konfigurasi
    pub fn new_with_config(
        node_id: [u8; 20],
        block_interval: u64,
        confirmation_depth: usize,
        _max_peers: usize,
        mempool_size: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_config_and_db(
            node_id,
            block_interval,
            confirmation_depth,
            _max_peers,
            mempool_size,
            "./data/contracts.db",
        )
    }

    pub fn new_with_config_and_db(
        node_id: [u8; 20],
        block_interval: u64,
        confirmation_depth: usize,
        _max_peers: usize,
        mempool_size: usize,
        db_path: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let chain_storage = Arc::new(
            ChainStorage::open(db_path)
                .map_err(|e| format!("Failed to open chain storage: {}", e))?,
        );

        let blockdag = Arc::new(RwLock::new(BlockDAG::new()));
        let state_manager = Arc::new(parking_lot::Mutex::new(StateManager::new()));
        let mempool = Arc::new(TxDagMempool::new(
            mempool_size,
            state_manager.clone(),
            crate::mempool::tx_dag_mempool::DEFAULT_MIN_GAS_PRICE,
        ));

        // Derive per-node DB paths
        let executor_db_path = format!("{}-executor", db_path);
        let registry_db_path = format!("{}-registry", db_path);
        let pruner_db_path = format!("{}-pruner", db_path);

        let default_vm = crate::vm::contract_executor::ContractExecutor::new(
            StateManager::new(),
            &format!("{}-registry", &executor_db_path),
            &format!("{}-storage", &executor_db_path),
        )
        .expect("Failed to initialize contract executor");

        let executor = Arc::new(parking_lot::Mutex::new(TransactionExecutor::new_with_vm(
            Arc::new(default_vm),
            &executor_db_path,
        )));
        let pruner = Some(Arc::new(parking_lot::Mutex::new(
            crate::storage::pruning::RollingWindowPruner::new(
                &pruner_db_path,
                BlockStore::new_with_path(&format!("{}-blocks", db_path)).unwrap(),
                100_000,
                1000,
                1000,
            )
            .unwrap(),
        )));

        let contract_registry = Arc::new(parking_lot::Mutex::new(
            ContractRegistry::new(&registry_db_path)
                .map_err(|e| format!("Failed to initialize contract registry: {}", e))?,
        ));

        // Create execution service
        let execution_service = ExecutionService::new(
            state_manager.clone(),
            executor,
            pruner,
            contract_registry,
            chain_storage.clone(),
        );

        // Create consensus components
        let finality_engine = Arc::new(FinalityEngine::new(confirmation_depth));
        let ghostdag = Arc::new(RwLock::new(GhostDagManager::new(
            GhostDagConfig::default(),
            blockdag.clone(),
        )));

        let block_producer = Arc::new(BlockProducer::new(
            mempool.clone(),
            blockdag.clone(),
            state_manager.clone(),
            node_id,
            0,
            100,
            crate::consensus::daa::DaaConfig::default(),
        ));

        // Initialize indexers
        let block_indexer = Arc::new(crate::indexer::BlockIndexer::new(&format!("{}-block-index", db_path))
            .map_err(|e| format!("Failed to initialize block indexer: {:?}", e))?);
        let tx_indexer = Arc::new(crate::indexer::TransactionIndexer::new(&format!("{}-tx-index", db_path))
            .map_err(|e| format!("Failed to initialize transaction indexer: {:?}", e))?);
        let address_indexer = Arc::new(crate::indexer::AddressIndexer::new(&format!("{}-address-index", db_path))
            .map_err(|e| format!("Failed to initialize address indexer: {:?}", e))?);

        // Create consensus wrapper
        let consensus = Arc::new(Consensus {
            blockdag: blockdag.clone(),
            ghostdag: ghostdag.clone(),
            state_manager: state_manager.clone(),
            mempool: mempool.clone(),
            block_producer: block_producer.clone(),
            fee_market: Arc::new(parking_lot::Mutex::new(FeeMarket::new(
                FeeConfig::default(),
                blockdag.clone(),
                mempool.clone(),
            ))),
            daa: Arc::new(parking_lot::Mutex::new(DaaManager::new(
                DaaConfig::default(),
                blockdag.clone(),
                ghostdag.clone(),
                Arc::new(BlueSetManager::new(BlueSetConfig::default(), blockdag.clone())),
            ))),
            block_indexer: block_indexer.clone(),
            tx_indexer: tx_indexer.clone(),
            address_indexer: address_indexer.clone(),
        });

        let mining_controller = Some(Arc::new(MiningController::new(consensus.clone())));

        let consensus_service = ConsensusService::new(
            ghostdag,
            finality_engine,
            mining_controller,
            consensus,
        );

        // Create network service
        let discovery_manager = Arc::new(DiscoveryManager::new(libp2p::PeerId::random()));
        let state_sync_manager = None;

        let network_service = NetworkService::new(
            discovery_manager,
            state_sync_manager,
        );

        // Create service manager
        let service_manager = Arc::new(ServiceManager::new(
            execution_service,
            consensus_service,
            network_service,
            blockdag.clone(),
            mempool,
            block_producer,
            block_indexer,
            tx_indexer,
            address_indexer,
            chain_storage,
            state_manager,
            executor,
            finality_engine,
        ));

        Ok(Self {
            service_manager,
            blockdag,
            node_id,
            block_interval,
            mining_enabled: Arc::new(AtomicBool::new(true)),
            allow_empty_blocks: true,
        })
    }

    /// Initialize node
    pub async fn initialize(&self) -> Result<(), String> {
        tracing::info!("Initializing node...");

        // Load persisted DAG index from storage
        self.blockdag.write().load_from_store();

        // Initialize genesis if needed
        if self.blockdag.read().is_empty() {
            self.blockdag.write().create_genesis_if_empty();

            // Initialize tokenomics
            self.service_manager.execution_service.initialize_tokenomics();

            // Persist initial state
            self.service_manager.execution_service.persist_state(
                &self.service_manager.execution_service.state_manager.lock().clone()
            );

            // Persist tips and height
            let tips = self.blockdag.read().get_tips();
            let _ = self.service_manager.chain_storage.persist_tips(&tips);
            let _ = self.service_manager.chain_storage.put_meta("height", &0u64.to_le_bytes());
        } else {
            // Load persisted state
            if let Some(persisted_state) = self.service_manager.execution_service.load_persisted_state() {
                *self.service_manager.execution_service.state_manager.lock() = persisted_state;
            } else {
                self.service_manager.execution_service.initialize_tokenomics();
                self.service_manager.execution_service.persist_state(
                    &self.service_manager.execution_service.state_manager.lock().clone()
                );
            }
        }

        tracing::info!("✓ Genesis block created and tokenomics initialized");
        tracing::info!("Node initialization complete");
        Ok(())
    }

    /// Main node loop
    pub async fn run(&self) -> Result<(), String> {
        self.initialize().await?;

        // Ensure genesis tip
        self.ensure_genesis_tip_sync()?;
        self.bootstrap_first_block_sync()?;

        // Start mining if enabled
        if self.mining_enabled.load(Ordering::Relaxed) {
            self.service_manager.start_mining().await?;
        }

        let mut finality_timer = interval(Duration::from_millis(1000));

        tracing::info!(
            "Node runtime started. Node ID: {}",
            hex::encode(self.node_id)
        );

        loop {
            tokio::select! {
                _ = finality_timer.tick() => {
                    self.service_manager.check_finality().ok();
                }
            }
        }
    }

    /// Snapshot of DAG information that can be safely moved into async tasks.
    fn get_dag_snapshot(&self) -> DagSnapshot {
        let (tip_count, height, last_block_timestamp, tip_hashes, genesis_hash) = {
            let dag = self.blockdag.read();
            let tip_hashes = dag.get_tips();
            let tip_blocks = dag.get_tip_blocks();

            let tip_count = tip_hashes.len();
            let height = tip_blocks
                .iter()
                .map(|b| b.header.chain_height)
                .max()
                .unwrap_or(0);
            let last_block_timestamp = tip_blocks.iter().map(|b| b.header.timestamp).max();

            let genesis_hash = dag
                .get_topological_order()
                .iter()
                .find_map(|h| dag.get_block(h).filter(|b| b.is_genesis()).map(|b| b.hash));

            (
                tip_count,
                height,
                last_block_timestamp,
                tip_hashes,
                genesis_hash,
            )
        };

        DagSnapshot {
            tip_count,
            height,
            last_block_timestamp,
            genesis_hash,
            tip_hashes,
        }
    }

    /// Ensure the genesis block is activated as a tip (for mining bootstrap)
    fn ensure_genesis_tip_sync(&self) -> Result<(), String> {
        let mut dag = self.blockdag.write();
        if dag.get_tips().is_empty() {
            tracing::warn!("No DAG tips detected; activating genesis tip");
            dag.create_genesis_if_empty();
            dag.rebuild_index();

            // Sync consensus components
            let dag_snapshot = dag.clone();
            drop(dag);
            self.service_manager.consensus_service.attach_dag(dag_snapshot);
            if let Err(e) = self.service_manager.consensus_service.generate_ordering() {
                tracing::warn!("Failed to generate ordering during genesis activation: {}", e);
            }
        }
        Ok(())
    }

    /// Produce a bootstrap block if the DAG height is still at 0.
    fn bootstrap_first_block_sync(&self) -> Result<(), String> {
        let height = {
            let dag = self.blockdag.read();
            dag.get_tip_blocks()
                .iter()
                .map(|b| b.header.chain_height)
                .max()
                .unwrap_or(0)
        };

        if height > 0 {
            return Ok(());
        }

        tracing::warn!("Bootstrapping first block on empty DAG");

        let difficulty = 0; // Use 0 for bootstrap
        let dag_snapshot = self.get_dag_snapshot();
        let base_fee = crate::consensus::next_base_fee(&self.blockdag.read());
        let state_root = self.service_manager.get_state_root();

        let block = self.service_manager.block_producer.create_block_with_snapshot(
            difficulty,
            base_fee,
            state_root,
            (dag_snapshot.height + 1) as f64,
            vec![],
            dag_snapshot.tip_hashes.clone(),
            dag_snapshot.genesis_hash,
        )?;

        // Insert into DAG
        {
            let mut dag = self.blockdag.write();
            dag.insert_block(block.clone())?;
        }

        // Update consensus
        let dag_snapshot2 = self.blockdag.read().clone();
        self.service_manager.consensus_service.attach_dag(dag_snapshot2);

        // Execute block
        self.execute_block(&block)?;

        Ok(())
    }

    /// Enable or disable mining
    pub fn set_mining(&self, enabled: bool) {
        let was_enabled = self.mining_enabled.swap(enabled, Ordering::Relaxed);

        if enabled && !was_enabled {
            if let Some(mining_controller) = &self.mining_controller {
                mining_controller.start(4);
                tracing::info!("Mining enabled");
            }
        } else if !enabled && was_enabled {
            if let Some(mining_controller) = &self.mining_controller {
                mining_controller.stop();
                tracing::info!("Mining disabled");
            }
        }
    }

    /// Produce block periodically
    #[allow(dead_code)]
    async fn produce_block(&self) -> Result<(), String> {
        // Only produce blocks when mining is enabled
        if !self.mining_enabled.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Allow empty blocks to be produced even when mempool is empty if configured
        if (*self.mempool).size() == 0 && !self.allow_empty_blocks {
            return Ok(());
        }

        tracing::debug!("Attempting to produce block...");

        // Snapshot DAG state early so we don't hold locks across async boundaries.
        let dag_snapshot = self.get_dag_snapshot();

        // Determine difficulty and base fee using current DAG state (locks are short-lived).
        let difficulty = {
            let dag = self.blockdag.read();
            crate::consensus::next_difficulty(&dag)
        };

        let base_fee = {
            let dag = self.blockdag.read();
            crate::consensus::next_base_fee(&dag)
        };

        // update mempool with current base fee so it can enforce and order
        self.mempool.set_base_fee(base_fee);
        let state_root = self.state_manager.lock().get_state_root();

        // Use snapshot information to avoid holding DAG locks while producing a block
        let chain_progress = (dag_snapshot.height + 1) as f64;
        let recent_timestamps = {
            let dag = self.blockdag.read();
            dag.get_recent_timestamps(
                crate::consensus::mining::RewardConfig::default().activity_window,
            )
        };
        let parent_hashes = crate::consensus::select_mining_parents(
            &dag_snapshot.tip_hashes,
            dag_snapshot.genesis_hash,
        );

        match self.block_producer.create_block_with_snapshot(
            difficulty,
            base_fee,
            state_root,
            chain_progress,
            recent_timestamps,
            parent_hashes,
            dag_snapshot.genesis_hash,
        ) {
            Ok(mut block) => {
                tracing::info!(
                    "Block produced: {} with {} transactions",
                    hex::encode(&block.hash[..16]),
                    block.transactions.len()
                );

                // Annotate consensus metadata (chain height, blue score, topo index)
                // based on the current DAG state.
                {
                    let dag_snapshot = self.blockdag.read().clone();
                    let mut ghostdag = self.ghostdag.write();
                    // Ensure ghostdag has the latest DAG snapshot for ordering
                    ghostdag.attach_dag(dag_snapshot.clone());
                    ghostdag.annotate_block(&dag_snapshot, &mut block)?;
                }

                // Add block to DAG
                {
                    let mut dag = self.blockdag.write();
                    dag.insert_block(block.clone())?;
                }

                // Update ghostdag engine with latest DAG state
                let dag_snapshot = self.blockdag.read().clone();
                self.ghostdag.write().attach_dag(dag_snapshot);

                // Remove transaksi dari mempool
                for tx in &block.transactions {
                    let tx_hash_vec = tx.hash();
                    let tx_hash: [u8; 32] = tx_hash_vec
                        .try_into()
                        .map_err(|_| "Invalid hash".to_string())?;
                    (*self.mempool).remove_transaction(&tx_hash);
                }

                // Execute block
                self.execute_block(&block)?;

                Ok(())
            }
            Err(e) => {
                tracing::debug!("Could not produce block: {}", e);
                Ok(())
            }
        }
    }

    /// Execute block dan apply state changes
    fn execute_block(&self, block: &crate::core::Block) -> Result<(), String> {
        self.service_manager.execute_block(block)
    }

    /// Check finality based on the current DAG state
    fn check_finality(&self) -> Result<(), String> {
        self.service_manager.check_finality()
    }

    /// Add transaction ke mempool
    pub async fn add_transaction(&self, tx: Transaction) -> Result<(), String> {
        self.service_manager.add_transaction(tx).await
    }

    /// Get current state root
    pub fn get_state_root(&self) -> [u8; 32] {
        self.service_manager.get_state_root()
    }

    /// Get current tips
    pub fn get_tips(&self) -> Vec<BlockHash> {
        self.service_manager.get_tips()
    }

    /// Get finality status
    pub fn get_finality_height(&self) -> Option<u64> {
        self.service_manager.get_finality_height()
    }

    /// Get mempool size
    pub fn get_mempool_size(&self) -> usize {
        self.service_manager.get_mempool_size()
    }

    /// Get connected peers
    pub fn get_connected_peers(&self) -> Vec<String> {
        self.service_manager.get_connected_peers()
    }

    /// Get peer count
    pub fn get_peer_count(&self) -> usize {
        self.service_manager.get_peer_count()
    }

    /// Get current height
    pub fn get_current_height(&self) -> u64 {
        self.service_manager.get_current_height()
    }

    /// Get connection count
    pub fn get_connection_count(&self) -> usize {
        self.service_manager.get_connection_count()
    }

    /// Get hash rate
    pub fn get_hash_rate(&self) -> f64 {
        self.service_manager.get_hash_rate()
    }

    /// Get balance
    pub fn get_balance(&self, address: &crate::core::transaction::Address) -> u64 {
        self.service_manager.get_balance(address)
    }

    /// Get nonce
    pub fn get_nonce(&self, address: &crate::core::transaction::Address) -> u64 {
        self.service_manager.get_nonce(address)
    }

    /// Start mining
    pub async fn start_mining(&self) -> Result<(), String> {
        self.service_manager.start_mining().await
    }

    /// Stop mining
    pub async fn stop_mining(&self) -> Result<(), String> {
        self.service_manager.stop_mining().await
    }

    /// Get metrics
    pub async fn get_metrics(&self) -> Result<SystemMetrics, String> {
        self.service_manager.get_metrics().await
    }

    /// Validate transaction
    pub async fn validate_transaction(&self, tx_hash: &[u8; 32]) -> Result<bool, String> {
        self.service_manager.validate_transaction(tx_hash).await
    }

    /// Get detailed peers
    pub async fn get_detailed_peers(&self) -> Result<Vec<PeerInfo>, String> {
        self.service_manager.get_detailed_peers().await
    }

    /// Prune storage
    pub async fn prune_storage(&self, window: usize) -> Result<usize, String> {
        self.service_manager.prune_storage(window).await
    }
}

/// Peer information structure
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub id: String,
    pub address: String,
    pub status: String,
}

/// System metrics structure
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub tps: f64,
    pub network_latency_ms: f64,
    pub memory_mb: u64,
    pub storage_mb: u64,
    pub mempool_size: usize,
    pub block_count: usize,
    pub finality_height: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_node_creation() {
        let node = NodeRuntime::new_with_config_and_db(
            [1; 20],
            500,
            3,
            10,
            1000,
            "./data/test_node_create.db",
        )
        .unwrap();
        assert_eq!(node.node_id, [1; 20]);
        assert_eq!(node.block_interval, 500);
    }

    #[tokio::test]
    async fn test_node_initialization() {
        let node = NodeRuntime::new_with_config_and_db(
            [2; 20],
            500,
            3,
            10,
            1000,
            "./data/test_node_init.db",
        )
        .unwrap();
        let result = node.initialize().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_tips() {
        let node = NodeRuntime::new_with_config_and_db(
            [3; 20],
            500,
            3,
            10,
            1000,
            "./data/test_node_tips.db",
        )
        .unwrap();
        node.initialize().await.unwrap();
        let tips = node.get_tips();
        assert!(!tips.is_empty());
    }

    #[tokio::test]
    async fn test_get_mempool_size() {
        let node = NodeRuntime::new_with_config_and_db(
            [4; 20],
            500,
            3,
            10,
            1000,
            "./data/test_node_mempool.db",
        )
        .unwrap();
        assert_eq!(node.get_mempool_size(), 0);
    }

    #[tokio::test]
    async fn test_get_connected_peers() {
        let node = NodeRuntime::new_with_config_and_db(
            [5; 20],
            500,
            3,
            10,
            1000,
            "./data/test_node_peers.db",
        )
        .unwrap();
        let peers = node.get_connected_peers();
        assert_eq!(peers.len(), 0);
    }

    #[tokio::test]
    async fn test_miner_reward_credit() {
        // construct a simple environment and manually execute a block with one tx
        let node = NodeRuntime::new_with_config_and_db(
            [6; 20],
            500,
            3,
            10,
            1000,
            "./data/test_node_miner.db",
        )
        .unwrap();
        node.initialize().await.unwrap();

        // give sender enough balance
        let sender: crate::core::transaction::Address = [9; 20];
        node.state_manager
            .lock()
            .state_tree
            .update_account(sender, crate::state::state_tree::Account::new(1000000, 0));
        let receiver: crate::core::transaction::Address = [8; 20];

        let mut tx =
            crate::core::transaction::Transaction::new_transfer(sender, receiver, 100, 0, 21000, 1);
        tx.sign(&secp256k1::SecretKey::from_slice(&[1; 32]).unwrap())
            .unwrap();

        let state_root = node.state_manager.lock().get_state_root();
        let block = crate::core::Block::new(vec![], 0, vec![tx], 0, 0, 0, node.node_id, state_root);
        // call the private helper which includes miner credit logic
        node.execute_block(&block).unwrap();

        // Miner should receive block reward + transaction fees
        let expected_credit = block.reward.saturating_add(21000);
        let state = node.state_manager.lock();
        let miner_acc = state.get_account(&block.producer).unwrap();
        assert_eq!(miner_acc.balance, expected_credit);
    }

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn test_node_runtime_send_sync() {
        assert_send::<NodeRuntime>();
        assert_sync::<NodeRuntime>();
    }
}

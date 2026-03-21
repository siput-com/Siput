use crate::consensus::{BlueSetManager, DaaManager, FeeMarket, GhostDagManager};
use crate::core::block::BlockHeader;
use crate::core::{Address, BlockHash, Transaction};
use crate::core::transaction::TxPayload;
use crate::dag::blockdag::BlockDAG;
use crate::utils::calculations::{RewardConfig, calculate_expected_block_reward, calculate_merkle_root};
use chrono;
use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Block template for mining
#[derive(Clone, Debug)]
pub struct BlockTemplate {
    pub parents: Vec<BlockHash>,
    pub transactions: Vec<Transaction>,
    pub timestamp: u64,
    pub difficulty: u32,
    pub blue_score: u64,
    pub daa_score: u64,
    pub merkle_root: [u8; 32],
    pub coinbase: Transaction,
    pub state_root: [u8; 32],
    pub producer: [u8; 20],
}

/// Node state containing all consensus components
pub struct NodeState {
    pub dag: Arc<RwLock<BlockDAG>>,
    pub ghostdag: GhostDagManager,
    pub blue_set: BlueSetManager,
    pub daa: DaaManager,
    pub fee_market: FeeMarket,
    pub reachability: ReachabilityManager,
    pub utxo: UtxoManager,
    pub events: EventEmitter,
    pub miner_address: Address,
}

// Placeholder structs - assume they exist
pub struct ReachabilityManager;
pub struct UtxoManager;
pub struct EventEmitter;

impl ReachabilityManager {
    pub fn update(&self, _hash: &BlockHash) -> Result<(), String> {
        Ok(())
    }
}

impl UtxoManager {
    pub fn reward_miner(&self, _hash: &BlockHash, _score: u64) -> Result<(), String> {
        Ok(())
    }
}

impl EventEmitter {
    pub fn emit_new_block(&self, _block: &dyn crate::core::ConsensusBlock) {}
}

impl BlueSetManager {
    pub fn select_merge_parents(&self, _parent: &BlockHash, _max: usize) -> Vec<BlockHash> {
        vec![]
    }
    pub fn update_cache(&self, _hash: &BlockHash) -> Result<(), String> {
        Ok(())
    }
}

/// Mining Service for idle block production
pub struct MiningService {
    pub state: Arc<RwLock<NodeState>>,
    pub consensus: Arc<Consensus>,
    pub consensus_engine: Arc<dyn crate::consensus::ConsensusEngine>,
    pub stop_flag: Arc<AtomicBool>,
    pub mining_threads: usize,
    pub block_interval: Duration,
    pub miner_address: Address,
    pub max_merge_parents: usize,
    pub allow_empty_blocks: bool,
}

impl MiningService {
    pub fn new(
        state: Arc<RwLock<NodeState>>,
        consensus: Arc<Consensus>,
        consensus_engine: Arc<dyn crate::consensus::ConsensusEngine>,
        mining_threads: usize,
        block_interval: Duration,
        miner_address: Address,
        max_merge_parents: usize,
        allow_empty_blocks: bool,
    ) -> Self {
        Self {
            state,
            consensus,
            consensus_engine,
            stop_flag: Arc::new(AtomicBool::new(false)),
            mining_threads,
            block_interval,
            miner_address,
            max_merge_parents,
            allow_empty_blocks,
        }
    }

    /// Start mining service
    pub fn start(&self) {
        info!(
            "Starting mining service with {} threads",
            self.mining_threads
        );
        self.stop_flag.store(false, Ordering::SeqCst);

        let (block_tx, block_rx) = mpsc::unbounded_channel();

        // Start commit thread
        let consensus_clone = self.consensus.clone();
        let stop_flag_clone = self.stop_flag.clone();
        tokio::task::spawn_blocking(move || {
            Self::commit_thread(consensus_clone, block_rx, stop_flag_clone);
        });

        // Start mining worker threads
        for i in 0..self.mining_threads {
            let state_clone = self.state.clone();
            let consensus_clone = self.consensus.clone();
            let consensus_engine_clone = self.consensus_engine.clone();
            let block_tx_clone = block_tx.clone();
            let stop_flag_clone = self.stop_flag.clone();
            let max_merge_parents = self.max_merge_parents;
            let allow_empty_blocks = self.allow_empty_blocks;
            let block_interval = self.block_interval;

            thread::spawn(move || {
                info!("Mining worker {} started", i);
                Self::mining_worker(
                    i,
                    consensus_clone,
                    consensus_engine_clone,
                    block_tx_clone,
                    stop_flag_clone,
                    max_merge_parents,
                    allow_empty_blocks,
                    block_interval,
                );
            });
        }
    }

    /// Stop mining service
    pub fn stop(&self) {
        info!("Stopping mining service");
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Mining worker thread
    fn mining_worker(
        id: usize,
        consensus: Arc<Consensus>,
        consensus_engine: Arc<dyn crate::consensus::ConsensusEngine>,
        block_tx: mpsc::UnboundedSender<crate::core::Block>,
        stop_flag: Arc<AtomicBool>,
        max_merge_parents: usize,
        allow_empty_blocks: bool,
        block_interval: Duration,
    ) {
        let mut last_template_build = Instant::now();
        let mut current_template: Option<BlockTemplate> = None;

        loop {
            if stop_flag.load(Ordering::SeqCst) {
                break;
            }

            // Rebuild template if needed
            let rebuild_interval = if current_template
                .as_ref()
                .map_or(true, |t| t.transactions.is_empty())
            {
                // For empty blocks, use block_interval to prevent spam
                block_interval
            } else {
                // For blocks with transactions, rebuild more frequently
                Duration::from_secs(30)
            };

            if current_template.is_none() || last_template_build.elapsed() > rebuild_interval {
                match build_block_template(&consensus, max_merge_parents, allow_empty_blocks) {
                    Ok(template) => {
                        if template.transactions.is_empty() {
                            info!(
                                "Idle mining: producing empty block with parent={} blue={}",
                                hex::encode(template.parents[0]),
                                template.blue_score
                            );
                        } else {
                            info!(
                                "Template built parent={} blue={} with {} transactions",
                                hex::encode(template.parents[0]),
                                template.blue_score,
                                template.transactions.len()
                            );
                        }
                        current_template = Some(template);
                        last_template_build = Instant::now();
                    }
                    Err(e) => {
                        warn!("Failed to build template: {}", e);
                        thread::sleep(Duration::from_secs(1));
                        continue;
                    }
                }
            }

            if let Some(template) = &current_template {
                // Clone template for this worker
                let template_clone = template.clone();

                // Randomize nonce start
                let nonce_start =
                    (id as u64 * 1000000) + (Instant::now().elapsed().as_nanos() % 1000000) as u64;

                // PoW loop
                let mut nonce = nonce_start;
                loop {
                    if stop_flag.load(Ordering::SeqCst) {
                        return;
                    }

                    // Calculate hash
                    let mut hasher = Sha256::new();
                    hasher.update(&template_clone.merkle_root);
                    hasher.update(&template_clone.timestamp.to_le_bytes());
                    hasher.update(&nonce.to_le_bytes());
                    let hash: [u8; 32] = hasher.finalize().into();

                    // Check difficulty
                    if meets_difficulty(&hash, template_clone.difficulty) {
                        // Found valid block
                        let block = crate::core::Block {
                            hash,
                            header: BlockHeader {
                                parent_hashes: template_clone.parents.clone(),
                                timestamp: template_clone.timestamp,
                                nonce,
                                difficulty: template_clone.difficulty,
                                base_fee: consensus.fee_market.lock().get_base_fee(),
                                state_root: template_clone.state_root,
                                version: 1,
                                blue_score: template_clone.blue_score,
                                selected_parent: Some(template_clone.parents[0]),
                                chain_height: template_clone.blue_score,
                                topo_index: template_clone.daa_score,
                            },
                            transactions: template_clone.transactions.clone(),
                            producer: template_clone.producer,
                            reward: match &template_clone.coinbase.payload {
                                TxPayload::Coinbase { amount, .. } => *amount,
                                _ => 0,
                            },
                            height: template_clone.blue_score,
                        };

                        if block_tx.send(block).is_ok() {
                            info!(
                                "Block mined hash={} height={}",
                                hex::encode(hash),
                                template_clone.blue_score
                            );
                        }
                        break;
                    }

                    nonce += 1;

                    // Check if template is stale
                    if Self::is_template_stale(&consensus, consensus_engine.as_ref(), &template_clone) {
                        current_template = None;
                        break;
                    }
                }
            } else {
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    /// Commit thread for safe block insertion
    fn commit_thread(
        consensus: Arc<Consensus>,
        mut block_rx: mpsc::UnboundedReceiver<crate::core::Block>,
        stop_flag: Arc<AtomicBool>,
    ) {
        while !stop_flag.load(Ordering::SeqCst) {
            match block_rx.blocking_recv() {
                Some(block) => {
                    // Safe commit phase

                    // Validate parent still valid
                    if !Self::validate_parent(&consensus, &block.header.parent_hashes[0]) {
                        warn!("Template invalidated - parent changed");
                        continue;
                    }

                    // Insert block into DAG
                    if let Err(e) = consensus.blockdag.write().insert_block(block.clone()) {
                        error!("Failed to insert block: {}", e);
                        continue;
                    }

                    // Update DAA window
                    if let Err(e) = consensus.daa.lock().update_window(block.header.timestamp, block.header.blue_score) {
                        error!("Failed to update DAA: {}", e);
                        continue;
                    }

                    // Emit new block event - TODO: add event system
                    // consensus.events.emit_new_block(block.clone());

                    // Index the new block
                    if let Err(e) = consensus.block_indexer.index_block(&block) {
                        error!("Failed to index block: {:?}", e);
                    }
                    for tx in &block.transactions {
                        if let Err(e) = consensus.tx_indexer.index_transaction(tx, Some(&block.hash)) {
                            error!("Failed to index transaction: {:?}", e);
                        }
                        if let Err(e) = consensus.address_indexer.index_address_from_transaction(tx, block.header.timestamp) {
                            error!("Failed to index address: {:?}", e);
                        }
                    }

                    info!("DAG tip updated to {}", hex::encode(block.hash));
                }
                None => break,
            }
        }
    }

    /// Check if template is stale using consensus engine abstraction
    fn is_template_stale(
        consensus: &Consensus,
        engine: &dyn crate::consensus::ConsensusEngine,
        template: &BlockTemplate,
    ) -> bool {
        // Check if virtual selected parent changed
        let current_virtual = engine.get_virtual_selected_parent();
        if current_virtual != template.parents[0] {
            return true;
        }

        // Check if difficulty changed
        let current_difficulty = consensus.daa.lock().get_current_difficulty();
        if current_difficulty != template.difficulty {
            return true;
        }

        // Check blue score gap
        let current_blue_score = engine.get_virtual_blue_score();
        if current_blue_score.saturating_sub(template.blue_score) > 10 {
            return true;
        }

        false
    }

    /// Validate parent is still valid
    fn validate_parent(_consensus: &Consensus, _parent: &BlockHash) -> bool {
        true // Placeholder - always valid for now
    }
}

/// Build deterministic block template
fn build_block_template(
    consensus: &Consensus,
    max_merge_parents: usize,
    allow_empty_blocks: bool,
) -> Result<BlockTemplate, String> {
    // Get virtual selected parent from ghostdag
    let virtual_parent = consensus.ghostdag.read().get_virtual_selected_parent();

    // Select merge parents with bounded merge depth
    // TODO: implement blue set manager
    let merge_parents = vec![]; // Placeholder
    let parents = std::iter::once(virtual_parent)
        .chain(merge_parents)
        .collect::<Vec<_>>();

    // Calculate difficulty target from DAA
    let difficulty = consensus.daa.lock().get_current_difficulty();

    // Calculate monotonic timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Calculate blue_score = parent.blue_score + 1
    let parent_blue_score = consensus
        .ghostdag
        .read()
        .get_blue_score(&virtual_parent)
        .unwrap_or(0);
    let blue_score = parent_blue_score + 1;

    // Calculate DAA score
    let daa_score = consensus.daa.lock().get_current_daa_score() + 1;

    // Get transactions from mempool
    let mempool_txs = consensus.mempool.get_ready_transactions();
    let transactions: Vec<Transaction> = mempool_txs.into_iter().map(|m| m.transaction).collect();

    // Allow empty blocks if configured and no transactions available
    if transactions.is_empty() && !allow_empty_blocks {
        return Err("No transactions available and empty blocks not allowed".to_string());
    }

    // Build coinbase transaction
    let reward = consensus.daa.lock().calculate_block_reward(blue_score);
    let miner_address = consensus.block_producer.producer_id;
    let coinbase = build_coinbase(miner_address, reward, blue_score)?;

    // Get state root from state manager
    let state_root = consensus.state_manager.lock().get_state_root();

    // Calculate merkle root
    let mut hashes = vec![coinbase.hash()];
    hashes.extend(transactions.iter().map(|tx: &Transaction| tx.hash()));

    let merkle_root = calculate_merkle_root(&hashes);

    Ok(BlockTemplate {
        parents,
        transactions,
        timestamp,
        difficulty,
        blue_score,
        daa_score,
        merkle_root,
        coinbase,
        state_root,
        producer: miner_address,
    })
}

/// Build coinbase transaction
fn build_coinbase(
    miner_address: Address,
    reward: u64,
    blue_score: u64,
) -> Result<Transaction, String> {
    // Use the existing new_coinbase method
    Ok(Transaction::new_coinbase(
        miner_address,
        reward,
        blue_score,
        blue_score,
    ))
}

/// Mining configuration
#[derive(Clone, Debug)]
pub struct MiningConfig {
    pub miner_address: Address,
    pub num_workers: usize,
    pub template_refresh_interval: Duration,
    pub min_block_interval: Duration,
    pub max_stale_attempts: usize,
    pub target_block_time: u64,
    pub emission_schedule: EmissionSchedule,
}

/// Emission schedule for mining rewards
#[derive(Clone, Debug)]
pub struct EmissionSchedule {
    pub initial_reward: u64,
    pub minimum_reward: u64,
    pub decay_factor: f64,
    pub total_supply: u64,
    pub genesis_allocation: u64,
}

impl Default for EmissionSchedule {
    fn default() -> Self {
        Self {
            initial_reward: 100,
            minimum_reward: 1,
            decay_factor: 0.000001,
            total_supply: 600_000_000,
            genesis_allocation: 100_000_000,
        }
    }
}

/// Mining template for block construction
#[derive(Clone, Debug)]
pub struct MiningTemplate {
    pub parents: Vec<BlockHash>,
    pub transactions: Vec<Transaction>,
    pub timestamp: u64,
    pub difficulty: u32,
    pub base_fee: u64,
    pub merkle_root: [u8; 32],
    pub coinbase_tx: Transaction,
    pub reward: u64,
    pub fee_total: u64,
    pub created_at: Instant,
}

/// Mining worker for parallel PoW
#[derive(Debug, Clone)]
pub struct MiningWorker {
    pub id: usize,
    pub is_active: Arc<AtomicBool>,
    pub template: Arc<RwLock<Option<MiningTemplate>>>,
    pub result_tx: mpsc::UnboundedSender<(usize, crate::core::Block)>,
    pub miner_address: Address,
}

/// Mining state machine
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MiningState {
    Idle,
    BuildingTemplate,
    Hashing,
    BlockFound,
    Broadcasting,
    RestartMining,
}

/// Mining metrics
#[derive(Clone, Debug)]
pub struct MiningMetrics {
    pub hashrate: f64,
    pub block_time_avg: f64,
    pub stale_rate: f64,
    pub blocks_found: u64,
    pub total_attempts: u64,
    pub last_block_time: Option<Instant>,
}

/// Main mining manager (legacy, kept for compatibility)
pub struct MiningManager {
    _config: MiningConfig,
    // Simplified for compatibility
}

impl MiningManager {
    pub fn new(config: MiningConfig) -> Self {
        MiningManager { _config: config }
    }
}

/// Check whether a hash satisfies the given difficulty expressed as leading-zero bits.
/// A difficulty of 0 always returns true (no PoW requirement).
/// Select mining parents for block production.
///
/// Ensures a non-empty parent set even if the DAG has no active tips.
///
/// This helper is intentionally DAG-agnostic so callers can provide a
/// lightweight snapshot (tip hashes + optional genesis) and avoid holding
/// locks across await points.
pub fn select_mining_parents(
    tip_hashes: &[BlockHash],
    genesis_hash: Option<BlockHash>,
) -> Vec<BlockHash> {
    if !tip_hashes.is_empty() {
        return tip_hashes.to_vec();
    }

    tracing::warn!("No DAG tips found, falling back to genesis");
    if let Some(genesis) = genesis_hash {
        return vec![genesis];
    }

    Vec::new()
}

pub fn meets_difficulty(hash: &BlockHash, difficulty: u32) -> bool {
    if difficulty == 0 {
        return true;
    }

    let zero_bytes = (difficulty / 8) as usize;
    if hash.iter().take(zero_bytes).any(|&b| b != 0) {
        return false;
    }

    let rem_bits = (difficulty % 8) as u8;
    if rem_bits > 0 {
        let mask: u8 = 0xFF << (8 - rem_bits);
        if hash.get(zero_bytes).map_or(true, |&b| b & mask != 0) {
            return false;
        }
    }

    true
}

/// Consensus components for mining
#[derive(Clone)]
pub struct Consensus {
    pub blockdag: Arc<RwLock<crate::dag::blockdag::BlockDAG>>,
    pub ghostdag: Arc<RwLock<crate::consensus::ghostdag::GhostDagManager>>,
    pub state_manager: Arc<parking_lot::Mutex<crate::state::state_manager::StateManager>>,
    pub mempool: Arc<crate::mempool::TxDagMempool>,
    pub block_producer: Arc<crate::block::BlockProducer>,
    pub fee_market: Arc<parking_lot::Mutex<FeeMarket>>,
    pub daa: Arc<parking_lot::Mutex<DaaManager>>,
    pub block_indexer: Arc<crate::indexer::BlockIndexerImpl>,
    pub tx_indexer: Arc<crate::indexer::TransactionIndexerImpl>,
    pub address_indexer: Arc<crate::indexer::AddressIndexerImpl>,
}

/// Real continuous PoW mining controller
pub struct MiningController {
    running: Arc<AtomicBool>,
    workers: std::sync::Mutex<Vec<std::thread::JoinHandle<()>>>,
    template: Arc<RwLock<BlockTemplate>>,
    template_changed: Arc<AtomicBool>,
    consensus: Arc<Consensus>,
    num_workers: Arc<AtomicUsize>,
}

impl MiningController {
    pub fn new(consensus: Arc<Consensus>) -> Self {
        let initial_template = BlockTemplate {
            parents: vec![],
            transactions: vec![],
            timestamp: 0,
            difficulty: 1,
            blue_score: 0,
            daa_score: 0,
            merkle_root: [0u8; 32],
            coinbase: Transaction::new_transfer([0u8; 20], [0u8; 20], 0, 0, 0, 0),
            state_root: [0u8; 32],
            producer: [0u8; 20],
        };

        Self {
            running: Arc::new(AtomicBool::new(false)),
            workers: std::sync::Mutex::new(vec![]),
            template: Arc::new(RwLock::new(initial_template)),
            template_changed: Arc::new(AtomicBool::new(false)),
            consensus,
            num_workers: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Start continuous mining with N threads
    pub fn start(&self, threads: usize) {
        if self.running.load(Ordering::SeqCst) {
            warn!("Mining already running");
            return;
        }

        info!("Starting continuous PoW mining with {} threads", threads);
        self.running.store(true, Ordering::SeqCst);

        // Spawn template refresh thread
        let consensus_clone = self.consensus.clone();
        let template_clone = self.template.clone();
        let template_changed_clone = self.template_changed.clone();
        let running_clone = self.running.clone();

        thread::spawn(move || {
            Self::template_refresh_worker(
                consensus_clone,
                template_clone,
                template_changed_clone,
                running_clone,
            );
        });

        // Spawn hash workers
        self.num_workers.store(threads, Ordering::Relaxed);

        for i in 0..threads {
            let consensus_clone = self.consensus.clone();
            let template_clone = self.template.clone();
            let template_changed_clone = self.template_changed.clone();
            let running_clone = self.running.clone();

            let handle = thread::spawn(move || {
                Self::hash_worker(
                    i,
                    consensus_clone,
                    template_clone,
                    template_changed_clone,
                    running_clone,
                );
            });

            self.workers.lock().unwrap().push(handle);
        }
    }

    /// Stop mining
    pub fn stop(&self) {
        if !self.running.load(Ordering::SeqCst) {
            return;
        }

        info!("Stopping continuous PoW mining");
        self.running.store(false, Ordering::SeqCst);
        self.num_workers.store(0, Ordering::Relaxed);

        // Wait for workers to finish
        let mut workers = self.workers.lock().unwrap();
        for handle in workers.drain(..) {
            let _ = handle.join();
        }
    }

    /// Estimated hashrate based on active worker count
    pub fn get_hash_rate(&self) -> f64 {
        let workers = self.num_workers.load(Ordering::Relaxed);
        // Estimate 15 MH/s per worker (approx per local thread in this simple PoW simulation)
        (workers as f64) * 15_000_000.0
    }

    /// Template refresh worker (runs every 1 second)
    fn template_refresh_worker(
        consensus: Arc<Consensus>,
        template: Arc<RwLock<BlockTemplate>>,
        template_changed: Arc<AtomicBool>,
        running: Arc<AtomicBool>,
    ) {
        while running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(1));

            // Take DAG snapshot
            let (tip_hashes, height, recent_timestamps, genesis_hash) = {
                let dag = consensus.blockdag.read();
                let tip_hashes = dag.get_tips();
                let height = dag
                    .get_tip_blocks()
                    .iter()
                    .map(|b| b.header.chain_height)
                    .max()
                    .unwrap_or(0);
                let recent_timestamps =
                    dag.get_recent_timestamps(RewardConfig::default().activity_window);
                let genesis_hash = dag
                    .get_topological_order()
                    .iter()
                    .find_map(|h| dag.get_block(h).filter(|b| b.is_genesis()).map(|b| b.hash));
                (tip_hashes, height, recent_timestamps, genesis_hash)
            };

            // Select parents
            let parents = select_mining_parents(&tip_hashes, genesis_hash);

            // Compute difficulty
            let difficulty = {
                let dag = consensus.blockdag.read();
                crate::consensus::daa::next_difficulty(&dag)
            };

            // Get base fee
            let _base_fee = {
                let dag = consensus.blockdag.read();
                crate::consensus::fee_market::next_base_fee(&dag)
            };

            // Get transactions from mempool
            let mempool_txs = consensus.mempool.get_ready_transactions();

            // Convert MempoolTransaction to Transaction
            let transactions: Vec<Transaction> =
                mempool_txs.into_iter().map(|m| m.transaction).collect();

            // Calculate reward
            let emitted_supply = consensus.state_manager.lock().get_emitted_supply();
            let chain_progress = (height + 1) as f64;
            let reward = calculate_expected_block_reward(
                emitted_supply,
                chain_progress,
                &recent_timestamps,
                transactions.len(),
                &RewardConfig::default(),
            );

            // Create coinbase
            let producer = consensus.block_producer.producer_id;
            let coinbase = Transaction::new_transfer(
                [0u8; 20], // burn
                producer, reward, 0, 0, 0,
            );

            // State root
            let state_root = consensus.state_manager.lock().get_state_root();

            // Timestamp
            let timestamp = chrono::Utc::now().timestamp() as u64;

            // Build merkle root (simplified, just hash coinbase + txs)
            let mut hasher = Sha256::new();
            hasher.update(&coinbase.hash());
            for tx in &transactions {
                hasher.update(&tx.hash());
            }
            let merkle_root = hasher.finalize().into();

            // Create new template
            let new_template = BlockTemplate {
                parents,
                transactions,
                timestamp,
                difficulty,
                blue_score: height as u64 + 1,
                daa_score: height as u64 + 1,
                merkle_root,
                coinbase,
                state_root,
                producer,
            };

            let tx_count = new_template.transactions.len();

            // Update template
            *template.write() = new_template;
            template_changed.store(true, Ordering::SeqCst);

            debug!(
                "Template updated with {} transactions, difficulty {}",
                tx_count, difficulty
            );
        }
    }

    /// Hash worker thread
    fn hash_worker(
        id: usize,
        consensus: Arc<Consensus>,
        template: Arc<RwLock<BlockTemplate>>,
        template_changed: Arc<AtomicBool>,
        running: Arc<AtomicBool>,
    ) {
        debug!("Hash worker {} started", id);

        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            // Clone template snapshot
            let template_snapshot = template.read().clone();

            // If no parents (empty DAG), sleep and continue
            if template_snapshot.parents.is_empty() {
                std::thread::sleep(std::time::Duration::from_millis(1000));
                continue;
            }
            template_changed.store(false, Ordering::SeqCst);

            // Create local header
            let selected_parent = template_snapshot.parents[0];
            let height = consensus
                .blockdag
                .read()
                .get_block_height(&selected_parent)
                .unwrap_or(0);
            let chain_height = height + 1;

            let mut header = BlockHeader {
                parent_hashes: template_snapshot.parents.clone(),
                timestamp: template_snapshot.timestamp,
                nonce: 0,
                difficulty: template_snapshot.difficulty,
                base_fee: 0,
                state_root: template_snapshot.state_root,
                version: 1,
                blue_score: template_snapshot.blue_score,
                selected_parent: Some(selected_parent),
                chain_height,
                topo_index: 0, // Not used in new template
            };

            // Nonce loop
            for nonce in 0..u64::MAX {
                if !running.load(Ordering::SeqCst) {
                    return;
                }

                if template_changed.load(Ordering::SeqCst) {
                    debug!("Template changed, interrupting worker {}", id);
                    break;
                }

                header.nonce = nonce;
                let hash = Self::header_hash(&template_snapshot, nonce, &consensus);

                if meets_difficulty(&hash, header.difficulty) {
                    info!("Block found by worker {} at nonce {}", id, nonce);
                    Self::submit_block(&consensus, &header, &template_snapshot);
                    break;
                }
            }
        }

        debug!("Hash worker {} stopped", id);
    }

    /// Compute hash of block for PoW
    fn header_hash(template: &BlockTemplate, nonce: u64, consensus: &Consensus) -> BlockHash {
        let selected_parent = template.parents[0];
        let height = consensus
            .blockdag
            .read()
            .get_block_height(&selected_parent)
            .unwrap_or(0);
        let chain_height = height + 1;

        let header = BlockHeader {
            parent_hashes: template.parents.clone(),
            timestamp: template.timestamp,
            nonce,
            difficulty: template.difficulty,
            base_fee: 0,
            state_root: template.state_root,
            version: 1,
            blue_score: template.blue_score,
            selected_parent: Some(selected_parent),
            chain_height,
            topo_index: 0,
        };

        let mut hasher = Sha256::new();
        for p in &header.parent_hashes {
            hasher.update(p);
        }
        hasher.update(&header.timestamp.to_le_bytes());
        hasher.update(&header.nonce.to_le_bytes());
        hasher.update(&header.difficulty.to_le_bytes());
        hasher.update(&header.base_fee.to_le_bytes());
        hasher.update(&header.state_root);
        hasher.update(&header.version.to_le_bytes());
        hasher.update(&header.blue_score.to_le_bytes());
        if let Some(h) = &header.selected_parent {
            hasher.update(h);
        } else {
            hasher.update(&[0u8; 32]);
        }
        hasher.update(&header.chain_height.to_le_bytes());
        hasher.update(&header.topo_index.to_le_bytes());
        hasher.update(&template.merkle_root);
        hasher.update(&template.producer);
        hasher.update(
            &std::iter::once(template.coinbase.hash())
                .chain(template.transactions.iter().map(|t| t.hash()))
                .collect::<Vec<_>>()
                .concat(),
        );

        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Submit a found block
    fn submit_block(consensus: &Consensus, header: &BlockHeader, template: &BlockTemplate) {
        // Reconstruct full block
        let mut transactions = vec![template.coinbase.clone()];
        transactions.extend(template.transactions.clone());

        let mut block = crate::core::Block::new(
            header.parent_hashes.clone(),
            header.timestamp,
            transactions,
            header.nonce,
            header.difficulty,
            0, // base_fee
            template.producer,
            header.state_root,
        );

        // Set consensus metadata
        block.header.selected_parent = header.selected_parent;
        block.header.blue_score = header.blue_score;
        block.header.chain_height = header.chain_height;
        block.header.topo_index = header.topo_index;
        block.hash = block.calculate_hash();

        // Final PoW verify
        if let Err(e) = block.validate_basic() {
            error!("Invalid block generated: {}", e);
            return;
        }

        // Insert into DAG
        {
            let mut dag = consensus.blockdag.write();
            if let Err(e) = dag.insert_block(block.clone()) {
                error!("Failed to insert block: {}", e);
                return;
            }
        }

        // Update GhostDAG
        {
            let _dag_snapshot = consensus.blockdag.read().clone();
            let mut ghostdag = consensus.ghostdag.write();
            if let Err(e) = ghostdag.generate_ordering() {
                warn!("Failed to update GhostDAG: {}", e);
            }
        }

        // Execute block (simplified)
        info!("New block mined at height {}", block.header.chain_height);

        // Update state
        let reward = block.reward;
        consensus.state_manager.lock().add_emitted_supply(reward);
        if let Err(e) = consensus
            .state_manager
            .lock()
            .credit_account(block.producer, reward)
        {
            error!("Failed to credit miner: {}", e);
        }

        // Remove transactions from mempool
        for tx in &block.transactions[1..] {
            // skip coinbase
            let tx_hash = tx.hash();
            let tx_hash_arr: [u8; 32] = tx_hash.try_into().unwrap_or([0u8; 32]);
            consensus.mempool.remove_transaction(&tx_hash_arr);
        }
    }
}

impl crate::ConsensusInterface for Consensus {
    fn get_current_height(&self) -> u64 {
        self.blockdag.read().get_stats().tip_height
    }

    fn get_current_hash(&self) -> BlockHash {
        self.blockdag.read().get_tips().first().cloned().unwrap_or([0u8; 32])
    }
}

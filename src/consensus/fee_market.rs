use crate::core::{Address, Transaction};
use crate::dag::BlockDAG;
use crate::mempool::TxDagMempool;
use parking_lot::RwLock;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use tokio::time::Instant;

/// Fee market configuration
#[derive(Clone, Debug)]
pub struct FeeConfig {
    pub target_block_utilization: f64,
    pub base_fee_adjust_up_percent: u64,
    pub base_fee_adjust_down_percent: u64,
    pub base_fee_min: u64,
    pub base_fee_max: u64,
    pub ema_alpha: f64,
    pub congestion_threshold: f64,
    pub max_mempool_size: usize,
    pub min_fee_threshold: u64,
    pub tx_per_address_limit: usize,
    pub address_limit_window: u64, // seconds
}

impl Default for FeeConfig {
    fn default() -> Self {
        Self {
            target_block_utilization: 0.75,
            base_fee_adjust_up_percent: 115,
            base_fee_adjust_down_percent: 95,
            base_fee_min: 1,
            base_fee_max: 1_000_000,
            ema_alpha: 0.2,
            congestion_threshold: 0.9,
            max_mempool_size: 50000,
            min_fee_threshold: 1,
            tx_per_address_limit: 100,
            address_limit_window: 60,
        }
    }
}

/// Mempool entry with fee information
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MempoolEntry {
    pub transaction: Arc<Transaction>,
    pub effective_fee_per_byte: u64,
    pub priority_fee: u64,
    pub max_fee: u64,
    pub size_bytes: usize,
    pub inserted_at: Instant,
    pub sender: Address,
}

impl Ord for MempoolEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher fee first, then earlier insertion time
        other
            .effective_fee_per_byte
            .cmp(&self.effective_fee_per_byte)
            .then_with(|| self.inserted_at.cmp(&other.inserted_at))
    }
}

impl PartialOrd for MempoolEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl MempoolEntry {
    /// Create new mempool entry
    pub fn new(tx: Arc<Transaction>, base_fee: u64) -> Self {
        let from = tx.from;
        // Estimate size (rough approximation)
        let size_bytes = 200; // Rough estimate for transaction size
        let effective_fee_per_byte = if tx.gas_limit > 0 {
            tx.gas_price.saturating_sub(base_fee) / size_bytes as u64
        } else {
            0
        };
        let priority_fee = tx.gas_price.saturating_sub(base_fee);
        let max_fee = tx.gas_price;

        Self {
            transaction: tx,
            effective_fee_per_byte,
            priority_fee,
            max_fee,
            size_bytes,
            inserted_at: Instant::now(),
            sender: from,
        }
    }
}

/// Fee estimator for revenue estimation
#[derive(Clone, Debug)]
pub struct FeeEstimator {
    pub estimated_base_fee: u64,
    pub estimated_priority_fee: u64,
    pub network_congestion: f64,
}

/// Main fee market engine
pub struct FeeMarket {
    config: FeeConfig,
    dag: Arc<RwLock<BlockDAG>>,
    mempool: Arc<TxDagMempool>,
    base_fee: Arc<RwLock<u64>>,
    ema_base_fee: Arc<RwLock<f64>>,
    congestion_metrics: Arc<RwLock<CongestionMetrics>>,
    sorted_mempool: Arc<RwLock<BinaryHeap<MempoolEntry>>>,
    address_tx_count: Arc<RwLock<HashMap<Address, (usize, u64)>>>, // (count, window_start)
}

/// Congestion metrics for DAG-aware fee calculation
#[derive(Clone, Debug)]
pub struct CongestionMetrics {
    pub mempool_pressure: f64,
    pub block_fullness: f64,
    pub dag_density: f64,
    pub blue_score_growth: f64,
    pub network_congestion: f64,
    pub last_update: Instant,
}

impl FeeMarket {
    /// Create new fee market
    pub fn new(
        config: FeeConfig,
        dag: Arc<RwLock<BlockDAG>>,
        mempool: Arc<TxDagMempool>,
    ) -> Self {
        let base_fee_min = config.base_fee_min;
        let congestion_metrics = CongestionMetrics {
            mempool_pressure: 0.0,
            block_fullness: 0.0,
            dag_density: 0.0,
            blue_score_growth: 0.0,
            network_congestion: 0.0,
            last_update: Instant::now(),
        };

        Self {
            config,
            dag,
            mempool,
            base_fee: Arc::new(RwLock::new(base_fee_min)),
            ema_base_fee: Arc::new(RwLock::new(base_fee_min as f64)),
            congestion_metrics: Arc::new(RwLock::new(congestion_metrics)),
            sorted_mempool: Arc::new(RwLock::new(BinaryHeap::new())),
            address_tx_count: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Insert transaction into fee market
    pub async fn insert_transaction(&self, tx: Transaction) -> Result<(), String> {
        // Check anti-spam
        self.check_anti_spam(&tx).await?;

        // Get current base fee
        let base_fee = *self.base_fee.read();

        // Create mempool entry
        let tx_arc = Arc::new(tx.clone());
        let entry = MempoolEntry::new(tx_arc, base_fee);

        // Check minimum fee threshold
        if entry.effective_fee_per_byte < self.config.min_fee_threshold {
            let congestion = self.congestion_metrics.read().network_congestion;
            if congestion > self.config.congestion_threshold {
                return Err("Fee too low during network congestion".to_string());
            }
        }

        // Add to TxDagMempool
        let tx_clone = tx.clone();
        self.mempool.add_transaction(tx, vec![], None)?;

        // Add to sorted mempool
        let mut sorted = self.sorted_mempool.write();
        sorted.push(entry);

        // Update address tx count
        self.update_address_tx_count(tx_clone.from).await;

        // Evict if mempool full
        self.evict_low_fee_transactions().await;

        Ok(())
    }

    /// Remove transaction from fee market
    pub async fn remove_transaction(&self, tx_hash: &[u8; 32]) -> Result<(), String> {
        // Note: TxDagMempool might not have remove method, assume it does or add
        // For now, just remove from sorted mempool
        let mut sorted = self.sorted_mempool.write();
        sorted.retain(|entry| entry.transaction.hash() != *tx_hash);
        Ok(())
    }

    /// Select transactions for block with fee priority
    pub async fn select_transactions_for_block(
        &self,
        max_size: usize,
    ) -> Result<Vec<Arc<Transaction>>, String> {
        let sorted = self.sorted_mempool.read();
        let mut selected = Vec::new();
        let mut current_size = 0;

        for entry in sorted.iter() {
            if current_size + entry.size_bytes > max_size {
                break;
            }
            selected.push(entry.transaction.clone());
            current_size += entry.size_bytes;
        }

        Ok(selected)
    }

    /// Compute next base fee (DAG congestion aware)
    pub async fn compute_base_fee(&self) -> Result<u64, String> {
        // Update congestion metrics
        self.update_congestion_metrics().await;

        let metrics = self.congestion_metrics.read();
        let current_base = *self.base_fee.read();

        // Calculate congestion factor
        let congestion_factor =
            (metrics.mempool_pressure + metrics.block_fullness + metrics.dag_density) / 3.0;

        // Adaptive formula
        let adjustment_ratio = 1.0 + congestion_factor * 0.1; // 10% max adjustment

        let raw_new_fee = (current_base as f64 * adjustment_ratio) as u64;

        // Apply EMA smoothing
        let mut ema = self.ema_base_fee.write();
        *ema = self.config.ema_alpha * raw_new_fee as f64 + (1.0 - self.config.ema_alpha) * *ema;
        let smoothed_fee = *ema as u64;

        // Clamp to min/max
        let new_fee = smoothed_fee.clamp(self.config.base_fee_min, self.config.base_fee_max);

        *self.base_fee.write() = new_fee;

        Ok(new_fee)
    }

    /// Estimate priority fee for transaction
    pub async fn estimate_priority_fee(&self, tx_size: usize) -> Result<u64, String> {
        let metrics = self.congestion_metrics.read();
        let base_fee = *self.base_fee.read();

        // Estimate based on congestion
        let priority_multiplier = 1.0 + metrics.network_congestion * 2.0;
        let estimated_fee = (base_fee as f64 * priority_multiplier) as u64 * tx_size as u64;

        Ok(estimated_fee)
    }

    /// Evict low fee transactions when mempool full
    pub async fn evict_low_fee_transactions(&self) {
        let mut sorted = self.sorted_mempool.write();
        while sorted.len() > self.config.max_mempool_size {
            if let Some(entry) = sorted.pop() {
                // Remove from TxDagMempool if possible
                let _ = self.remove_transaction(&entry.transaction.hash()).await;
            }
        }

        // Also evict old transactions
        let cutoff = Instant::now() - tokio::time::Duration::from_secs(3600); // 1 hour
        sorted.retain(|entry| entry.inserted_at > cutoff);
    }

    /// Update congestion metrics
    pub async fn update_congestion_metrics(&self) {
        let dag = self.dag.read();
        let sorted = self.sorted_mempool.read();

        // Mempool pressure
        let mempool_pressure = sorted.len() as f64 / self.config.max_mempool_size as f64;

        // Block fullness (average of recent blocks)
        let tips = dag.get_tips();
        let mut total_fullness = 0.0;
        let mut count = 0;
        for tip in &tips {
            if let Some(block) = dag.get_block(tip) {
                let fullness = block.transactions.len() as f64 / 1000.0; // Assume 1000 tx target
                total_fullness += fullness.min(1.0);
                count += 1;
            }
        }
        let block_fullness = if count > 0 {
            total_fullness / count as f64
        } else {
            0.0
        };

        // DAG density
        let total_blocks = dag.block_count();
        let mut total_edges = 0;
        for tip in &tips {
            if let Some(block) = dag.get_block(tip) {
                total_edges += block.header.parent_hashes.len();
            }
        }
        let dag_density = if total_blocks > 0 {
            total_edges as f64 / total_blocks as f64
        } else {
            0.0
        };

        // Blue score growth (simplified)
        let blue_score_growth = 1.0; // Placeholder

        let network_congestion = (mempool_pressure + block_fullness + dag_density) / 3.0;

        let mut metrics = self.congestion_metrics.write();
        metrics.mempool_pressure = mempool_pressure;
        metrics.block_fullness = block_fullness;
        metrics.dag_density = dag_density;
        metrics.blue_score_growth = blue_score_growth;
        metrics.network_congestion = network_congestion;
        metrics.last_update = Instant::now();
    }

    /// Estimate block fee reward for miner
    pub async fn estimate_block_fee_reward(
        &self,
        transactions: &[Arc<Transaction>],
    ) -> Result<u64, String> {
        let base_fee = *self.base_fee.read();
        let mut total_fees = 0u64;

        for tx in transactions {
            let gas_cost = tx.gas_limit * tx.gas_price;
            let burned = base_fee * tx.gas_limit;
            let miner_fee = gas_cost.saturating_sub(burned);
            total_fees = total_fees.saturating_add(miner_fee);
        }

        // Apply DAG fairness (simplified)
        let fairness_factor = 1.0; // Could adjust based on merge sets
        Ok((total_fees as f64 * fairness_factor) as u64)
    }

    /// Check anti-spam rules
    async fn check_anti_spam(&self, tx: &Transaction) -> Result<(), String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut address_counts = self.address_tx_count.write();
        let (count, window_start) = address_counts.entry(tx.from).or_insert((0, now));

        // Reset window if expired
        if now - *window_start > self.config.address_limit_window {
            *count = 0;
            *window_start = now;
        }

        if *count >= self.config.tx_per_address_limit {
            return Err("Too many transactions from this address".to_string());
        }

        *count += 1;
        Ok(())
    }

    /// Update address transaction count
    async fn update_address_tx_count(&self, address: Address) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut address_counts = self.address_tx_count.write();
        let (count, window_start) = address_counts.entry(address).or_insert((0, now));

        if now - *window_start > self.config.address_limit_window {
            *count = 1;
            *window_start = now;
        } else {
            *count += 1;
        }
    }

    /// Get current base fee
    pub fn get_base_fee(&self) -> u64 {
        *self.base_fee.read()
    }

    /// Get fee estimator
    pub async fn get_fee_estimator(&self) -> Result<FeeEstimator, String> {
        let estimated_priority = self.estimate_priority_fee(100).await?; // Assume 100 bytes
        let metrics = self.congestion_metrics.read();

        Ok(FeeEstimator {
            estimated_base_fee: self.get_base_fee(),
            estimated_priority_fee: estimated_priority,
            network_congestion: metrics.network_congestion,
        })
    }

    /// Prune finalized transactions
    pub async fn prune_finalized(&self, finalized_txs: &[[u8; 32]]) {
        let mut sorted = self.sorted_mempool.write();
        for hash in finalized_txs {
            sorted.retain(|entry| entry.transaction.hash() != *hash);
        }
    }
}

/// Legacy function for compatibility
pub fn next_base_fee(dag: &BlockDAG) -> u64 {
    // Simple implementation for backward compatibility
    const BASE_FEE_MIN: u64 = 1;
    let order = dag.get_topological_order();
    if let Some(last_hash) = order.last() {
        if let Some(last_block) = dag.get_block(last_hash) {
            let prev_base = last_block.header.base_fee.max(BASE_FEE_MIN);
            let tx_count = last_block.transactions.len() as u64;
            let target = 1000;
            if tx_count * 2 > target {
                prev_base + prev_base * 10 / 100
            } else {
                prev_base.saturating_sub(prev_base * 10 / 100)
            }
            .max(BASE_FEE_MIN)
        } else {
            BASE_FEE_MIN
        }
    } else {
        BASE_FEE_MIN
    }
}

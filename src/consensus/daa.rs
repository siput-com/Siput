use super::blue_set::{BlueSetManager, BlueSetResult};
use super::ghostdag::GhostDagManager;
use crate::core::{Block, BlockHash};
use crate::dag::BlockDAG;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;

/// Difficulty adjustment algorithm (DAA) helpers.
///
/// The algorithm uses a moving-average window of the most recent N blocks
/// (currently 263).  It computes the actual elapsed time between the first
/// and last block in the window, clamping any individual timestamp gaps to
/// a reasonable range to resist manipulation.  The new difficulty is then
/// scaled linearly by the ratio of actual vs expected time, with a hard
/// cap on how quickly difficulty can change.
///
/// The difficulty value is expressed as "leading-zero bits"; we perform the
/// adjustment on the numeric value and cast back to `u32`.  This is
/// not cryptographically precise but is sufficient for a proof-of-concept.

const DAA_WINDOW: usize = 263;
const TARGET_BLOCK_TIME: u64 = 1; // 1 second per block
const MIN_ADJUST_RATIO: f64 = 0.25; // no more than 4x down
const MAX_ADJUST_RATIO: f64 = 4.0; // no more than 4x up
const MAX_GAP: u64 = TARGET_BLOCK_TIME * 4; // clamp individual gap to 4s

/// Configuration for DAA manager
#[derive(Clone, Debug)]
pub struct DaaConfig {
    pub window_size: usize,
    pub target_block_time: u64,
    pub max_adjustment_factor: f64,
    pub min_difficulty: u32,
    pub max_difficulty: u32,
    pub ema_alpha: f64,
    pub timestamp_future_limit: u64,
    pub timestamp_past_limit: u64,
    pub burst_threshold_blocks: usize,
    pub burst_adjustment_multiplier: f64,
}

impl Default for DaaConfig {
    fn default() -> Self {
        Self {
            window_size: 263, // ~1 hour at 13.5s blocks
            target_block_time: 30,
            max_adjustment_factor: 4.0,
            min_difficulty: 1,
            max_difficulty: u32::MAX / 4,
            ema_alpha: 0.1,
            timestamp_future_limit: 7200, // 2 hours
            timestamp_past_limit: 86400,  // 24 hours
            burst_threshold_blocks: 10,
            burst_adjustment_multiplier: 2.0,
        }
    }
}

/// Persistent difficulty state
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DifficultyState {
    pub current_difficulty: u32,
    pub last_update_timestamp: u64,
    pub ema_difficulty: f64,
    pub block_count_since_adjustment: usize,
    pub last_window_median_time: u64,
}

/// Window of blocks for DAA calculation
#[derive(Clone, Debug)]
pub struct DaaWindow {
    pub blocks: VecDeque<BlockHash>,
    pub timestamps: VecDeque<u64>,
    pub blue_scores: VecDeque<u64>,
}

/// Main DAA manager with concurrency support
pub struct DaaManager {
    config: DaaConfig,
    dag: Arc<RwLock<BlockDAG>>,
    _ghostdag: Arc<RwLock<GhostDagManager>>,
    blue_set: Arc<BlueSetManager>,
    state: Arc<RwLock<DifficultyState>>,
    window: Arc<RwLock<DaaWindow>>,
    persistent_store: Option<Arc<RwLock<rocksdb::DB>>>,
}

impl DaaManager {
    /// Create new DAA manager
    pub fn new(
        config: DaaConfig,
        dag: Arc<RwLock<BlockDAG>>,
        ghostdag: Arc<RwLock<GhostDagManager>>,
        blue_set: Arc<BlueSetManager>,
    ) -> Self {
        let initial_state = DifficultyState {
            current_difficulty: 1,
            last_update_timestamp: 0,
            ema_difficulty: 1.0,
            block_count_since_adjustment: 0,
            last_window_median_time: 0,
        };

        let window = DaaWindow {
            blocks: VecDeque::new(),
            timestamps: VecDeque::new(),
            blue_scores: VecDeque::new(),
        };

        DaaManager {
            config,
            dag,
            _ghostdag: ghostdag,
            blue_set,
            state: Arc::new(RwLock::new(initial_state)),
            window: Arc::new(RwLock::new(window)),
            persistent_store: None,
        }
    }

    /// Attach persistent storage
    pub fn with_persistent_store(mut self, db: Arc<RwLock<rocksdb::DB>>) -> Self {
        self.persistent_store = Some(db);
        self
    }

    /// Calculate next difficulty for a new block
    pub async fn calculate_next_difficulty(&self, new_block_timestamp: u64, blue_score: u64) -> Result<u32, String> {
        // Validate timestamp
        self.validate_timestamp(new_block_timestamp)?;

        // Update window with new block
        self.update_window(new_block_timestamp, blue_score)?;

        // Collect DAA window
        let window_data = self.collect_daa_window().await?;

        if window_data.len() < self.config.window_size {
            // Not enough data, return current difficulty
            let state = self.state.read();
            return Ok(state.current_difficulty);
        }

        // Compute median timestamp
        let median_time = self.median_timestamp(&window_data)?;

        // Compute block rate
        let block_rate = self.compute_block_rate(&window_data, median_time).await?;

        // Compute DAG-aware adjustment
        let dag_adjustment = self.compute_dag_adjustment().await?;

        // Compute target adjustment
        let target_adjustment = self.compute_target_adjustment(block_rate)?;

        // Apply burst mining protection
        let burst_adjustment = self.compute_burst_adjustment(&window_data, new_block_timestamp)?;

        // Combine adjustments
        let mut total_adjustment = target_adjustment * dag_adjustment * burst_adjustment;

        // Apply oscillation protection
        total_adjustment = total_adjustment.clamp(
            1.0 / self.config.max_adjustment_factor,
            self.config.max_adjustment_factor,
        );

        // Apply EMA smoothing
        let smoothed_adjustment = self.apply_ema_smoothing(total_adjustment)?;

        // Calculate new difficulty
        let state = self.state.read();
        let prev_diff = state.current_difficulty as f64;
        let mut new_diff = (prev_diff * smoothed_adjustment).round() as u32;

        // Apply floor and ceiling
        new_diff = new_diff.clamp(self.config.min_difficulty, self.config.max_difficulty);

        Ok(new_diff)
    }

    /// Validate timestamp against time warp attacks
    fn validate_timestamp(&self, timestamp: u64) -> Result<(), String> {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if timestamp > current_time + self.config.timestamp_future_limit {
            return Err("Timestamp too far in the future".to_string());
        }

        if timestamp < current_time.saturating_sub(self.config.timestamp_past_limit) {
            return Err("Timestamp too far in the past".to_string());
        }

        Ok(())
    }

    /// Update DAA window with new block
    pub fn update_window(&self, timestamp: u64, blue_score: u64) -> Result<(), String> {
        // First update timestamps
        {
            let mut window = self.window.write();
            window.timestamps.push_back(timestamp);
            if window.timestamps.len() > self.config.window_size {
                window.timestamps.pop_front();
            }
        }

        // Update blue scores
        {
            let mut window = self.window.write();
            window.blue_scores.push_back(blue_score);
            if window.blue_scores.len() > self.config.window_size {
                window.blue_scores.pop_front();
            }
        }

        Ok(())
    }

    /// Collect DAA window data
    async fn collect_daa_window(&self) -> Result<Vec<(u64, u64)>, String> {
        let window = self.window.read();
        let mut data = Vec::new();

        for i in 0..window.timestamps.len() {
            let timestamp = window.timestamps[i];
            let blue_score = window.blue_scores.get(i).copied().unwrap_or(0);
            data.push((timestamp, blue_score));
        }

        Ok(data)
    }

    /// Compute median timestamp from window
    fn median_timestamp(&self, window_data: &[(u64, u64)]) -> Result<u64, String> {
        if window_data.is_empty() {
            return Err("Empty window data".to_string());
        }

        let mut timestamps: Vec<u64> = window_data.iter().map(|(ts, _)| *ts).collect();
        timestamps.sort();

        let mid = timestamps.len() / 2;
        Ok(timestamps[mid])
    }

    /// Compute block rate from window
    async fn compute_block_rate(
        &self,
        window_data: &[(u64, u64)],
        _median_time: u64,
    ) -> Result<f64, String> {
        if window_data.len() < 2 {
            return Ok(1.0 / self.config.target_block_time as f64);
        }

        let first_time = window_data[0].0;
        let last_time = window_data[window_data.len() - 1].0;

        if last_time <= first_time {
            return Ok(1.0 / self.config.target_block_time as f64);
        }

        let actual_time_span = last_time - first_time;
        let _expected_time_span = (window_data.len() - 1) as u64 * self.config.target_block_time;

        // Adjust for blue score growth
        let blue_score_growth = window_data.last().unwrap().1 as f64 - window_data[0].1 as f64;
        let effective_blocks = blue_score_growth.max(1.0);

        Ok(effective_blocks / actual_time_span as f64)
    }

    /// Compute DAG-aware adjustment
    async fn compute_dag_adjustment(&self) -> Result<f64, String> {
        let tips = {
            let dag = self.dag.read();
            dag.get_tips()
        };

        // Parallel blocks factor
        let parallel_factor = tips.len() as f64;

        // Merge rate factor
        let mut merge_rate = 0.0;
        for i in 0..tips.len() {
            for j in (i + 1)..tips.len() {
                let merge_set = match self.blue_set.compute_merge_set(&tips[i], &tips[j]).await {
                    BlueSetResult::Ok(set) => set,
                    BlueSetResult::Err(e) => return Err(e),
                };
                merge_rate += merge_set.len() as f64;
            }
        }
        merge_rate /= tips.len().max(1) as f64;

        // DAG density
        let density = {
            let dag = self.dag.read();
            self.compute_dag_density(&dag)?
        };

        // Combine factors
        let adjustment = 1.0 + (parallel_factor - 1.0) * 0.1 + merge_rate * 0.05 + density * 0.2;
        Ok(adjustment.clamp(0.5, 2.0))
    }

    /// Compute DAG density
    fn compute_dag_density(&self, dag: &BlockDAG) -> Result<f64, String> {
        let total_blocks = dag.block_count();
        if total_blocks == 0 {
            return Ok(0.0);
        }

        let tips = dag.get_tips();
        let mut total_edges = 0;
        for tip in tips {
            if let Some(block) = dag.get_block(&tip) {
                total_edges += block.header.parent_hashes.len();
            }
        }

        Ok(total_edges as f64 / total_blocks as f64)
    }

    /// Compute target adjustment based on block rate
    fn compute_target_adjustment(&self, block_rate: f64) -> Result<f64, String> {
        let target_rate = 1.0 / self.config.target_block_time as f64;
        Ok(target_rate / block_rate)
    }

    /// Compute burst mining adjustment
    fn compute_burst_adjustment(
        &self,
        window_data: &[(u64, u64)],
        current_timestamp: u64,
    ) -> Result<f64, String> {
        let recent_window = window_data
            .iter()
            .rev()
            .take(self.config.burst_threshold_blocks)
            .collect::<Vec<_>>();

        if recent_window.len() < self.config.burst_threshold_blocks {
            return Ok(1.0);
        }

        let recent_time_span = current_timestamp - recent_window.last().unwrap().0;
        let expected_time =
            self.config.burst_threshold_blocks as u64 * self.config.target_block_time;

        if recent_time_span < expected_time / 2 {
            // Burst detected, increase difficulty
            Ok(self.config.burst_adjustment_multiplier)
        } else {
            Ok(1.0)
        }
    }

    /// Apply EMA smoothing
    fn apply_ema_smoothing(&self, adjustment: f64) -> Result<f64, String> {
        let mut state = self.state.write();
        let alpha = self.config.ema_alpha;
        state.ema_difficulty = alpha * adjustment + (1.0 - alpha) * state.ema_difficulty;
        Ok(state.ema_difficulty)
    }

    /// Update difficulty state after block acceptance
    pub async fn update_difficulty_state(
        &self,
        new_difficulty: u32,
        timestamp: u64,
    ) -> Result<(), String> {
        let mut state = self.state.write();
        state.current_difficulty = new_difficulty;
        state.last_update_timestamp = timestamp;
        state.block_count_since_adjustment += 1;

        // Persist state if available
        if let Some(store) = &self.persistent_store {
            let db = store.read();
            let key = b"difficulty_state";
            let data = bincode::serialize(&*state).map_err(|e| e.to_string())?;
            db.put(key, data).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// Load difficulty state from storage
    pub fn load_difficulty_state(&self) -> Result<(), String> {
        if let Some(store) = &self.persistent_store {
            let db = store.read();
            if let Ok(Some(data)) = db.get(b"difficulty_state") {
                let loaded_state: DifficultyState =
                    bincode::deserialize(&data).map_err(|e| e.to_string())?;
                *self.state.write() = loaded_state;
            }
        }
        Ok(())
    }

    /// Get current difficulty
    pub fn get_current_difficulty(&self) -> u32 {
        self.state.read().current_difficulty
    }

    pub fn get_current_daa_score(&self) -> u64 {
        let window = self.window.read();
        window.blue_scores.back().copied().unwrap_or(0)
    }

    pub fn calculate_block_reward(&self, blue_score: u64) -> u64 {
        let halving_interval = 210_000; // blocks
        let initial_reward = 500; // CTH
        let halvings = blue_score / halving_interval;
        if halvings >= 64 {
            0 // minimum reward
        } else {
            initial_reward >> halvings // divide by 2^halvings
        }
    }

    /// Prune old window data for memory management
    pub async fn prune_window(&self, _pruning_point: &BlockHash) {
        // In a full implementation, would remove blocks below pruning point
        // For now, just maintain window size
        let mut window = self.window.write();
        while window.timestamps.len() > self.config.window_size {
            window.timestamps.pop_front();
            window.blue_scores.pop_front();
        }
    }
}

impl DifficultyState {
    /// Create new difficulty state
    pub fn new() -> Self {
        Self {
            current_difficulty: 1,
            last_update_timestamp: 0,
            ema_difficulty: 1.0,
            block_count_since_adjustment: 0,
            last_window_median_time: 0,
        }
    }

    /// Add a block to the difficulty state (for testing)
    pub fn add_block(&mut self, timestamp: u64) {
        self.block_count_since_adjustment += 1;
        self.last_update_timestamp = timestamp;
    }

    /// Get current difficulty
    pub fn get_difficulty(&self) -> u32 {
        self.current_difficulty
    }
}

/// Calculate the next difficulty given a slice of chronologically ordered blocks.
///
/// If fewer than `DAA_WINDOW` blocks are supplied the previous difficulty is
/// returned unchanged.
pub fn calculate_next_difficulty(blocks: &[Block]) -> u32 {
    if blocks.len() < DAA_WINDOW {
        return blocks.last().map(|b| b.header.difficulty).unwrap_or(1);
    }

    // consider only the last window
    let recent = &blocks[blocks.len() - DAA_WINDOW..];

    // compute actual timespan with clamped deltas
    let mut actual: u64 = 0;
    let mut prev_ts = recent[0].header.timestamp;
    for blk in &recent[1..] {
        let ts = blk.header.timestamp;
        let delta = if ts > prev_ts { ts - prev_ts } else { 0 };
        actual = actual.saturating_add(delta.min(MAX_GAP));
        prev_ts = ts;
    }

    let expected = (DAA_WINDOW as u64) * TARGET_BLOCK_TIME;
    let mut ratio = (actual as f64) / (expected as f64);
    if ratio < MIN_ADJUST_RATIO {
        ratio = MIN_ADJUST_RATIO;
    } else if ratio > MAX_ADJUST_RATIO {
        ratio = MAX_ADJUST_RATIO;
    }

    let prev_diff = recent.last().unwrap().header.difficulty as f64;
    let mut next = (prev_diff * ratio).round();
    if next < 1.0 {
        next = 1.0;
    }
    if next > (u32::MAX as f64) {
        next = u32::MAX as f64;
    }
    next as u32
}

/// Convenience wrapper: compute next difficulty directly from a BlockDAG.
///
/// This pulls the most recent blocks from the DAG's topological order and
/// invokes `calculate_next_difficulty`.
pub fn next_difficulty(dag: &BlockDAG) -> u32 {
    let order = dag.get_topological_order();
    let mut recent_blocks: Vec<Block> = Vec::new();
    for hash in order.iter().rev().take(DAA_WINDOW) {
        if let Some(b) = dag.get_block(hash) {
            recent_blocks.push(b);
        }
    }
    recent_blocks.reverse();
    // Ensure we never return a difficulty of 0 (which would disable PoW).
    // This protects the bootstrap period where the genesis block has difficulty 0.
    calculate_next_difficulty(&recent_blocks).max(1)
}

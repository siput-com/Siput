use sha2::{Digest, Sha256};
use crate::core::BlockHash;

/// Reward configuration
#[derive(Debug, Clone)]
pub struct RewardConfig {
    /// Base block reward
    pub base_reward: u64,
    /// Reward halving interval
    pub halving_interval: u64,
    /// Activity window for reward calculation
    pub activity_window: usize,
    /// Fee collection percentage (0.0-1.0)
    pub fee_percentage: f64,
}

impl Default for RewardConfig {
    fn default() -> Self {
        Self {
            base_reward: 500,          // 500 CTH base reward
            halving_interval: 210_000, // Halve every 210k blocks
            activity_window: 1000,     // Look at last 1000 blocks for activity
            fee_percentage: 0.1,       // 10% of fees go to miner
        }
    }
}

/// Calculate expected block reward based on network conditions
pub fn calculate_expected_block_reward(
    emitted_supply: u64,
    chain_progress: f64,
    recent_timestamps: &[u64],
    tx_count: usize,
    config: &RewardConfig,
) -> u64 {
    // Base reward with halving
    let halvings = (emitted_supply / config.halving_interval) as f64;
    let base_reward = (config.base_reward as f64 * (0.5f64).powf(halvings)) as u64;

    // Activity factor based on recent block times
    let activity_factor = if recent_timestamps.len() >= 2 {
        let intervals: Vec<_> = recent_timestamps
            .windows(2)
            .map(|w| w[1].saturating_sub(w[0]))
            .collect();
        let avg_interval = intervals.iter().sum::<u64>() as f64 / intervals.len() as f64;
        // Target 30 seconds, so activity factor is inverse of interval
        (30.0 / avg_interval.max(1.0)).clamp(0.1, 10.0)
    } else {
        1.0
    };

    // Transaction factor
    let tx_factor = (tx_count as f64 / 100.0).clamp(0.1, 5.0);

    // Chain progress factor (early blocks get higher rewards)
    let progress_factor = if chain_progress < 0.1 {
        2.0 // Bootstrap period
    } else {
        1.0
    };

    ((base_reward as f64 * activity_factor * tx_factor * progress_factor) as u64).max(1)
}

/// Legacy reward calculation (kept for backwards compatibility)
/// New reward model is calculated via `calculate_expected_block_reward`.
#[deprecated(note = "Use calculate_expected_block_reward instead")]
pub fn calculate_reward(tx_count: usize) -> u64 {
    calculate_expected_block_reward(0, 0.0, &[], tx_count, &RewardConfig::default())
}

/// Calculate block reward based on blue score (halving schedule)
pub fn calculate_block_reward(blue_score: u64) -> u64 {
    let halving_interval = 210_000; // blocks
    let initial_reward = 500; // CTH
    let halvings = blue_score / halving_interval;
    if halvings >= 64 {
        0 // minimum reward
    } else {
        initial_reward >> halvings // divide by 2^halvings
    }
}

/// Calculate merkle root from a list of hashes
pub fn calculate_merkle_root(hashes: &[BlockHash]) -> [u8; 32] {
    if hashes.is_empty() {
        return [0u8; 32];
    }

    let mut current_hashes: Vec<[u8; 32]> = hashes.iter().map(|h| *h).collect();

    while current_hashes.len() > 1 {
        let mut next_level = Vec::new();
        for chunk in current_hashes.chunks(2) {
            let mut hasher = Sha256::new();
            hasher.update(&chunk[0]);
            if chunk.len() > 1 {
                hasher.update(&chunk[1]);
            } else {
                hasher.update(&chunk[0]); // Duplicate for odd number
            }
            next_level.push(hasher.finalize().into());
        }
        current_hashes = next_level;
    }

    current_hashes[0]
}

/// Calculate next difficulty based on DAA window
/// This is a simplified version - full implementation would be in DAA manager
pub fn calculate_next_difficulty(
    current_difficulty: u32,
    window_timestamps: &[u64],
    target_block_time: u64,
    min_difficulty: u32,
    max_difficulty: u32,
) -> u32 {
    if window_timestamps.len() < 2 {
        return current_difficulty;
    }

    // Calculate average block time in window
    let intervals: Vec<u64> = window_timestamps
        .windows(2)
        .map(|w| w[1].saturating_sub(w[0]))
        .collect();

    let avg_block_time = intervals.iter().sum::<u64>() as f64 / intervals.len() as f64;

    // Calculate adjustment factor
    let adjustment = target_block_time as f64 / avg_block_time;

    // Apply adjustment with damping
    let damped_adjustment = 1.0 + (adjustment - 1.0) * 0.25;

    // Calculate new difficulty
    let new_diff = (current_difficulty as f64 * damped_adjustment).round() as u32;

    // Clamp to min/max
    new_diff.clamp(min_difficulty, max_difficulty)
}

/// Calculate blue score for a block
/// Simplified version - full implementation in GhostDAG
pub fn calculate_blue_score(block_height: u64, parent_blue_scores: &[u64]) -> u64 {
    if parent_blue_scores.is_empty() {
        return 1; // Genesis block
    }

    // Blue score is max of parent blue scores + 1
    parent_blue_scores.iter().max().unwrap() + 1
}

/// Calculate hash rate from difficulty and block time
pub fn calculate_hash_rate(difficulty: u32, block_time_seconds: f64) -> f64 {
    // Simplified calculation: hash_rate = difficulty / block_time
    // In reality, this depends on the specific PoW algorithm
    difficulty as f64 / block_time_seconds
}
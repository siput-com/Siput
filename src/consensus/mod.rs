pub mod blue_set;
pub mod daa;
pub mod dag_traversal;
pub mod error;
pub mod engine;
pub mod fee_market;
pub mod ghostdag;
pub mod mining;

pub use engine::ConsensusEngine;
pub use blue_set::{
    BlueSet, BlueSetCache, BlueSetConfig, BlueSetManager, BlueSetResult, BlueSetStats,
};
pub use dag_traversal::DAGTraversal;
pub use error::ConsensusError;
pub use ghostdag::{
    BlueWork, GHOSTDAGEngine, GHOSTDAGStats, GhostDagConfig, GhostDagManager, GhostDagResult,
};

// expose reward + PoW helpers
pub use crate::utils::calculations::{calculate_expected_block_reward, RewardConfig};
pub use daa::{
    calculate_next_difficulty, next_difficulty, DaaConfig, DaaManager, DaaWindow, DifficultyState,
};
pub use fee_market::{next_base_fee, FeeConfig, FeeEstimator, FeeMarket, MempoolEntry};
pub use mining::{
    meets_difficulty, select_mining_parents,
    BlockTemplate, Consensus, MiningConfig, MiningController, MiningManager, MiningMetrics,
    MiningState, MiningTemplate, MiningWorker, CTS_GENESIS_ALLOCATION,
    CTS_MAX_SUPPLY, CTS_MINING_SUPPLY,
};

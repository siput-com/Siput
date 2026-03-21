use std::sync::Arc;
use parking_lot::RwLock;
use tokio::time::Duration;

use crate::consensus::mining::Consensus;
use crate::consensus::mining::MiningController;
use crate::consensus::{FeeMarket, FeeConfig, DaaManager, DaaConfig, BlueSetManager, BlueSetConfig, GhostDagManager, GhostDagConfig};
use crate::core::Block;
use crate::dag::blockdag::BlockDAG;
use crate::finality::FinalityEngine;

/// Service untuk menangani consensus operations
pub struct ConsensusService {
    /// GHOSTDAG consensus engine
    ghostdag: Arc<RwLock<GhostDagManager>>,
    /// Finality engine
    finality_engine: Arc<FinalityEngine>,
    /// Mining controller
    mining_controller: Option<Arc<MiningController>>,
    /// Consensus wrapper
    consensus: Arc<Consensus>,
}

impl ConsensusService {
    /// Buat consensus service baru
    pub fn new(
        ghostdag: Arc<RwLock<GhostDagManager>>,
        finality_engine: Arc<FinalityEngine>,
        mining_controller: Option<Arc<MiningController>>,
        consensus: Arc<Consensus>,
    ) -> Self {
        Self {
            ghostdag,
            finality_engine,
            mining_controller,
            consensus,
        }
    }

    /// Attach DAG to consensus engines
    pub fn attach_dag(&self, dag: BlockDAG) {
        self.ghostdag.write().attach_dag(dag.clone());
        // Update consensus components if needed
    }

    /// Annotate block with consensus metadata
    pub fn annotate_block(&self, dag: &BlockDAG, block: &mut Block) -> Result<(), String> {
        self.ghostdag.write().annotate_block(dag, block)
    }

    /// Generate ordering
    pub fn generate_ordering(&self) -> Result<Vec<crate::core::BlockHash>, String> {
        self.ghostdag.write().generate_ordering()
    }

    /// Check finality
    pub fn check_finality(&self, dag: &BlockDAG) -> Result<(), String> {
        // Update ghostdag with latest DAG state
        self.ghostdag.write().attach_dag(dag.clone());

        // Generate ordering and run finality calculation
        if let Ok(_ordering) = self.ghostdag.write().generate_ordering() {
            self.finality_engine.compute_finality(dag)?;
        }

        Ok(())
    }

    /// Get finality height
    pub fn get_finality_height(&self) -> Option<u64> {
        self.finality_engine.get_finalization_height()
    }

    /// Start mining
    pub fn start_mining(&self, threads: usize) {
        if let Some(controller) = &self.mining_controller {
            controller.start(threads);
        }
    }

    /// Stop mining
    pub fn stop_mining(&self) {
        if let Some(controller) = &self.mining_controller {
            controller.stop();
        }
    }

    /// Get hash rate
    pub fn get_hash_rate(&self) -> f64 {
        self.mining_controller
            .as_ref()
            .map(|c| c.get_hash_rate())
            .unwrap_or(0.0)
    }

    /// Get consensus reference
    pub fn consensus(&self) -> &Arc<Consensus> {
        &self.consensus
    }
}
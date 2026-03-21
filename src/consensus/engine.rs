use crate::core::{BlockHash, ConsensusBlock};
use crate::dag::blockdag::BlockDAG;

/// Abstraction for consensus engine used by node and mining components.
pub trait ConsensusEngine: Send + Sync {
    fn attach_dag(&mut self, dag: BlockDAG);
    fn get_virtual_selected_parent(&self) -> BlockHash;
    fn get_virtual_blue_score(&self) -> u64;
    fn calculate_blue_score(&self, hash: &BlockHash) -> Result<u64, String>;
    fn annotate_block(&mut self, dag: &BlockDAG, block: &mut dyn ConsensusBlock) -> Result<(), String>;
}

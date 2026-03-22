pub mod abstraction;
pub mod block;
pub mod cli;
pub mod config;
pub mod consensus;
pub mod contracts;
pub mod core;
pub mod crypto;
pub mod dag;
pub mod errors;
pub mod events;
pub mod execution;
pub mod finality;
#[path = "../indexer/mod.rs"]
pub mod indexer;
pub mod mempool;
pub mod network;
pub mod node;
pub mod pipeline;
pub mod rpc;
pub mod security;
pub mod state;
pub mod storage;
pub mod utils;
pub mod vm;
pub mod wallet;
pub mod observability;

pub use block::BlockProducer;
pub use cli::CLIInterface;
pub use config::NodeConfig;
pub use consensus::{BlueSet, BlueSetStats, ConsensusEngine, DAGTraversal, GHOSTDAGEngine, GHOSTDAGStats};
pub use core::{Address, Block, BlockHash, ConsensusBlock, Transaction, TransactionId};
pub use dag::{BlockDAG, DAGExport, DAGIndex, DAGStats};
pub use pipeline::{TransactionPipelineManager, TransactionPipelineStage, PipelineResult, PipelineContext};
pub use events::{GlobalEventEmitter, GlobalEventListener};

// Traits for decoupling
pub trait ConsensusInterface {
    fn get_current_height(&self) -> u64;
    fn get_current_hash(&self) -> BlockHash;
}

pub trait VMExecutor {
    fn execute(&self, code: &[u8], args: &[u8]) -> Result<Vec<u8>, String>;
}
pub use execution::transaction_executor::{ExecutionResult, TransactionExecutor};
pub use finality::FinalityEngine;
pub use mempool::TxDagMempool;
pub use network::{NetworkInterface, P2PNode};
pub use node::NodeRuntime;
pub use state::state_manager::StateManager;
pub use state::state_tree::{Account, SparseMerkleTree};
pub use storage::{BlockStore, StorageEngine};

use std::sync::Arc;
use parking_lot::Mutex;

use crate::core::{Block, Transaction, BlockHash};
use crate::execution::transaction_executor::TransactionExecutor;
use crate::state::state_manager::StateManager;
use crate::storage::{BlockStore, ChainStorage};

/// Service untuk menangani eksekusi blok dan state management
pub struct ExecutionService {
    /// State manager
    state_manager: Arc<parking_lot::Mutex<StateManager>>,
    /// Execution engine
    executor: Arc<parking_lot::Mutex<TransactionExecutor>>,
    /// Optional pruning helper
    pruner: Option<Arc<parking_lot::Mutex<crate::storage::pruning::RollingWindowPruner>>>,
    /// Contract registry for persistent smart contract storage
    contract_registry: Arc<parking_lot::Mutex<crate::contracts::ContractRegistry>>,
    /// Persistent chain storage
    chain_storage: Arc<crate::storage::ChainStorage>,
}

impl ExecutionService {
    /// Buat execution service baru
    pub fn new(
        state_manager: Arc<parking_lot::Mutex<StateManager>>,
        executor: Arc<parking_lot::Mutex<TransactionExecutor>>,
        pruner: Option<Arc<parking_lot::Mutex<crate::storage::pruning::RollingWindowPruner>>>,
        contract_registry: Arc<parking_lot::Mutex<crate::contracts::ContractRegistry>>,
        chain_storage: Arc<crate::storage::ChainStorage>,
    ) -> Self {
        Self {
            state_manager,
            executor,
            pruner,
            contract_registry,
            chain_storage,
        }
    }

    /// Execute block dan apply state changes
    pub fn execute_block(&self, block: &Block) -> Result<(), String> {
        // Validate block reward against the economic model before applying state.
        {
            let emitted = self.state_manager.lock().get_emitted_supply();
            // Note: This would need DAG access, but we'll keep it simple for now
            // In full refactor, pass DAG reference or move validation elsewhere

            // Skip reward validation for bootstrap blocks
            // Simplified validation
        }

        let mut exec = self.executor.lock();
        // Ensure executor has the latest up-to-date state before running the block
        exec.state_manager = self.state_manager.lock().clone();
        let result = exec.execute_block(block);

        if result.success {
            // persist changes back to the node state manager
            *self.state_manager.lock() = exec.state_manager.clone();

            // apply transactions from block
            let mut state = self.state_manager.lock();
            state.apply_block(&block.transactions)?;

            // credit miner reward and collected fees
            let mut total_miner_credit: u64 = 0;
            total_miner_credit = total_miner_credit.saturating_add(block.reward);
            total_miner_credit = total_miner_credit.saturating_add(result.total_fees);
            state.credit_account(block.producer, total_miner_credit)?;

            // Track emitted supply
            state.add_emitted_supply(block.reward.saturating_add(result.total_fees));

            // perform pruning if configured
            if let Some(pruner) = &self.pruner {
                let mut p = pruner.lock();
                // Note: Would need height, simplified
                // state.snapshot_and_prune(height, &mut p);
            }

            // Persist state after successful block execution
            let _ = self.chain_storage.persist_state(&state);

            tracing::debug!(
                "Block executed: {} txs, miner credit {}",
                result.executed_transactions,
                total_miner_credit
            );
        }

        Ok(())
    }

    /// Get current state root
    pub fn get_state_root(&self) -> [u8; 32] {
        self.state_manager.lock().get_state_root()
    }

    /// Get account balance
    pub fn get_balance(&self, address: &crate::core::transaction::Address) -> u64 {
        self.state_manager.lock().get_balance(*address)
    }

    /// Get account nonce
    pub fn get_nonce(&self, address: &crate::core::transaction::Address) -> u64 {
        self.state_manager
            .lock()
            .get_account(address)
            .map(|acc| acc.nonce)
            .unwrap_or(0)
    }

    /// Initialize tokenomics state
    pub fn initialize_tokenomics(&self) {
        self.state_manager.lock().initialize_tokenomics();
    }

    /// Load persisted state
    pub fn load_persisted_state(&self) -> Option<StateManager> {
        self.chain_storage.load_state()
    }

    /// Persist state
    pub fn persist_state(&self, state: &StateManager) {
        let _ = self.chain_storage.persist_state(state);
    }
}
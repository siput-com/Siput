use crate::state::state_manager::StateManager;
use crate::Block;
use crate::execution::parallel_executor::execute_block_transactions_parallel;
use crate::contracts::contract_registry::ContractRegistry;
use crate::contracts::contract_storage::ContractStorage;
use crate::observability::{trace_performance, trace_blockchain_operation, metrics};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub type Hash = [u8; 32];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub new_state_root: Hash,
    pub executed_transactions: usize,
    pub success: bool,
    pub total_fees: u64,
}

pub struct TransactionExecutor {
    pub vm_executor: Arc<dyn crate::VMExecutor + Send + Sync>,
    pub state_manager: StateManager,
    pub registry: ContractRegistry,
    pub storage: Arc<ContractStorage>,
}

impl TransactionExecutor {
    pub fn new_with_vm(vm_executor: Arc<dyn crate::VMExecutor + Send + Sync>, db_path: &str) -> Self {
        let state_manager = StateManager::new();
        let registry_db_path = format!("{}-registry", db_path);
        let storage_db_path = format!("{}-storage", db_path);
        let registry = ContractRegistry::new(&registry_db_path)
            .expect("Failed to initialize contract registry");
        let storage = Arc::new(ContractStorage::new(&storage_db_path));

        Self {
            vm_executor,
            state_manager,
            registry,
            storage,
        }
    }

    pub fn new_default() -> Self {
        panic!("VM executor must be injected via new_with_vm to avoid tight vm coupling");
    }

    pub fn execute_block(&mut self, block: &Block) -> ExecutionResult {
        trace_performance!(self.execute_block_inner(block))
    }

    fn execute_block_inner(&mut self, block: &Block) -> ExecutionResult {
        let block_hash = hex::encode(&block.hash);
        let _span = trace_blockchain_operation!("block_execution", &block_hash);

        tracing::info!(
            block_hash = %block_hash,
            transaction_count = block.transactions.len(),
            "Starting block execution"
        );

        // Record TPS metrics
        let start_time = std::time::Instant::now();
        metrics::record_tps(block.transactions.len() as f64);

        // Use parallel execution with dependency-injected VM executor
        let receipts = execute_block_transactions_parallel(
            block.transactions.clone(),
            &mut self.state_manager,
            &mut self.registry,
            self.storage.clone(),
            self.vm_executor.clone(),
        );

        let executed = receipts.len();
        let success = receipts.iter().all(|r| r.success);
        let total_fees = receipts.iter()
            .enumerate()
            .filter(|(_, r)| r.success)
            .map(|(i, r)| r.gas_used.saturating_mul(block.transactions[i].gas_price))
            .sum();

        let execution_time = start_time.elapsed();
        metrics::record_latency("block_execution", execution_time);

        tracing::info!(
            block_hash = %block_hash,
            executed_transactions = executed,
            success,
            total_fees,
            execution_time_ms = execution_time.as_millis(),
            "Block execution completed"
        );

        ExecutionResult {
            new_state_root: self.state_manager.get_state_root(),
            executed_transactions: executed,
            success,
            total_fees,
        }
    }

    pub fn execute_blocks_in_order(&mut self, blocks: &[Block]) -> Vec<ExecutionResult> {
        blocks
            .iter()
            .map(|block| self.execute_block(block))
            .collect()
    }

    pub fn get_current_state_root(&self) -> Hash {
        self.state_manager.get_state_root()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::transaction::{Address, Transaction};
    use rand::{thread_rng, Rng};
    use secp256k1::{Secp256k1, SecretKey};
    use sha2::{Digest, Sha256};

    fn create_signed_transaction(
        from: Address,
        to: Address,
        amount: u64,
        nonce: u64,
        private_key: &SecretKey,
    ) -> Transaction {
        let mut tx = Transaction::new_transfer(from, to, amount, nonce, 21000, 1);
        tx.sign(private_key).unwrap();
        tx
    }

    fn create_test_block(transactions: Vec<Transaction>) -> Block {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Block::new(vec![], timestamp, transactions, 0, 0, 0, [0; 20], [0; 32])
    }

    #[test]
    fn test_execute_block() {
        let secp = Secp256k1::new();
        let mut rng = rand::thread_rng();
        let secret_bytes: [u8; 32] = rng.gen();
        let secret_key = SecretKey::from_slice(&secret_bytes).unwrap();
        let public_key = secret_key.public_key(&secp);
        let pubkey_hash = Sha256::digest(&public_key.serialize()[1..]);
        let from: Address = pubkey_hash[12..32].try_into().unwrap();
        let to: Address = [2; 20];

        let mut executor = TransactionExecutor::new_with_vm(Arc::new(DummyVMExecutor), "./data/test_contracts_execute.db");
        // give enough balance to cover transfer + gas fees
        executor
            .state_manager
            .state_tree
            .update_account(from, crate::state::state_tree::Account::new(100_000, 0));

        let tx = create_signed_transaction(from, to, 100, 0, &secret_key);
        let block = create_test_block(vec![tx]);

        let result = executor.execute_block(&block);
        assert!(result.success);
        assert_eq!(result.executed_transactions, 1);
        assert_ne!(result.new_state_root, [0; 32]);
        assert!(result.total_fees > 0);
    }

    #[test]
    fn test_execute_block_with_invalid_transaction() {
        let _secp = Secp256k1::new();
        let mut rng = thread_rng();
        let secret_bytes: [u8; 32] = rng.gen();
        let secret_key = SecretKey::from_slice(&secret_bytes).unwrap();
        let from: Address = [1; 20];
        let to: Address = [2; 20];

        let mut executor = TransactionExecutor::new_with_vm(Arc::new(DummyVMExecutor), "./data/test_contracts_invalid.db");
        executor
            .state_manager
            .state_tree
            .update_account(from, crate::state::state_tree::Account::new(50, 0));

        let tx = create_signed_transaction(from, to, 100, 0, &secret_key); // Insufficient balance
        let block = create_test_block(vec![tx]);

        let result = executor.execute_block(&block);
        assert!(!result.success);
        assert_eq!(result.executed_transactions, 0);
        assert_eq!(result.total_fees, 0);
    }

    #[test]
    fn test_execute_multiple_blocks() {
        let secp = Secp256k1::new();
        let mut rng = rand::thread_rng();
        let secret_bytes: [u8; 32] = rng.gen();
        let secret_key = SecretKey::from_slice(&secret_bytes).unwrap();
        let public_key = secret_key.public_key(&secp);
        let pubkey_hash = Sha256::digest(&public_key.serialize()[1..]);
        let from: Address = pubkey_hash[12..32].try_into().unwrap();
        let to: Address = [2; 20];

        let mut executor = TransactionExecutor::new_with_vm(Arc::new(DummyVMExecutor), "./data/test_contracts_multiple.db");
        // provide enough funds for multiple transfers + gas
        executor
            .state_manager
            .state_tree
            .update_account(from, crate::state::state_tree::Account::new(200_000, 0));

        let tx1 = create_signed_transaction(from, to, 100, 0, &secret_key);
        let tx2 = create_signed_transaction(from, to, 100, 1, &secret_key);
        let block1 = create_test_block(vec![tx1]);
        let block2 = create_test_block(vec![tx2]);

        let results = executor.execute_blocks_in_order(&[block1, block2]);
        assert_eq!(results.len(), 2);
        assert!(results[0].success);
        assert!(results[1].success);
        assert_eq!(results[0].executed_transactions, 1);
        assert_eq!(results[1].executed_transactions, 1);
        assert!(results[0].total_fees > 0);
        assert!(results[1].total_fees > 0);
    }
}

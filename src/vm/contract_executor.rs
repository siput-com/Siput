use crate::contracts::contract_registry::ContractRegistry;
use crate::contracts::contract_storage::ContractStorage;
use crate::core::transaction::Transaction;
use crate::state::state_manager::StateManager;
use crate::vm::{VmEngineManager, VmType, BlockContext, WasmVmEngine, CustomVmEngine, InMemoryCustomStorage};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Simple storage implementation for WASM VM
struct SimpleWasmStorage {
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl SimpleWasmStorage {
    fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl crate::vm::wasm_runtime::Storage for SimpleWasmStorage {
    fn read(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.data.read().unwrap().get(key).cloned()
    }

    fn write(&self, key: &[u8], value: &[u8]) {
        self.data.write().unwrap().insert(key.to_vec(), value.to_vec());
    }

    fn box_clone(&self) -> Box<dyn crate::vm::wasm_runtime::Storage> {
        Box::new(SimpleWasmStorage {
            data: Arc::clone(&self.data),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Executor responsible for handling contract transactions and integrating
/// with the existing state manager.
pub struct ContractExecutor {
    pub state_manager: StateManager,
    pub registry: ContractRegistry,
    pub storage: ContractStorage,
    pub vm_manager: Arc<VmEngineManager>,
}

impl ContractExecutor {
    /// Create a new executor with a path to a persistent contract registry and default VM setup.
    pub fn new(
        state_manager: StateManager,
        contract_registry_path: &str,
        contract_storage_path: &str,
    ) -> Result<Self, String> {
        let mut vm_manager = VmEngineManager::new();

        // Register WASM VM by default
        let wasm_storage = Arc::new(SimpleWasmStorage::new()); // Placeholder storage
        let wasm_vm = WasmVmEngine::new(wasm_storage)
            .map_err(|e| format!("Failed to create WASM VM: {:?}", e))?;
        vm_manager.register_engine(Box::new(wasm_vm))
            .map_err(|e| format!("Failed to register WASM VM: {}", e))?;

        // Register Custom VM
        let custom_storage = Arc::new(InMemoryCustomStorage::new());
        let custom_vm = CustomVmEngine::new(custom_storage);
        vm_manager.register_engine(Box::new(custom_vm))
            .map_err(|e| format!("Failed to register Custom VM: {}", e))?;

        Self::new_with_vm_manager(
            state_manager,
            contract_registry_path,
            contract_storage_path,
            Arc::new(vm_manager),
        )
    }

    /// Create a new executor with a custom VM manager.
    pub fn new_with_vm_manager(
        state_manager: StateManager,
        contract_registry_path: &str,
        contract_storage_path: &str,
        vm_manager: Arc<VmEngineManager>,
    ) -> Result<Self, String> {
        Ok(Self {
            registry: ContractRegistry::new(contract_registry_path)?,
            storage: ContractStorage::new(contract_storage_path),
            vm_manager,
            state_manager,
        })
    }

    /// Set the default VM type for this executor
    pub fn set_default_vm(&mut self, vm_type: VmType) -> Result<(), String> {
        Arc::get_mut(&mut self.vm_manager)
            .ok_or("VM manager is shared")?
            .set_default_vm(vm_type)
    }

    /// Get available VM types
    pub fn available_vms(&self) -> Vec<VmType> {
        self.vm_manager.list_vm_types()
    }

    /// Handle a transaction of any supported type.
    /// Execute a transaction and return the amount of gas actually used.  The
    /// caller (e.g. `TransactionExecutor`) can then calculate the fee and
    /// credit it appropriately.
    pub fn execute_transaction(&mut self, tx: &Transaction) -> Result<u64, String> {
        match &tx.payload {
            crate::core::transaction::TxPayload::Transfer { .. } => {
                // For simple transfers we assume the sender uses the entire gas
                // limit.  The fee deduction is handled inside `apply_transaction`.
                self.state_manager.apply_transaction(tx)?;
                Ok(tx.gas_limit)
            }
            crate::core::transaction::TxPayload::Coinbase { .. } => {
                Err("Coinbase transactions cannot be executed in contract executor".to_string())
            }
            crate::core::transaction::TxPayload::ContractDeploy {
                wasm_code,
                init_args,
            } => self.deploy_contract(tx, wasm_code.clone(), init_args.clone()),
            crate::core::transaction::TxPayload::ContractCall {
                contract_address,
                method,
                args,
            } => self.call_contract(tx, *contract_address, method.clone(), args.clone()),
        }
    }

    fn deploy_contract(
        &mut self,
        tx: &Transaction,
        bytecode: Vec<u8>,
        _init_args: Vec<u8>,
    ) -> Result<u64, String> {
        // Auto-detect VM type from bytecode
        let vm_engine = self.vm_manager.get_engine_for_bytecode(&bytecode);

        // Create block context
        let block_context = BlockContext {
            block_number: 0, // Would get from current block
            block_hash: [0; 32], // Would get from current block
            timestamp: 0, // Would get current timestamp
            gas_price: tx.gas_price,
        };

        // Deploy contract
        let contract_instance = vm_engine.deploy_contract(
            &bytecode,
            &tx.from,
            tx.gas_limit,
            &block_context,
        ).map_err(|e| format!("Contract deployment failed: {:?}", e))?;

        // Register contract in registry
        self.registry.register_contract(contract_instance.address, bytecode)?;

        // Initialize contract storage if needed
        self.storage.initialize_contract(contract_instance.address)?;

        // Calculate fee (simplified)
        let gas_used = tx.gas_limit / 2; // Would calculate actual gas used
        let fee = gas_used.saturating_mul(tx.gas_price);
        self.state_manager.deduct_fee(tx.from, fee)?;

        Ok(gas_used)
    }

    fn call_contract(
        &mut self,
        tx: &Transaction,
        contract_address: [u8; 20],
        method: String,
        args: Vec<u8>,
    ) -> Result<u64, String> {
        // Get contract bytecode to determine VM type
        let bytecode = self.registry.get_contract_code(contract_address)?
            .ok_or_else(|| format!("Contract not found: {:?}", contract_address))?;

        let vm_engine = self.vm_manager.get_engine_for_bytecode(&bytecode);

        // Create block context
        let block_context = BlockContext {
            block_number: 0,
            block_hash: [0; 32],
            timestamp: 0,
            gas_price: tx.gas_price,
        };

        // Execute contract call
        let result = vm_engine.execute_contract(
            &contract_address,
            &method,
            &args,
            &tx.from,
            tx.gas_limit,
            &block_context,
        ).map_err(|e| format!("Contract execution failed: {:?}", e))?;

        if !result.success {
            return Err("Contract execution failed".to_string());
        }

        // Calculate fee
        let fee = result.gas_used.saturating_mul(tx.gas_price);
        self.state_manager.deduct_fee(tx.from, fee)?;

        Ok(result.gas_used)
    }

    /// Execute contract method with given args (legacy method for compatibility)
    pub fn execute_contract(&self, contract_address: &[u8; 20], method: &str, args: &[u8]) -> Result<Vec<u8>, String> {
        // Get contract bytecode
        let bytecode = self.registry.get_contract_code(*contract_address)?
            .ok_or_else(|| format!("Contract not found: {:?}", contract_address))?;

        let vm_engine = self.vm_manager.get_engine_for_bytecode(&bytecode);

        // Create block context
        let block_context = BlockContext {
            block_number: 0,
            block_hash: [0; 32],
            timestamp: 0,
            gas_price: 0,
        };

        // Execute with dummy caller and gas
        let dummy_caller = [0u8; 20];
        let result = vm_engine.execute_contract(
            contract_address,
            method,
            args,
            &dummy_caller,
            1_000_000, // High gas limit for direct calls
            &block_context,
        ).map_err(|e| format!("Contract execution failed: {:?}", e))?;

        if !result.success {
            return Err("Contract execution failed".to_string());
        }

        Ok(result.return_data)
    }
}

impl crate::VMExecutor for ContractExecutor {
    fn execute(&self, code: &[u8], args: &[u8]) -> Result<Vec<u8>, String> {
        // For decoupling, assume code is contract address
        let mut address = [0u8; 20];
        address.copy_from_slice(&code[..20]);
        self.execute_contract(&address, "main", args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::transaction::{Address, Transaction};
    use rand::{thread_rng, Rng};
    use secp256k1::{Secp256k1, SecretKey};
    use sha2::Digest;

    fn create_signed_tx(
        from: Address,
        to: Address,
        amount: u64,
        nonce: u64,
        gas_limit: u64,
        gas_price: u64,
        privk: &SecretKey,
    ) -> Transaction {
        let mut tx = Transaction::new_transfer(from, to, amount, nonce, gas_limit, gas_price);
        tx.sign(privk).unwrap();
        tx
    }

    #[test]
    fn test_transfer_charges_full_gas() {
        let mut rng = thread_rng();
        let secp = Secp256k1::new();
        let mut priv_bytes = [0u8; 32];
        rng.fill(&mut priv_bytes);
        let privk = SecretKey::from_slice(&priv_bytes).unwrap();
        let pubk = privk.public_key(&secp);
        let pubhash = sha2::Sha256::digest(&pubk.serialize()[1..]);
        let from: Address = pubhash[12..32].try_into().unwrap();
        let to: Address = [2; 20];

        // Use separate DB paths for registry and storage to avoid conflicts during tests
        let registry_path = "./data/test_contracts_executor_registry.db";
        let storage_path = "./data/test_contracts_executor_storage.db";
        let mut executor = ContractExecutor::new(StateManager::new(), registry_path, storage_path)
            .expect("Should create executor");
        // Ensure sender has enough balance to cover transfer + gas
        executor
            .state_manager
            .state_tree
            .update_account(from, crate::state::state_tree::Account::new(100000, 0));

        let tx = create_signed_tx(from, to, 100, 0, 21000, 1, &privk);
        let used = executor
            .execute_transaction(&tx)
            .expect("tx should succeed");
        assert_eq!(used, 21000);
        // fee = 21000*1
        let sender_acc = executor.state_manager.get_account(&from).unwrap();
        assert_eq!(sender_acc.balance, 100000 - 100 - 21000);
        let recv_acc = executor.state_manager.get_account(&to).unwrap();
        assert_eq!(recv_acc.balance, 100);
    }

    #[test]
    fn test_contract_deploy_and_call_works() {
        let _ = std::fs::remove_dir_all("./data/test_contracts_executor_registry2.db");
        let _ = std::fs::remove_dir_all("./data/test_contracts_executor_storage2.db");
        let mut executor = ContractExecutor::new(
            StateManager::new(),
            "./data/test_contracts_executor_registry2.db",
            "./data/test_contracts_executor_storage2.db",
        )
        .expect("Should create executor");

        let from: Address = [1; 20];
        executor
            .state_manager
            .state_tree
            .update_account(from, crate::state::state_tree::Account::new(1_000_000, 0));

        let wat = "(module (func (export \"foo\") (nop)))";
        let wasm = wat::parse_str(wat).unwrap();
        let deploy_tx = crate::core::transaction::Transaction::new_deploy(from, wasm.clone(), vec![], 0, 100000, 1);
        let deployed_gas = executor.execute_transaction(&deploy_tx).expect("deploy should succeed");

        assert!(deployed_gas > 0);

        let contract_address: [u8; 20] = deploy_tx.hash()[0..20].try_into().unwrap();
        let call_tx = crate::core::transaction::Transaction::new_call(from, contract_address, "foo".to_string(), vec![], 1, 100000, 1);
        let call_gas = executor.execute_transaction(&call_tx).expect("call should succeed");
        assert!(call_gas > 0);
    }
}


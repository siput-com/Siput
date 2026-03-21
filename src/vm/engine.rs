use crate::core::{Address, BlockHash, Transaction};
use std::collections::HashMap;

/// VM Engine abstraction for smart contract execution
/// Supports multiple VM implementations (WASM, Custom, etc.)
pub trait VmEngine: Send + Sync {
    /// Get VM type identifier
    fn vm_type(&self) -> VmType;

    /// Deploy/instantiate a contract
    fn deploy_contract(
        &self,
        bytecode: &[u8],
        deployer: &Address,
        gas_limit: u64,
        block_context: &BlockContext,
    ) -> Result<ContractInstance, VmError>;

    /// Execute a contract method
    fn execute_contract(
        &self,
        contract_address: &Address,
        method: &str,
        args: &[u8],
        caller: &Address,
        gas_limit: u64,
        block_context: &BlockContext,
    ) -> Result<ExecutionResult, VmError>;

    /// Get contract code
    fn get_contract_code(&self, contract_address: &Address) -> Result<Option<Vec<u8>>, VmError>;

    /// Check if contract exists
    fn contract_exists(&self, contract_address: &Address) -> bool;

    /// Validate contract bytecode before deployment
    fn validate_bytecode(&self, bytecode: &[u8]) -> Result<(), VmError>;

    /// Get supported contract types
    fn supported_contract_types(&self) -> Vec<String>;
}

/// VM Type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmType {
    Wasm,
    Custom,
    EVM, // Future support
}

/// Block context for execution
#[derive(Debug, Clone)]
pub struct BlockContext {
    pub block_number: u64,
    pub block_hash: BlockHash,
    pub timestamp: u64,
    pub gas_price: u64,
}

/// Contract instance handle
#[derive(Debug, Clone)]
pub struct ContractInstance {
    pub address: Address,
    pub vm_type: VmType,
    pub code_hash: [u8; 32],
}

/// Execution result
#[derive(Debug)]
pub struct ExecutionResult {
    pub return_data: Vec<u8>,
    pub gas_used: u64,
    pub logs: Vec<ContractLog>,
    pub success: bool,
}

/// Contract execution log
#[derive(Debug, Clone)]
pub struct ContractLog {
    pub address: Address,
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

/// VM Error types
#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("Invalid bytecode: {0}")]
    InvalidBytecode(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Out of gas")]
    OutOfGas,

    #[error("Contract not found: {0}")]
    ContractNotFound(String),

    #[error("Invalid method: {0}")]
    InvalidMethod(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("VM internal error: {0}")]
    InternalError(String),
}

/// VM Engine Manager - manages multiple VM implementations
pub struct VmEngineManager {
    engines: HashMap<VmType, Box<dyn VmEngine>>,
    default_vm: VmType,
}

impl VmEngineManager {
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
            default_vm: VmType::Wasm,
        }
    }

    /// Register a VM engine
    pub fn register_engine(&mut self, engine: Box<dyn VmEngine>) -> Result<(), String> {
        let vm_type = engine.vm_type();
        if self.engines.contains_key(&vm_type) {
            return Err(format!("VM engine for type {:?} already registered", vm_type));
        }
        self.engines.insert(vm_type, engine);
        Ok(())
    }

    /// Set default VM
    pub fn set_default_vm(&mut self, vm_type: VmType) -> Result<(), String> {
        if !self.engines.contains_key(&vm_type) {
            return Err(format!("VM engine for type {:?} not registered", vm_type));
        }
        self.default_vm = vm_type;
        Ok(())
    }

    /// Get VM engine by type
    pub fn get_engine(&self, vm_type: VmType) -> Option<&dyn VmEngine> {
        self.engines.get(&vm_type).map(|e| e.as_ref())
    }

    /// Get default VM engine
    pub fn get_default_engine(&self) -> &dyn VmEngine {
        self.engines.get(&self.default_vm).expect("Default VM not registered")
    }

    /// Auto-detect VM type from bytecode and get appropriate engine
    pub fn get_engine_for_bytecode(&self, bytecode: &[u8]) -> &dyn VmEngine {
        // Simple detection logic - can be extended
        if bytecode.starts_with(&[0x00, 0x61, 0x73, 0x6D]) { // WASM magic bytes
            self.get_engine(VmType::Wasm).unwrap_or(self.get_default_engine())
        } else {
            self.get_default_engine()
        }
    }

    /// List registered VM types
    pub fn list_vm_types(&self) -> Vec<VmType> {
        self.engines.keys().cloned().collect()
    }
}

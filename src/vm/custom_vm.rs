use crate::vm::engine::{VmEngine, VmType, BlockContext, ContractInstance, ExecutionResult, ContractLog, VmError};
use crate::core::Address;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Storage for Custom VM contracts
pub trait CustomStorage: Send + Sync {
    fn store_contract(&self, address: &Address, bytecode: &[u8]) -> Result<(), VmError>;
    fn load_contract(&self, address: &Address) -> Result<Option<Vec<u8>>, VmError>;
    fn store_state(&self, address: &Address, key: &[u8], value: &[u8]) -> Result<(), VmError>;
    fn load_state(&self, address: &Address, key: &[u8]) -> Result<Option<Vec<u8>>, VmError>;
}

/// Simple in-memory storage for Custom VM
#[derive(Debug, Clone)]
pub struct InMemoryCustomStorage {
    contracts: Arc<RwLock<HashMap<Address, Vec<u8>>>>,
    states: Arc<RwLock<HashMap<(Address, Vec<u8>), Vec<u8>>>>,
}

impl InMemoryCustomStorage {
    pub fn new() -> Self {
        Self {
            contracts: Arc::new(RwLock::new(HashMap::new())),
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl CustomStorage for InMemoryCustomStorage {
    fn store_contract(&self, address: &Address, bytecode: &[u8]) -> Result<(), VmError> {
        self.contracts.write().unwrap().insert(*address, bytecode.to_vec());
        Ok(())
    }

    fn load_contract(&self, address: &Address) -> Result<Option<Vec<u8>>, VmError> {
        Ok(self.contracts.read().unwrap().get(address).cloned())
    }

    fn store_state(&self, address: &Address, key: &[u8], value: &[u8]) -> Result<(), VmError> {
        self.states.write().unwrap().insert((*address, key.to_vec()), value.to_vec());
        Ok(())
    }

    fn load_state(&self, address: &Address, key: &[u8]) -> Result<Option<Vec<u8>>, VmError> {
        Ok(self.states.read().unwrap().get(&(*address, key.to_vec())).cloned())
    }
}

/// Custom VM Engine - simple interpreted VM for demonstration
pub struct CustomVmEngine<S: CustomStorage> {
    storage: Arc<S>,
}

impl<S: CustomStorage> CustomVmEngine<S> {
    pub fn new(storage: Arc<S>) -> Self {
        Self { storage }
    }
}

impl<S: CustomStorage> VmEngine for CustomVmEngine<S> {
    fn vm_type(&self) -> VmType {
        VmType::Custom
    }

    fn deploy_contract(
        &self,
        bytecode: &[u8],
        deployer: &Address,
        gas_limit: u64,
        block_context: &BlockContext,
    ) -> Result<ContractInstance, VmError> {
        // Validate custom bytecode
        self.validate_bytecode(bytecode)?;

        // Generate contract address
        let contract_address = self.generate_contract_address(deployer, bytecode);

        // Store contract
        self.storage.store_contract(&contract_address, bytecode)?;

        let code_hash = self.hash_bytecode(bytecode);

        Ok(ContractInstance {
            address: contract_address,
            vm_type: VmType::Custom,
            code_hash,
        })
    }

    fn execute_contract(
        &self,
        contract_address: &Address,
        method: &str,
        args: &[u8],
        caller: &Address,
        gas_limit: u64,
        block_context: &BlockContext,
    ) -> Result<ExecutionResult, VmError> {
        // Load contract bytecode
        let bytecode = self.storage.load_contract(contract_address)?
            .ok_or_else(|| VmError::ContractNotFound(hex::encode(contract_address)))?;

        // Simple interpretation of custom bytecode
        let result = self.interpret_bytecode(&bytecode, method, args, caller, gas_limit)?;

        Ok(ExecutionResult {
            return_data: result,
            gas_used: gas_limit / 2, // Simplified gas calculation
            logs: vec![],
            success: true,
        })
    }

    fn get_contract_code(&self, contract_address: &Address) -> Result<Option<Vec<u8>>, VmError> {
        self.storage.load_contract(contract_address)
    }

    fn contract_exists(&self, contract_address: &Address) -> bool {
        self.storage.load_contract(contract_address).unwrap_or(None).is_some()
    }

    fn validate_bytecode(&self, bytecode: &[u8]) -> Result<(), VmError> {
        // Custom validation - check for magic bytes or structure
        if bytecode.is_empty() {
            return Err(VmError::InvalidBytecode("Empty bytecode".to_string()));
        }

        // Check for custom VM magic bytes
        if !bytecode.starts_with(&[0x43, 0x56, 0x4D]) { // "CVM"
            return Err(VmError::InvalidBytecode("Not valid Custom VM bytecode".to_string()));
        }

        Ok(())
    }

    fn supported_contract_types(&self) -> Vec<String> {
        vec!["custom".to_string(), "simple".to_string()]
    }
}

impl<S: CustomStorage> CustomVmEngine<S> {
    fn generate_contract_address(&self, deployer: &Address, bytecode: &[u8]) -> Address {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(deployer);
        hasher.update(bytecode);
        hasher.update(b"custom");
        let hash = hasher.finalize();
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash[..20]);
        address
    }

    fn hash_bytecode(&self, bytecode: &[u8]) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytecode);
        hasher.finalize().into()
    }

    fn interpret_bytecode(
        &self,
        bytecode: &[u8],
        method: &str,
        args: &[u8],
        caller: &Address,
        gas_limit: u64,
    ) -> Result<Vec<u8>, VmError> {
        // Very simple interpreter for demonstration
        // In real implementation, this would be a proper bytecode interpreter

        match method {
            "add" => {
                if args.len() < 8 {
                    return Err(VmError::ExecutionFailed("Invalid args for add".to_string()));
                }
                let a = u64::from_le_bytes(args[..8].try_into().unwrap());
                let b = u64::from_le_bytes(args[8..16].try_into().unwrap());
                Ok((a + b).to_le_bytes().to_vec())
            }
            "store" => {
                if args.len() < 32 {
                    return Err(VmError::ExecutionFailed("Invalid args for store".to_string()));
                }
                let key = &args[..32];
                let value = &args[32..];
                // In real implementation, would store in contract state
                Ok(vec![1]) // Success
            }
            "load" => {
                if args.len() < 32 {
                    return Err(VmError::ExecutionFailed("Invalid args for load".to_string()));
                }
                let key = &args[..32];
                // In real implementation, would load from contract state
                Ok(vec![0; 32]) // Dummy value
            }
            _ => Err(VmError::InvalidMethod(method.to_string())),
        }
    }
}
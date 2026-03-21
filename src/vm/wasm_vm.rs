use crate::vm::engine::{VmEngine, VmType, BlockContext, ContractInstance, ExecutionResult, ContractLog, VmError};
use crate::vm::wasm_runtime::{WasmRuntime, RuntimeState, Storage};
use crate::core::Address;
use std::sync::Arc;

/// WASM VM Engine implementation
pub struct WasmVmEngine<S: Storage + 'static> {
    runtime: WasmRuntime,
    storage: Arc<S>,
}

impl<S: Storage + 'static> WasmVmEngine<S> {
    pub fn new(storage: Arc<S>) -> Result<Self, VmError> {
        let runtime = WasmRuntime::new().map_err(|e| VmError::InternalError(e.to_string()))?;

        Ok(Self {
            runtime,
            storage,
        })
    }
}

impl<S: Storage + 'static> VmEngine for WasmVmEngine<S> {
    fn vm_type(&self) -> VmType {
        VmType::Wasm
    }

    fn deploy_contract(
        &self,
        bytecode: &[u8],
        deployer: &Address,
        gas_limit: u64,
        block_context: &BlockContext,
    ) -> Result<ContractInstance, VmError> {
        // Validate WASM bytecode
        self.validate_bytecode(bytecode)?;

        // Generate contract address (simplified)
        let contract_address = self.generate_contract_address(deployer, bytecode);

        // Store contract code
        self.storage.write(&contract_address, bytecode);

        // Create contract instance
        let code_hash = self.hash_bytecode(bytecode);

        Ok(ContractInstance {
            address: contract_address,
            vm_type: VmType::Wasm,
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
        // Get contract code
        let bytecode = self.storage.read(contract_address)
            .ok_or_else(|| VmError::ContractNotFound(hex::encode(contract_address)))?;

        // Create runtime state
        let mut gas_meter = crate::vm::gas_meter::GasMeter::new(gas_limit);
        let runtime_state = RuntimeState {
            contract_address: *contract_address,
            caller: *caller,
            block_height: block_context.block_number,
            timestamp: block_context.timestamp,
            storage: self.storage.box_clone(),
            gas_meter,
            memory_limiter: None,
        };

        // Instantiate contract
        let (mut store, instance) = self.runtime.instantiate_contract(
            &bytecode,
            runtime_state,
            gas_limit,
        ).map_err(|e| VmError::ExecutionFailed(e.to_string()))?;

        // Call method
        let result = self.runtime.call_function(
            &mut store,
            &instance,
            method,
            &[],
        ).map_err(|e| VmError::ExecutionFailed(e.to_string()))?;

        // Extract return data
        let return_data = if let Some(wasmtime::Val::I32(ptr)) = result.get(0) {
            // For simplicity, assume return data is at memory location
            // In real implementation, would need proper memory handling
            vec![]
        } else {
            vec![]
        };

        let gas_used = gas_limit - store.data().gas_meter.remaining_gas();

        Ok(ExecutionResult {
            return_data,
            gas_used,
            logs: vec![], // Would collect logs from execution
            success: true,
        })
    }

    fn get_contract_code(&self, contract_address: &Address) -> Result<Option<Vec<u8>>, VmError> {
        Ok(self.storage.read(contract_address))
    }

    fn contract_exists(&self, contract_address: &Address) -> bool {
        self.storage.read(contract_address).is_some()
    }

    fn validate_bytecode(&self, bytecode: &[u8]) -> Result<(), VmError> {
        // Check WASM magic bytes
        if !bytecode.starts_with(&[0x00, 0x61, 0x73, 0x6D]) {
            return Err(VmError::InvalidBytecode("Not valid WASM bytecode".to_string()));
        }

        // Additional validation can be added here
        Ok(())
    }

    fn supported_contract_types(&self) -> Vec<String> {
        vec!["wasm".to_string()]
    }
}

impl<S: Storage + 'static> WasmVmEngine<S> {
    fn generate_contract_address(&self, deployer: &Address, bytecode: &[u8]) -> Address {
        // Simple address generation - in production, use proper CREATE2-like scheme
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(deployer);
        hasher.update(bytecode);
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
}
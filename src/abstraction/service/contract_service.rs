use crate::contracts::contract_registry::ContractRegistry;
use crate::core::Address;
use crate::rpc::interfaces::*;
use parking_lot::Mutex;
use std::sync::Arc;

pub struct ContractService {
    contract_registry: Arc<Mutex<ContractRegistry>>,
    blockchain_service: Arc<dyn BlockchainInterface>,
}

impl ContractService {
    pub fn new(
        contract_registry: Arc<Mutex<ContractRegistry>>,
        blockchain_service: Arc<dyn BlockchainInterface>,
    ) -> Self {
        ContractService {
            contract_registry,
            blockchain_service,
        }
    }
}

#[async_trait::async_trait]
impl ContractInterface for ContractService {
    async fn get_contract_info(&self, _address: Address) -> Result<Option<ContractInfo>, RpcError> {
        let _ = self.contract_registry.lock();
        Ok(None)
    }

    async fn list_contracts(&self) -> Result<Vec<ContractInfo>, RpcError> {
        Ok(vec![])
    }

    async fn deploy_contract(
        &self,
        _bytecode: Vec<u8>,
        _constructor_args: Vec<u8>,
        _sender: Address,
    ) -> Result<Address, RpcError> {
        Err(RpcError::InternalError("Contract deployment not implemented".to_string()))
    }

    async fn call_contract(
        &self,
        _address: Address,
        _method: String,
        _args: Vec<u8>,
        _sender: Address,
    ) -> Result<Vec<u8>, RpcError> {
        Err(RpcError::InternalError("Contract calling not implemented".to_string()))
    }
}

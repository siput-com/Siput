//! RPC interface wiring and DI helpers
//!
//! This module moves concrete implementations into abstraction/service/* and re-exports them for RPC.

pub use crate::abstraction::service::blockchain_service::BlockchainService as BlockchainServiceImpl;
pub use crate::abstraction::service::contract_service::ContractService as ContractServiceImpl;
pub use crate::abstraction::service::node_service::NodeService as NodeServiceImpl;

use crate::rpc::interfaces::*;
use crate::core::{Transaction, Address, Block, BlockHash};
use async_trait::async_trait;
use std::sync::Arc;

/// Combined RPC service implementation
pub struct RpcService {
    pub blockchain: Arc<dyn BlockchainInterface>,
    pub contracts: Arc<dyn ContractInterface>,
    pub network: Arc<dyn NetworkInterface>,
}

impl RpcService {
    pub fn new(
        blockchain: Arc<dyn BlockchainInterface>,
        contracts: Arc<dyn ContractInterface>,
        network: Arc<dyn NetworkInterface>,
    ) -> Self {
        RpcService {
            blockchain,
            contracts,
            network,
        }
    }

    pub fn blockchain(&self) -> Arc<dyn BlockchainInterface> {
        self.blockchain.clone()
    }

    pub fn contracts(&self) -> Arc<dyn ContractInterface> {
        self.contracts.clone()
    }

    pub fn network(&self) -> Arc<dyn NetworkInterface> {
        self.network.clone()
    }
}

#[async_trait]
impl BlockchainInterface for RpcService {
    async fn submit_transaction(&self, tx: Transaction) -> Result<(), RpcError> {
        self.blockchain.submit_transaction(tx).await
    }

    async fn get_balance(&self, address: Address) -> Result<u64, RpcError> {
        self.blockchain.get_balance(address).await
    }

    async fn get_nonce(&self, address: Address) -> Result<u64, RpcError> {
        self.blockchain.get_nonce(address).await
    }

    async fn get_block(&self, hash: BlockHash) -> Result<Option<Block>, RpcError> {
        self.blockchain.get_block(hash).await
    }

    async fn get_transaction(&self, hash: [u8; 32]) -> Result<Option<Transaction>, RpcError> {
        self.blockchain.get_transaction(hash).await
    }

    async fn get_transaction_status(&self, hash: [u8; 32]) -> Result<TransactionStatus, RpcError> {
        self.blockchain.get_transaction_status(hash).await
    }

    async fn get_dag_info(&self) -> Result<DagInfo, RpcError> {
        self.blockchain.get_dag_info().await
    }

    async fn get_node_info(&self) -> Result<NodeInfo, RpcError> {
        self.blockchain.get_node_info().await
    }

    async fn get_mempool_info(&self) -> Result<MempoolInfo, RpcError> {
        self.blockchain.get_mempool_info().await
    }
}

#[async_trait]
impl ContractInterface for RpcService {
    async fn get_contract_info(&self, address: Address) -> Result<Option<ContractInfo>, RpcError> {
        self.contracts.get_contract_info(address).await
    }

    async fn list_contracts(&self) -> Result<Vec<ContractInfo>, RpcError> {
        self.contracts.list_contracts().await
    }

    async fn deploy_contract(&self, bytecode: Vec<u8>, constructor_args: Vec<u8>, sender: Address) -> Result<Address, RpcError> {
        self.contracts.deploy_contract(bytecode, constructor_args, sender).await
    }

    async fn call_contract(&self, address: Address, method: String, args: Vec<u8>, sender: Address) -> Result<Vec<u8>, RpcError> {
        self.contracts.call_contract(address, method, args, sender).await
    }
}

#[async_trait]
impl NetworkInterface for RpcService {
    async fn get_connected_peers(&self) -> Result<Vec<String>, RpcError> {
        self.network.get_connected_peers().await
    }

    async fn get_network_stats(&self) -> Result<NetworkStats, RpcError> {
        self.network.get_network_stats().await
    }
}

impl RpcInterface for RpcService {}


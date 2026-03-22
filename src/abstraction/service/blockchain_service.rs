use crate::core::{Address, Block, Transaction};
use crate::dag::blockdag::BlockDAG;
use crate::mempool::tx_dag_mempool::TxDagMempool;
use crate::state::state_manager::StateManager;
use crate::rpc::interfaces::*;
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

pub struct BlockchainService {
    dag: Arc<RwLock<BlockDAG>>,
    mempool: Arc<TxDagMempool>,
    state_manager: Arc<Mutex<StateManager>>,
}

impl BlockchainService {
    pub fn new(
        dag: Arc<RwLock<BlockDAG>>,
        mempool: Arc<TxDagMempool>,
        state_manager: Arc<Mutex<StateManager>>,
    ) -> Self {
        BlockchainService {
            dag,
            mempool,
            state_manager,
        }
    }
}

#[async_trait::async_trait]
impl BlockchainInterface for BlockchainService {
    async fn submit_transaction(&self, tx: Transaction) -> Result<(), RpcError> {
        tx.validate_basic().map_err(|e| RpcError::ValidationError(e))?;

        self.mempool
            .add_transaction(tx, vec![], None)
            .map_err(|e| RpcError::InternalError(format!("Failed to add transaction: {}", e)))
    }

    async fn get_balance(&self, address: Address) -> Result<u64, RpcError> {
        let state = self.state_manager.lock();
        Ok(state.get_balance(address))
    }

    async fn get_nonce(&self, address: Address) -> Result<u64, RpcError> {
        let state = self.state_manager.lock();
        Ok(state.get_nonce(address))
    }

    async fn get_block(&self, hash: [u8; 32]) -> Result<Option<Block>, RpcError> {
        let dag = self.dag.read();
        Ok(dag.get_block(&hash))
    }

    async fn get_transaction(&self, hash: [u8; 32]) -> Result<Option<Transaction>, RpcError> {
        if let Some(mempool_tx) = self.mempool.get_transaction(&hash) {
            return Ok(Some(mempool_tx.transaction.clone()));
        }

        let dag = self.dag.read();
        for block in dag.get_all_blocks() {
            for tx in &block.transactions {
                if tx.hash() == hash {
                    return Ok(Some(tx.clone()));
                }
            }
        }

        Ok(None)
    }

    async fn get_transaction_status(&self, hash: [u8; 32]) -> Result<TransactionStatus, RpcError> {
        if self.mempool.get_transaction(&hash).is_some() {
            return Ok(TransactionStatus::Pending);
        }

        let dag = self.dag.read();
        for block in dag.get_all_blocks() {
            for tx in &block.transactions {
                if tx.hash() == hash {
                    return Ok(TransactionStatus::Confirmed {
                        block_hash: block.hash(),
                        block_height: block.height(),
                    });
                }
            }
        }

        Err(RpcError::NotFound("Transaction not found".to_string()))
    }

    async fn get_dag_info(&self) -> Result<DagInfo, RpcError> {
        let dag = self.dag.read();
        let tips: Vec<String> = dag.get_tips().into_iter().map(|h| hex::encode(h)).collect();
        let total_blocks = dag.get_all_blocks().len();
        let height = dag.get_height();

        let stats = serde_json::json!({
            "tips_count": tips.len(),
            "total_blocks": total_blocks,
            "orphans": 0,
            "avg_block_time": 5000
        });

        Ok(DagInfo {
            tips,
            total_blocks,
            height,
            stats,
        })
    }

    async fn get_node_info(&self) -> Result<NodeInfo, RpcError> {
        let mempool_size = self.mempool.tx_count();
        let dag = self.dag.read();
        let dag_height = dag.block_count();

        Ok(NodeInfo {
            peer_id: "unknown".to_string(),
            connected_peers: vec![],
            mempool_size,
            dag_height,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime: 0,
        })
    }

    async fn get_mempool_info(&self) -> Result<MempoolInfo, RpcError> {
        let tx_count = self.mempool.tx_count();
        let tx_hashes: Vec<String> = self
            .mempool
            .get_all_transactions()
            .into_iter()
            .map(|tx| hex::encode(tx.hash()))
            .collect();

        let total_gas = self
            .mempool
            .get_all_transactions()
            .into_iter()
            .map(|tx| tx.gas_limit)
            .sum();

        Ok(MempoolInfo {
            tx_count,
            tx_hashes,
            total_gas,
        })
    }
}

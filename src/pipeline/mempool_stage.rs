use crate::core::Transaction;
use crate::pipeline::{TransactionPipelineStage, PipelineContext, PipelineResult};
use crate::mempool::TxDagMempool;
use async_trait::async_trait;
use std::sync::Arc;

/// Stage mempool untuk transaksi
pub struct MempoolStage {
    mempool: Arc<TxDagMempool>,
}

impl MempoolStage {
    pub fn new(mempool: Arc<TxDagMempool>) -> Self {
        Self { mempool }
    }
}

#[async_trait]
impl TransactionPipelineStage for MempoolStage {
    fn name(&self) -> &'static str {
        "mempool"
    }

    async fn process(&self, context: &mut PipelineContext) -> Result<PipelineResult, String> {
        let tx = &context.transaction;

        // Tambahkan transaksi ke mempool
        self.mempool.add_transaction(tx.clone(), vec![], None)?;

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("mempool_added".to_string(), "true".to_string());
        metadata.insert("mempool_size".to_string(), self.mempool.size().to_string());

        Ok(PipelineResult {
            success: true,
            transaction_hash: Some(tx.hash()),
            error_message: None,
            metadata,
        })
    }
}
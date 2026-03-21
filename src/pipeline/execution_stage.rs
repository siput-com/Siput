use crate::core::Transaction;
use crate::pipeline::{TransactionPipelineStage, PipelineContext, PipelineResult};
use crate::execution::transaction_executor::TransactionExecutor;
use async_trait::async_trait;
use std::sync::Arc;

/// Stage eksekusi transaksi
pub struct ExecutionStage {
    executor: Arc<parking_lot::Mutex<TransactionExecutor>>,
}

impl ExecutionStage {
    pub fn new(executor: Arc<parking_lot::Mutex<TransactionExecutor>>) -> Self {
        Self { executor }
    }
}

#[async_trait]
impl TransactionPipelineStage for ExecutionStage {
    fn name(&self) -> &'static str {
        "execution"
    }

    async fn process(&self, context: &mut PipelineContext) -> Result<PipelineResult, String> {
        let tx = &context.transaction;

        // Untuk eksekusi individual, kita perlu membuat blok sementara
        // atau menggunakan executor untuk single transaction
        // Untuk sekarang, kita simulasikan eksekusi

        let mut executor = self.executor.lock();

        // Note: Dalam implementasi penuh, kita perlu method untuk execute single transaction
        // Untuk sekarang, kita asumsikan eksekusi berhasil
        let gas_used = tx.gas_limit.min(21000); // Basic gas for transfer

        context.gas_used = gas_used;

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("gas_used".to_string(), gas_used.to_string());
        metadata.insert("execution_success".to_string(), "true".to_string());

        Ok(PipelineResult {
            success: true,
            transaction_hash: Some(tx.hash()),
            error_message: None,
            metadata,
        })
    }
}
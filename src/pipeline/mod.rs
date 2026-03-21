use crate::core::Transaction;
use async_trait::async_trait;
use std::sync::Arc;

/// Result dari pemrosesan stage pipeline
#[derive(Debug, Clone)]
pub struct PipelineResult {
    pub success: bool,
    pub transaction_hash: Option<[u8; 32]>,
    pub error_message: Option<String>,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Context yang dibagikan antar stages
#[derive(Clone)]
pub struct PipelineContext {
    pub transaction: Transaction,
    pub block_hash: Option<[u8; 32]>,
    pub gas_used: u64,
    pub execution_result: Option<crate::execution::transaction_executor::ExecutionResult>,
}

/// Stage dalam pipeline transaksi
#[async_trait]
pub trait TransactionPipelineStage: Send + Sync {
    /// Nama stage untuk logging dan debugging
    fn name(&self) -> &'static str;

    /// Proses transaksi melalui stage ini
    async fn process(&self, context: &mut PipelineContext) -> Result<PipelineResult, String>;

    /// Apakah stage ini required atau optional
    fn is_required(&self) -> bool { true }

    /// Timeout untuk stage ini (dalam ms)
    fn timeout_ms(&self) -> u64 { 5000 }
}

/// Manager untuk mengatur pipeline transaksi
pub struct TransactionPipelineManager {
    stages: Vec<Box<dyn TransactionPipelineStage>>,
}

impl TransactionPipelineManager {
    /// Buat pipeline manager baru
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
        }
    }

    /// Tambahkan stage ke pipeline
    pub fn add_stage(&mut self, stage: Box<dyn TransactionPipelineStage>) {
        self.stages.push(stage);
    }

    /// Proses transaksi melalui semua stages
    pub async fn process_transaction(&self, transaction: Transaction) -> Result<PipelineResult, String> {
        let mut context = PipelineContext {
            transaction,
            block_hash: None,
            gas_used: 0,
            execution_result: None,
        };

        let mut final_result = PipelineResult {
            success: true,
            transaction_hash: Some(context.transaction.hash()),
            error_message: None,
            metadata: std::collections::HashMap::new(),
        };

        for stage in &self.stages {
            let stage_name = stage.name();
            tracing::debug!("Processing transaction through stage: {}", stage_name);

            let result = match tokio::time::timeout(
                std::time::Duration::from_millis(stage.timeout_ms()),
                stage.process(&mut context)
            ).await {
                Ok(Ok(result)) => result,
                Ok(Err(e)) => {
                    if stage.is_required() {
                        return Err(format!("Stage {} failed: {}", stage_name, e));
                    } else {
                        tracing::warn!("Optional stage {} failed: {}", stage_name, e);
                        continue;
                    }
                }
                Err(_) => {
                    if stage.is_required() {
                        return Err(format!("Stage {} timed out", stage_name));
                    } else {
                        tracing::warn!("Optional stage {} timed out", stage_name);
                        continue;
                    }
                }
            };

            if !result.success && stage.is_required() {
                final_result.success = false;
                final_result.error_message = result.error_message;
                break;
            }

            // Merge metadata
            for (key, value) in result.metadata {
                final_result.metadata.insert(key, value);
            }
        }

        Ok(final_result)
    }

    /// Get list of stage names
    pub fn get_stage_names(&self) -> Vec<&str> {
        self.stages.iter().map(|s| s.name()).collect()
    }
}

impl Default for TransactionPipelineManager {
    fn default() -> Self {
        Self::new()
    }
}

// Export stages
pub mod validation_stage;
pub mod mempool_stage;
pub mod execution_stage;
pub mod state_update_stage;
pub mod finality_stage;

pub use validation_stage::ValidationStage;
pub use mempool_stage::MempoolStage;
pub use execution_stage::ExecutionStage;
pub use state_update_stage::StateUpdateStage;
pub use finality_stage::FinalityStage;
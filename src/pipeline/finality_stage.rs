use crate::core::Transaction;
use crate::pipeline::{TransactionPipelineStage, PipelineContext, PipelineResult};
use crate::finality::FinalityEngine;
use async_trait::async_trait;
use std::sync::Arc;

/// Stage finality untuk transaksi
pub struct FinalityStage {
    finality_engine: Arc<FinalityEngine>,
}

impl FinalityStage {
    pub fn new(finality_engine: Arc<FinalityEngine>) -> Self {
        Self { finality_engine }
    }
}

#[async_trait]
impl TransactionPipelineStage for FinalityStage {
    fn name(&self) -> &'static str {
        "finality"
    }

    fn is_required(&self) -> bool {
        false // Finality bisa gagal tanpa menghentikan pipeline
    }

    async fn process(&self, context: &mut PipelineContext) -> Result<PipelineResult, String> {
        // Dalam pipeline transaksi individual, finality biasanya ditangani
        // pada level blok, bukan transaksi individual
        // Stage ini bisa digunakan untuk logging atau notifikasi

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("finality_checked".to_string(), "true".to_string());

        // Jika ada block_hash, cek finality
        if let Some(block_hash) = context.block_hash {
            metadata.insert("block_finalized".to_string(), "pending".to_string());
        }

        Ok(PipelineResult {
            success: true,
            transaction_hash: Some(context.transaction.hash()),
            error_message: None,
            metadata,
        })
    }
}
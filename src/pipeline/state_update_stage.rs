use crate::core::Transaction;
use crate::pipeline::{TransactionPipelineStage, PipelineContext, PipelineResult};
use crate::state::state_manager::StateManager;
use async_trait::async_trait;
use std::sync::Arc;

/// Stage update state
pub struct StateUpdateStage {
    state_manager: Arc<parking_lot::Mutex<StateManager>>,
}

impl StateUpdateStage {
    pub fn new(state_manager: Arc<parking_lot::Mutex<StateManager>>) -> Self {
        Self { state_manager }
    }
}

#[async_trait]
impl TransactionPipelineStage for StateUpdateStage {
    fn name(&self) -> &'static str {
        "state_update"
    }

    async fn process(&self, context: &mut PipelineContext) -> Result<PipelineResult, String> {
        let tx = &context.transaction;
        let gas_used = context.gas_used;

        let mut state = self.state_manager.lock();

        // Update nonce
        let current_nonce = state.get_nonce(tx.from);
        state.set_nonce(tx.from, current_nonce + 1);

        // Deduct gas fee
        let gas_fee = gas_used.saturating_mul(tx.gas_price);
        let total_deduct = tx.amount.saturating_add(gas_fee);

        let sender_balance = state.get_balance(tx.from);
        if sender_balance < total_deduct {
            return Err("Insufficient balance for execution".to_string());
        }

        state.deduct_balance(tx.from, total_deduct)?;
        state.credit_account(tx.to, tx.amount)?;

        // Credit miner dengan gas fee (dalam kasus ini, miner adalah node sendiri)
        // Dalam implementasi penuh, ini akan dikredit ke miner dari blok
        state.add_emitted_supply(gas_fee);

        // Emit transaction confirmed event
        let emitter = crate::events::GlobalEventEmitter::instance();
        emitter.emit_transaction_confirmed(
            tx.clone(),
            [0u8; 32], // block_hash will be set by caller
            "pipeline".to_string()
        ).await;

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("state_updated".to_string(), "true".to_string());
        metadata.insert("new_nonce".to_string(), (current_nonce + 1).to_string());
        metadata.insert("gas_fee".to_string(), gas_fee.to_string());
        metadata.insert("new_state_root".to_string(), hex::encode(state.get_state_root()));

        Ok(PipelineResult {
            success: true,
            transaction_hash: Some(tx.hash()),
            error_message: None,
            metadata,
        })
    }
}
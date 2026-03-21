use crate::core::Transaction;
use crate::pipeline::{TransactionPipelineStage, PipelineContext, PipelineResult};
use async_trait::async_trait;
use std::sync::Arc;

/// Stage validasi transaksi
pub struct ValidationStage {
    state_manager: Arc<parking_lot::Mutex<crate::state::state_manager::StateManager>>,
}

impl ValidationStage {
    pub fn new(state_manager: Arc<parking_lot::Mutex<crate::state::state_manager::StateManager>>) -> Self {
        Self { state_manager }
    }

    /// Validasi signature transaksi
    fn validate_signature(&self, tx: &Transaction) -> Result<(), String> {
        if tx.signature.data.is_empty() {
            return Err("Transaction must be signed".to_string());
        }

        // Validasi signature menggunakan secp256k1
        let message = tx.signing_message();
        let signature = &tx.signature;

        // Recover public key dari signature
        use secp256k1::{Secp256k1, Message, Signature, RecoveryId};
        let secp = Secp256k1::new();
        let msg = Message::from_digest_slice(&message).map_err(|e| format!("Invalid message: {}", e))?;
        let sig = Signature::from_compact(&signature.data).map_err(|e| format!("Invalid signature: {}", e))?;
        let recovery_id = RecoveryId::from_i32(signature.recovery_id as i32).map_err(|e| format!("Invalid recovery id: {}", e))?;

        let recovered_key = secp.recover_ecdsa(&msg, &sig, &recovery_id)
            .map_err(|e| format!("Signature recovery failed: {}", e))?;

        // Convert public key to address (keccak256 hash, take last 20 bytes)
        use sha3::{Digest, Keccak256};
        let pubkey_bytes = &recovered_key.serialize()[1..]; // Skip 0x04 prefix
        let hash = Keccak256::digest(pubkey_bytes);
        let mut pubkey_hash = [0u8; 20];
        pubkey_hash.copy_from_slice(&hash[12..32]);

        if pubkey_hash != tx.from {
            return Err("Signature does not match sender address".to_string());
        }

        Ok(())
    }

    /// Validasi balance dan nonce
    fn validate_balance_and_nonce(&self, tx: &Transaction) -> Result<(), String> {
        let state = self.state_manager.lock();

        // Cek balance
        let balance = state.get_balance(tx.from);
        let required = tx.amount.saturating_add(tx.gas_limit.saturating_mul(tx.gas_price));
        if balance < required {
            return Err(format!("Insufficient balance: {} < {}", balance, required));
        }

        // Cek nonce
        let current_nonce = state.get_nonce(tx.from);
        if tx.nonce < current_nonce {
            return Err(format!("Nonce too low: {} < {}", tx.nonce, current_nonce));
        }

        Ok(())
    }

    /// Validasi format dan batasan transaksi
    fn validate_format(&self, tx: &Transaction) -> Result<(), String> {
        // Cek ukuran transaksi
        if tx.data.len() > 128 * 1024 { // 128KB limit
            return Err("Transaction data too large".to_string());
        }

        // Cek gas limit
        if tx.gas_limit == 0 || tx.gas_limit > 10_000_000 {
            return Err("Invalid gas limit".to_string());
        }

        // Cek amount tidak overflow
        if tx.amount > u64::MAX / 2 {
            return Err("Amount too large".to_string());
        }

        Ok(())
    }
}

#[async_trait]
impl TransactionPipelineStage for ValidationStage {
    fn name(&self) -> &'static str {
        "validation"
    }

    async fn process(&self, context: &mut PipelineContext) -> Result<PipelineResult, String> {
        let tx = &context.transaction;

        // Validasi signature
        self.validate_signature(tx)?;

        // Validasi format
        self.validate_format(tx)?;

        // Validasi balance dan nonce
        self.validate_balance_and_nonce(tx)?;

        // Emit validation success event
        let emitter = crate::events::GlobalEventEmitter::instance();
        emitter.emit_custom(
            "transaction_validated".to_string(),
            serde_json::json!({
                "transaction_hash": hex::encode(tx.hash()),
                "sender": hex::encode(tx.from),
                "amount": tx.amount,
                "gas_price": tx.gas_price
            }),
            "pipeline".to_string()
        ).await;

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("validation_passed".to_string(), "true".to_string());
        metadata.insert("sender_balance".to_string(), self.state_manager.lock().get_balance(tx.from).to_string());
        metadata.insert("sender_nonce".to_string(), self.state_manager.lock().get_nonce(tx.from).to_string());

        Ok(PipelineResult {
            success: true,
            transaction_hash: Some(tx.hash()),
            error_message: None,
            metadata,
        })
    }
}
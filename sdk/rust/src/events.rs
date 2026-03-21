//! Event system integration for Siput SDK
//!
//! This module provides high-level APIs for subscribing to blockchain events
//! and real-time updates in dApps and wallets.

use siput_core::{GlobalEventListener, Event, EventType};
use std::sync::Arc;
use tokio::sync::mpsc;

/// SDK Event listener with high-level APIs
pub struct SdkEventListener {
    listener: GlobalEventListener,
    event_sender: mpsc::UnboundedSender<Event>,
    _event_receiver: mpsc::UnboundedReceiver<Event>,
}

impl SdkEventListener {
    /// Create new SDK event listener
    pub fn new(client_id: String) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        SdkEventListener {
            listener: GlobalEventListener::new(client_id),
            event_sender: tx,
            _event_receiver: rx,
        }
    }

    /// Subscribe to new blocks
    pub async fn on_new_blocks<F>(&self, callback: F) -> Result<(), String>
    where
        F: Fn(siput_core::Block, u64) -> () + Send + Sync + 'static,
    {
        self.listener.on_block_created(move |event| {
            let callback = callback.clone();
            async move {
                if let EventType::BlockCreated { block, block_height } = &event.event_type {
                    callback(block.clone(), *block_height);
                }
            }
        }).await;
        Ok(())
    }

    /// Subscribe to transaction confirmations
    pub async fn on_transaction_confirmations<F>(&self, callback: F) -> Result<(), String>
    where
        F: Fn(siput_core::Transaction, siput_core::BlockHash) -> () + Send + Sync + 'static,
    {
        self.listener.on_transaction_confirmed(move |event| {
            let callback = callback.clone();
            async move {
                if let EventType::TransactionConfirmed { transaction, block_hash, .. } = &event.event_type {
                    callback(transaction.clone(), *block_hash);
                }
            }
        }).await;
        Ok(())
    }

    /// Subscribe to contract executions
    pub async fn on_contract_executions<F>(&self, callback: F) -> Result<(), String>
    where
        F: Fn(siput_core::Address, String, Vec<u8>, u64) -> () + Send + Sync + 'static,
    {
        self.listener.on_contract_executed(move |event| {
            let callback = callback.clone();
            async move {
                if let EventType::ContractExecuted { contract_address, method, result, gas_used } = &event.event_type {
                    callback(*contract_address, method.clone(), result.clone(), *gas_used);
                }
            }
        }).await;
        Ok(())
    }

    /// Subscribe to custom events
    pub async fn on_custom_events<F>(&self, event_name: String, callback: F) -> Result<(), String>
    where
        F: Fn(serde_json::Value) -> () + Send + Sync + 'static,
    {
        self.listener.on_custom_event(event_name, move |event| {
            let callback = callback.clone();
            async move {
                if let EventType::Custom { data, .. } = &event.event_type {
                    callback(data.clone());
                }
            }
        }).await;
        Ok(())
    }

    /// Get raw event stream for advanced usage
    pub fn event_stream(&self) -> mpsc::UnboundedReceiver<Event> {
        let (_, rx) = mpsc::unbounded_channel();
        // In real implementation, this would return the actual receiver
        rx
    }
}

/// High-level event emitter for SDK
pub struct SdkEventEmitter {
    emitter: Arc<siput_core::GlobalEventEmitter>,
    client_id: String,
}

impl SdkEventEmitter {
    pub fn new(client_id: String) -> Self {
        SdkEventEmitter {
            emitter: siput_core::GlobalEventEmitter::instance(),
            client_id,
        }
    }

    /// Emit custom application event
    pub async fn emit_app_event(&self, event_name: String, data: serde_json::Value) {
        self.emitter.emit_custom(event_name, data, self.client_id.clone()).await;
    }

    /// Emit user action event
    pub async fn emit_user_action(&self, action: String, user_id: String, metadata: serde_json::Value) {
        let data = serde_json::json!({
            "action": action,
            "user_id": user_id,
            "metadata": metadata,
            "timestamp": chrono::Utc::now().timestamp()
        });
        self.emitter.emit_custom("user_action".to_string(), data, self.client_id.clone()).await;
    }
}

/// Example usage in a wallet application
pub mod wallet_example {
    use super::*;

    pub async fn setup_wallet_events() -> Result<(), Box<dyn std::error::Error>> {
        let listener = SdkEventListener::new("wallet_app".to_string());

        // Listen for transaction confirmations
        listener.on_transaction_confirmations(|tx, block_hash| {
            println!("Transaction confirmed: {} in block {}",
                    hex::encode(tx.hash()),
                    hex::encode(block_hash));
            // Update wallet balance
            // Refresh transaction history
        }).await?;

        // Listen for new blocks for balance updates
        listener.on_new_blocks(|block, height| {
            println!("New block: {} at height {}", hex::encode(block.hash()), height);
            // Check for relevant transactions
            // Update balance if needed
        }).await?;

        Ok(())
    }
}

/// Example usage in a dApp
pub mod dapp_example {
    use super::*;

    pub async fn setup_dapp_events() -> Result<(), Box<dyn std::error::Error>> {
        let listener = SdkEventListener::new("my_dapp".to_string());
        let emitter = SdkEventEmitter::new("my_dapp".to_string());

        // Listen for contract executions
        listener.on_contract_executions(|contract_addr, method, result, gas_used| {
            println!("Contract {} executed method {} with gas {}", 
                    hex::encode(contract_addr), method, gas_used);
            // Update dApp state based on contract execution
        }).await?;

        // Listen for custom dApp events
        listener.on_custom_events("game_score_update".to_string(), |data| {
            println!("Game score updated: {:?}", data);
            // Update game UI
        }).await?;

        // Emit user interactions
        emitter.emit_user_action(
            "button_click".to_string(),
            "user123".to_string(),
            serde_json::json!({"button": "submit", "page": "home"})
        ).await;

        Ok(())
    }
}
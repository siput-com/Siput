use crate::events::event_types::{Event, EventType};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use once_cell::sync::Lazy;

/// Global event emitter singleton
pub static GLOBAL_EVENT_EMITTER: Lazy<Arc<GlobalEventEmitter>> =
    Lazy::new(|| Arc::new(GlobalEventEmitter::new()));

/// Global event emitter untuk mengirim event ke seluruh sistem
pub struct GlobalEventEmitter {
    event_bus: Arc<EventBus>,
}

impl GlobalEventEmitter {
    /// Create new global event emitter
    pub fn new() -> Self {
        GlobalEventEmitter {
            event_bus: Arc::new(EventBus::new()),
        }
    }

    /// Get the global instance
    pub fn instance() -> Arc<Self> {
        GLOBAL_EVENT_EMITTER.clone()
    }

    /// Emit block created event
    pub async fn emit_block_created(&self, block: crate::core::Block, block_height: u64, source_node: String) {
        let event = Event::new_block(block, block_height, source_node);
        self.event_bus.publish(event).await;
    }

    /// Emit transaction confirmed event
    pub async fn emit_transaction_confirmed(&self, transaction: crate::core::Transaction, block_hash: crate::core::BlockHash, source_node: String) {
        let event = Event::new_transaction_confirmed(transaction, block_hash, source_node);
        self.event_bus.publish(event).await;
    }

    /// Emit contract executed event
    pub async fn emit_contract_executed(&self, contract_address: crate::core::Address, method: String, result: Vec<u8>, gas_used: u64, source_node: String) {
        let event = Event::new_contract_executed(contract_address, method, result, gas_used, source_node);
        self.event_bus.publish(event).await;
    }

    /// Emit custom event
    pub async fn emit_custom(&self, event_name: String, data: serde_json::Value, source_node: String) {
        let event = Event::new_custom(event_name, data, source_node);
        self.event_bus.publish(event).await;
    }

    /// Get event bus for advanced usage
    pub fn event_bus(&self) -> Arc<EventBus> {
        self.event_bus.clone()
    }
}

/// Internal event bus implementation
pub struct EventBus {
    channels: Arc<RwLock<HashMap<String, broadcast::Sender<Event>>>>,
    capacity: usize,
}

impl EventBus {
    pub fn new() -> Self {
        EventBus {
            channels: Arc::new(RwLock::new(HashMap::new())),
            capacity: 10000, // Larger capacity for global events
        }
    }

    async fn get_or_create_channel(&self, event_type: &str) -> broadcast::Sender<Event> {
        let mut channels = self.channels.write().await;

        if let Some(sender) = channels.get(event_type) {
            sender.clone()
        } else {
            let (sender, _) = broadcast::channel(self.capacity);
            channels.insert(event_type.to_string(), sender.clone());
            sender
        }
    }

    pub async fn publish(&self, event: Event) {
        let event_type = event.event_type_string();

        if let Ok(_) = self.get_or_create_channel(&event_type).await.send(event.clone()) {
            tracing::debug!("Global event published: {}", event_type);
        } else {
            tracing::warn!("Failed to publish global event: {} (no subscribers or channel full)", event_type);
        }
    }

    pub async fn subscribe(&self, event_type: &str) -> broadcast::Receiver<Event> {
        let sender = self.get_or_create_channel(event_type).await;
        sender.subscribe()
    }
}
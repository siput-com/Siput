use crate::events::global_emitter::GlobalEventEmitter;
use crate::events::event_types::Event;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Global event listener untuk mendengarkan event dari seluruh sistem
pub struct GlobalEventListener {
    emitter: Arc<GlobalEventEmitter>,
    node_id: String,
}

impl GlobalEventListener {
    /// Create new global event listener
    pub fn new(node_id: String) -> Self {
        GlobalEventListener {
            emitter: GlobalEventEmitter::instance(),
            node_id,
        }
    }

    /// Listen for block created events
    pub async fn on_block_created<F, Fut>(&self, callback: F)
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut receiver = self.emitter.event_bus().subscribe("block_created").await;
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                callback(event).await;
            }
        });
    }

    /// Listen for transaction confirmed events
    pub async fn on_transaction_confirmed<F, Fut>(&self, callback: F)
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut receiver = self.emitter.event_bus().subscribe("transaction_confirmed").await;
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                callback(event).await;
            }
        });
    }

    /// Listen for contract executed events
    pub async fn on_contract_executed<F, Fut>(&self, callback: F)
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut receiver = self.emitter.event_bus().subscribe("contract_executed").await;
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                callback(event).await;
            }
        });
    }

    /// Listen for custom events by name
    pub async fn on_custom_event<F, Fut>(&self, event_name: String, callback: F)
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut receiver = self.emitter.event_bus().subscribe(&event_name).await;
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                callback(event).await;
            }
        });
    }

    /// Listen for all events
    pub async fn on_all_events<F, Fut>(&self, callback: F)
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        // Subscribe to all known event types
        let event_types = vec![
            "new_block",
            "new_transaction",
            "contract_event",
            "peer_connected",
            "peer_disconnected",
            "node_status",
            "block_created",
            "transaction_confirmed",
            "contract_executed",
        ];

        for event_type in event_types {
            let mut receiver = self.emitter.event_bus().subscribe(event_type).await;
            let callback_clone = callback.clone();
            tokio::spawn(async move {
                while let Ok(event) = receiver.recv().await {
                    callback_clone(event).await;
                }
            });
        }
    }

    /// Get raw receiver for custom event handling
    pub async fn subscribe(&self, event_type: &str) -> broadcast::Receiver<Event> {
        self.emitter.event_bus().subscribe(event_type).await
    }
}

/// Convenience macro for creating event listeners
#[macro_export]
macro_rules! event_listener {
    ($node_id:expr) => {
        crate::events::GlobalEventListener::new($node_id.to_string())
    };
}

/// Convenience macro for emitting events
#[macro_export]
macro_rules! emit_event {
    (block_created, $block:expr, $height:expr) => {
        crate::events::global_emitter::GlobalEventEmitter::instance()
            .emit_block_created($block, $height, "node".to_string()).await
    };
    (transaction_confirmed, $tx:expr, $block_hash:expr) => {
        crate::events::global_emitter::GlobalEventEmitter::instance()
            .emit_transaction_confirmed($tx, $block_hash, "node".to_string()).await
    };
    (contract_executed, $addr:expr, $method:expr, $result:expr, $gas:expr) => {
        crate::events::global_emitter::GlobalEventEmitter::instance()
            .emit_contract_executed($addr, $method, $result, $gas, "node".to_string()).await
    };
    (custom, $name:expr, $data:expr) => {
        crate::events::global_emitter::GlobalEventEmitter::instance()
            .emit_custom($name, $data, "node".to_string()).await
    };
}
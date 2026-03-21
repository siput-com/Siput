# Global Event System Usage Examples

Event system global Siput memungkinkan komunikasi real-time antar komponen sistem.

## SDK Usage

```rust
use siput_sdk::{Client, GlobalEventListener, emit_event};

// Create event listener
let listener = GlobalEventListener::new("sdk_client".to_string());

// Listen for block creation
listener.on_block_created(|event| async move {
    println!("New block created: {:?}", event);
    // Update UI or trigger actions
}).await;

// Listen for transaction confirmations
listener.on_transaction_confirmed(|event| async move {
    println!("Transaction confirmed: {:?}", event);
    // Update transaction status in wallet
}).await;

// Listen for contract executions
listener.on_contract_executed(|event| async move {
    println!("Contract executed: {:?}", event);
    // Update dApp state
}).await;
```

## RPC Server Usage

```rust
use siput_core::{GlobalEventEmitter, GlobalEventListener};
use siput_rpc::WebSocketServer;

// In RPC server initialization
pub async fn start_rpc_server() -> Result<(), Box<dyn std::error::Error>> {
    let emitter = GlobalEventEmitter::instance();
    let listener = GlobalEventListener::new("rpc_server".to_string());

    // Forward blockchain events to WebSocket clients
    listener.on_block_created(|event| async move {
        // Broadcast to all WebSocket clients
        websocket_broadcast("block_created", &event).await;
    }).await;

    listener.on_transaction_confirmed(|event| async move {
        // Broadcast to subscribed clients
        websocket_broadcast("transaction_confirmed", &event).await;
    }).await;

    // Start WebSocket server
    let server = WebSocketServer::new("127.0.0.1:8080".parse()?);
    server.start().await?;

    Ok(())
}
```

## Plugin System Usage

```rust
use siput_core::{GlobalEventEmitter, GlobalEventListener};
use siput_plugins::{Plugin, PluginContext};

pub struct AnalyticsPlugin {
    listener: GlobalEventListener,
}

impl AnalyticsPlugin {
    pub fn new() -> Self {
        AnalyticsPlugin {
            listener: GlobalEventListener::new("analytics_plugin".to_string()),
        }
    }
}

#[async_trait::async_trait]
impl Plugin for AnalyticsPlugin {
    async fn initialize(&mut self, context: &PluginContext) -> Result<(), String> {
        // Listen for all events for analytics
        self.listener.on_all_events(|event| async move {
            // Collect metrics
            collect_event_metrics(&event).await;

            // Store in analytics database
            store_event_data(&event).await;
        }).await;

        Ok(())
    }

    async fn start(&mut self) -> Result<(), String> {
        println!("Analytics plugin started - collecting events");
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), String> {
        println!("Analytics plugin stopped");
        Ok(())
    }
}
```

## Node Runtime Integration

```rust
use siput_core::{GlobalEventEmitter, NodeRuntime};

// In node runtime
impl NodeRuntime {
    pub async fn emit_block_created(&self, block: Block, height: u64) {
        let emitter = GlobalEventEmitter::instance();
        emitter.emit_block_created(block, height, self.node_id.to_string()).await;
    }

    pub async fn emit_transaction_confirmed(&self, tx: Transaction, block_hash: BlockHash) {
        let emitter = GlobalEventEmitter::instance();
        emitter.emit_transaction_confirmed(tx, block_hash, self.node_id.to_string()).await;
    }
}
```

## Custom Events

```rust
use siput_core::GlobalEventEmitter;
use serde_json::json;

// Emit custom event
let emitter = GlobalEventEmitter::instance();
emitter.emit_custom(
    "custom_metric".to_string(),
    json!({
        "tps": 1500,
        "latency_ms": 45,
        "active_connections": 1200
    }),
    "metrics_collector".to_string()
).await;

// Listen for custom events
let listener = GlobalEventListener::new("dashboard".to_string());
listener.on_custom_event("custom_metric".to_string(), |event| async move {
    if let EventType::Custom { data, .. } = &event.event_type {
        println!("Custom metric: {:?}", data);
    }
}).await;
```

## Macro Usage

```rust
use siput_core::{event_listener, emit_event};

// Using convenience macros
let listener = event_listener!("my_app");

// Emit events using macros
emit_event!(block_created, block, block_height);
emit_event!(transaction_confirmed, transaction, block_hash);
emit_event!(contract_executed, contract_addr, "transfer".to_string(), result, gas_used);
emit_event!(custom, "user_action".to_string(), json!({"action": "login", "user_id": 123}));
```

## Error Handling

```rust
use siput_core::GlobalEventListener;

let listener = GlobalEventListener::new("error_handler".to_string());

// Handle events with error recovery
listener.on_block_created(|event| async move {
    match process_block_event(&event) {
        Ok(_) => println!("Block processed successfully"),
        Err(e) => {
            eprintln!("Failed to process block: {}", e);
            // Retry logic or alert monitoring
        }
    }
}).await;
```

## Performance Considerations

- Event channels have 10,000 message capacity
- Events are broadcast to all subscribers
- Use selective listening to avoid performance issues
- Consider event filtering for high-frequency events
- Monitor subscriber count to prevent memory leaks
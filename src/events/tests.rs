#[cfg(test)]
mod tests {
    use super::*;
    use siput_core::{GlobalEventEmitter, GlobalEventListener, EventType};
    use std::sync::Arc;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_global_event_emitter_singleton() {
        let emitter1 = GlobalEventEmitter::instance();
        let emitter2 = GlobalEventEmitter::instance();

        // Should be the same instance
        assert_eq!(Arc::as_ptr(&emitter1), Arc::as_ptr(&emitter2));
    }

    #[tokio::test]
    async fn test_block_created_event() {
        let emitter = GlobalEventEmitter::instance();
        let listener = GlobalEventListener::new("test_listener".to_string());

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        // Listen for block created events
        listener.on_block_created(move |event| {
            let tx = tx.clone();
            async move {
                let _ = tx.send(event).await;
            }
        }).await;

        // Create a test block
        let block = siput_core::Block {
            producer: [0u8; 20],
            timestamp: 1234567890,
            transactions: vec![],
            prev_block_hash: [0u8; 32],
            nonce: 0,
            reward: 1000,
            difficulty: 1,
            hash: [0u8; 32],
        };

        // Emit block created event
        emitter.emit_block_created(block.clone(), 42, "test_node".to_string()).await;

        // Wait for event with timeout
        let received_event = timeout(Duration::from_secs(1), rx.recv()).await
            .expect("Event not received within timeout")
            .expect("Channel closed");

        // Verify event
        match &received_event.event_type {
            EventType::BlockCreated { block: received_block, block_height } => {
                assert_eq!(received_block.timestamp, block.timestamp);
                assert_eq!(*block_height, 42);
            }
            _ => panic!("Wrong event type received"),
        }

        assert_eq!(received_event.source_node, "test_node");
    }

    #[tokio::test]
    async fn test_transaction_confirmed_event() {
        let emitter = GlobalEventEmitter::instance();
        let listener = GlobalEventListener::new("test_listener".to_string());

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        // Listen for transaction confirmed events
        listener.on_transaction_confirmed(move |event| {
            let tx = tx.clone();
            async move {
                let _ = tx.send(event).await;
            }
        }).await;

        // Create a test transaction
        let transaction = siput_core::Transaction::new_transfer(
            [1u8; 20], // from
            [2u8; 20], // to
            1000,      // amount
            0,         // nonce
            21000,     // gas_limit
            1,         // gas_price
        );

        let block_hash = [3u8; 32];

        // Emit transaction confirmed event
        emitter.emit_transaction_confirmed(
            transaction.clone(),
            block_hash,
            "test_node".to_string()
        ).await;

        // Wait for event
        let received_event = timeout(Duration::from_secs(1), rx.recv()).await
            .expect("Event not received within timeout")
            .expect("Channel closed");

        // Verify event
        match &received_event.event_type {
            EventType::TransactionConfirmed { transaction: received_tx, block_hash: received_hash, .. } => {
                assert_eq!(received_tx.amount, transaction.amount);
                assert_eq!(*received_hash, block_hash);
            }
            _ => panic!("Wrong event type received"),
        }
    }

    #[tokio::test]
    async fn test_contract_executed_event() {
        let emitter = GlobalEventEmitter::instance();
        let listener = GlobalEventListener::new("test_listener".to_string());

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        // Listen for contract executed events
        listener.on_contract_executed(move |event| {
            let tx = tx.clone();
            async move {
                let _ = tx.send(event).await;
            }
        }).await;

        let contract_address = [1u8; 20];
        let method = "transfer".to_string();
        let result = vec![0xAA, 0xBB, 0xCC];
        let gas_used = 50000;

        // Emit contract executed event
        emitter.emit_contract_executed(
            contract_address,
            method.clone(),
            result.clone(),
            gas_used,
            "test_node".to_string()
        ).await;

        // Wait for event
        let received_event = timeout(Duration::from_secs(1), rx.recv()).await
            .expect("Event not received within timeout")
            .expect("Channel closed");

        // Verify event
        match &received_event.event_type {
            EventType::ContractExecuted {
                contract_address: addr,
                method: m,
                result: r,
                gas_used: g
            } => {
                assert_eq!(*addr, contract_address);
                assert_eq!(m, &method);
                assert_eq!(r, &result);
                assert_eq!(*g, gas_used);
            }
            _ => panic!("Wrong event type received"),
        }
    }

    #[tokio::test]
    async fn test_custom_event() {
        let emitter = GlobalEventEmitter::instance();
        let listener = GlobalEventListener::new("test_listener".to_string());

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        let event_name = "test_custom_event".to_string();

        // Listen for custom events
        listener.on_custom_event(event_name.clone(), move |event| {
            let tx = tx.clone();
            async move {
                let _ = tx.send(event).await;
            }
        }).await;

        let custom_data = serde_json::json!({
            "key": "value",
            "number": 42,
            "array": [1, 2, 3]
        });

        // Emit custom event
        emitter.emit_custom(
            event_name.clone(),
            custom_data.clone(),
            "test_node".to_string()
        ).await;

        // Wait for event
        let received_event = timeout(Duration::from_secs(1), rx.recv()).await
            .expect("Event not received within timeout")
            .expect("Channel closed");

        // Verify event
        match &received_event.event_type {
            EventType::Custom { event_name: name, data } => {
                assert_eq!(name, &event_name);
                assert_eq!(data, &custom_data);
            }
            _ => panic!("Wrong event type received"),
        }
    }

    #[tokio::test]
    async fn test_multiple_listeners() {
        let emitter = GlobalEventEmitter::instance();

        let listener1 = GlobalEventListener::new("listener1".to_string());
        let listener2 = GlobalEventListener::new("listener2".to_string());

        let (tx1, mut rx1) = tokio::sync::mpsc::channel(1);
        let (tx2, mut rx2) = tokio::sync::mpsc::channel(1);

        // Both listeners subscribe to the same event
        listener1.on_block_created(move |event| {
            let tx = tx1.clone();
            async move {
                let _ = tx.send(("listener1", event)).await;
            }
        }).await;

        listener2.on_block_created(move |event| {
            let tx = tx2.clone();
            async move {
                let _ = tx.send(("listener2", event)).await;
            }
        }).await;

        // Emit event
        let block = siput_core::Block {
            producer: [0u8; 20],
            timestamp: 1234567890,
            transactions: vec![],
            prev_block_hash: [0u8; 32],
            nonce: 0,
            reward: 1000,
            difficulty: 1,
            hash: [0u8; 32],
        };

        emitter.emit_block_created(block, 1, "test_node".to_string()).await;

        // Both listeners should receive the event
        let result1 = timeout(Duration::from_secs(1), rx1.recv()).await
            .expect("Listener1 didn't receive event")
            .expect("Channel1 closed");

        let result2 = timeout(Duration::from_secs(1), rx2.recv()).await
            .expect("Listener2 didn't receive event")
            .expect("Channel2 closed");

        assert_eq!(result1.0, "listener1");
        assert_eq!(result2.0, "listener2");
    }
}
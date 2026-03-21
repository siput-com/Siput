//! Example plugin that demonstrates event system integration
//!
//! This plugin listens to blockchain events and provides analytics
//! and monitoring capabilities.

use siput_core::{GlobalEventEmitter, GlobalEventListener, Event, EventType};
use siput_plugins::{Plugin, PluginContext};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Analytics plugin that collects blockchain metrics
pub struct AnalyticsPlugin {
    listener: GlobalEventListener,
    emitter: Arc<GlobalEventEmitter>,
    metrics: Arc<RwLock<HashMap<String, u64>>>,
}

impl AnalyticsPlugin {
    pub fn new() -> Self {
        AnalyticsPlugin {
            listener: GlobalEventListener::new("analytics_plugin".to_string()),
            emitter: GlobalEventEmitter::instance(),
            metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn increment_metric(&self, metric: &str) {
        let mut metrics = self.metrics.write().await;
        *metrics.entry(metric.to_string()).or_insert(0) += 1;
    }

    async fn get_metric(&self, metric: &str) -> u64 {
        let metrics = self.metrics.read().await;
        *metrics.get(metric).unwrap_or(&0)
    }
}

#[async_trait::async_trait]
impl Plugin for AnalyticsPlugin {
    async fn initialize(&mut self, _context: &PluginContext) -> Result<(), String> {
        // Listen for block creation events
        let metrics_clone = self.metrics.clone();
        self.listener.on_block_created(move |event| {
            let metrics = metrics_clone.clone();
            async move {
                if let EventType::BlockCreated { block, block_height } = &event.event_type {
                    let mut m = metrics.write().await;
                    *m.entry("blocks_created".to_string()).or_insert(0) += 1;
                    *m.entry("total_transactions".to_string()).or_insert(0) += block.transactions.len() as u64;

                    println!("📊 Block {} created with {} transactions at height {}",
                            hex::encode(block.hash()),
                            block.transactions.len(),
                            block_height);
                }
            }
        }).await;

        // Listen for transaction confirmations
        let metrics_clone = self.metrics.clone();
        self.listener.on_transaction_confirmed(move |event| {
            let metrics = metrics_clone.clone();
            async move {
                if let EventType::TransactionConfirmed { transaction, .. } = &event.event_type {
                    let mut m = metrics.write().await;
                    *m.entry("transactions_confirmed".to_string()).or_insert(0) += 1;
                    *m.entry("total_gas_used".to_string()).or_insert(0) += transaction.gas_limit;

                    println!("✅ Transaction {} confirmed",
                            hex::encode(transaction.hash()));
                }
            }
        }).await;

        // Listen for contract executions
        let metrics_clone = self.metrics.clone();
        self.listener.on_contract_executed(move |event| {
            let metrics = metrics_clone.clone();
            async move {
                if let EventType::ContractExecuted { gas_used, .. } = &event.event_type {
                    let mut m = metrics.write().await;
                    *m.entry("contracts_executed".to_string()).or_insert(0) += 1;
                    *m.entry("contract_gas_used".to_string()).or_insert(0) += gas_used;

                    println!("⚙️ Contract executed with {} gas", gas_used);
                }
            }
        }).await;

        // Listen for custom events
        let emitter_clone = self.emitter.clone();
        self.listener.on_custom_event("plugin_health_check".to_string(), move |event| {
            let emitter = emitter_clone.clone();
            async move {
                // Respond to health check
                emitter.emit_custom(
                    "plugin_health_response".to_string(),
                    serde_json::json!({
                        "plugin": "analytics",
                        "status": "healthy",
                        "timestamp": chrono::Utc::now().timestamp()
                    }),
                    "analytics_plugin".to_string()
                ).await;
            }
        }).await;

        Ok(())
    }

    async fn start(&mut self) -> Result<(), String> {
        println!("🚀 Analytics plugin started - collecting blockchain metrics");

        // Emit startup event
        self.emitter.emit_custom(
            "plugin_started".to_string(),
            serde_json::json!({
                "plugin": "analytics",
                "capabilities": ["block_tracking", "transaction_monitoring", "contract_analytics"]
            }),
            "analytics_plugin".to_string()
        ).await;

        // Start periodic metrics reporting
        let metrics_clone = self.metrics.clone();
        let emitter_clone = self.emitter.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;

                let metrics = metrics_clone.read().await;
                let report = serde_json::json!({
                    "blocks_created": metrics.get("blocks_created").unwrap_or(&0),
                    "transactions_confirmed": metrics.get("transactions_confirmed").unwrap_or(&0),
                    "contracts_executed": metrics.get("contracts_executed").unwrap_or(&0),
                    "total_gas_used": metrics.get("total_gas_used").unwrap_or(&0),
                    "timestamp": chrono::Utc::now().timestamp()
                });

                emitter_clone.emit_custom(
                    "analytics_report".to_string(),
                    report,
                    "analytics_plugin".to_string()
                ).await;
            }
        });

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), String> {
        println!("🛑 Analytics plugin stopped");

        // Emit final metrics
        let metrics = self.metrics.read().await;
        let final_report = serde_json::json!({
            "final_metrics": serde_json::Value::Object(
                metrics.iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::Number((*v).into())))
                    .collect()
            ),
            "shutdown_time": chrono::Utc::now().timestamp()
        });

        self.emitter.emit_custom(
            "plugin_stopped".to_string(),
            final_report,
            "analytics_plugin".to_string()
        ).await;

        Ok(())
    }

    async fn get_status(&self) -> Result<serde_json::Value, String> {
        let metrics = self.metrics.read().await;
        Ok(serde_json::json!({
            "status": "active",
            "metrics_collected": metrics.len(),
            "blocks_tracked": metrics.get("blocks_created").unwrap_or(&0),
            "transactions_tracked": metrics.get("transactions_confirmed").unwrap_or(&0)
        }))
    }
}

/// Monitoring plugin that alerts on unusual activity
pub struct MonitoringPlugin {
    listener: GlobalEventListener,
    emitter: Arc<GlobalEventEmitter>,
    alert_thresholds: HashMap<String, u64>,
}

impl MonitoringPlugin {
    pub fn new() -> Self {
        let mut thresholds = HashMap::new();
        thresholds.insert("high_gas_usage".to_string(), 10_000_000);
        thresholds.insert("large_transaction".to_string(), 1_000_000_000); // 1000 SIPUT

        MonitoringPlugin {
            listener: GlobalEventListener::new("monitoring_plugin".to_string()),
            emitter: GlobalEventEmitter::instance(),
            alert_thresholds: thresholds,
        }
    }
}

#[async_trait::async_trait]
impl Plugin for MonitoringPlugin {
    async fn initialize(&mut self, _context: &PluginContext) -> Result<(), String> {
        // Monitor high-value transactions
        let emitter_clone = self.emitter.clone();
        let thresholds = self.alert_thresholds.clone();
        self.listener.on_transaction_confirmed(move |event| {
            let emitter = emitter_clone.clone();
            let thresholds = thresholds.clone();
            async move {
                if let EventType::TransactionConfirmed { transaction, .. } = &event.event_type {
                    if transaction.amount > *thresholds.get("large_transaction").unwrap_or(&0) {
                        emitter.emit_custom(
                            "security_alert".to_string(),
                            serde_json::json!({
                                "alert_type": "large_transaction",
                                "transaction_hash": hex::encode(transaction.hash()),
                                "amount": transaction.amount,
                                "from": hex::encode(transaction.from),
                                "to": hex::encode(transaction.to),
                                "timestamp": event.timestamp
                            }),
                            "monitoring_plugin".to_string()
                        ).await;
                    }
                }
            }
        }).await;

        // Monitor contract executions with high gas usage
        let emitter_clone = self.emitter.clone();
        let thresholds = self.alert_thresholds.clone();
        self.listener.on_contract_executed(move |event| {
            let emitter = emitter_clone.clone();
            let thresholds = thresholds.clone();
            async move {
                if let EventType::ContractExecuted { gas_used, contract_address, method, .. } = &event.event_type {
                    if *gas_used > *thresholds.get("high_gas_usage").unwrap_or(&0) {
                        emitter.emit_custom(
                            "performance_alert".to_string(),
                            serde_json::json!({
                                "alert_type": "high_gas_usage",
                                "contract": hex::encode(*contract_address),
                                "method": method,
                                "gas_used": gas_used,
                                "timestamp": event.timestamp
                            }),
                            "monitoring_plugin".to_string()
                        ).await;
                    }
                }
            }
        }).await;

        Ok(())
    }

    async fn start(&mut self) -> Result<(), String> {
        println!("👁️ Monitoring plugin started - watching for anomalies");
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), String> {
        println!("🙈 Monitoring plugin stopped");
        Ok(())
    }
}
//! Metrics Collection
//!
//! Provides comprehensive metrics collection including:
//! - TPS (Transactions Per Second)
//! - Latency measurements
//! - Counter metrics
//! - Gauge metrics for system state
//! - Prometheus-compatible output

use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec, register_int_counter_vec,
    CounterVec, Encoder, GaugeVec, HistogramVec, IntCounterVec, TextEncoder,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

lazy_static! {
    // Transaction metrics
    static ref TX_PROCESSED_TOTAL: IntCounterVec = register_int_counter_vec!(
        "siput_transactions_processed_total",
        "Total number of transactions processed",
        &["type", "status"]
    ).unwrap();

    static ref TX_PROCESSING_DURATION: HistogramVec = register_histogram_vec!(
        "siput_transaction_processing_duration_seconds",
        "Transaction processing duration in seconds",
        &["type"],
        vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0]
    ).unwrap();

    // TPS metrics
    static ref TPS_CURRENT: GaugeVec = register_gauge_vec!(
        "siput_tps_current",
        "Current transactions per second",
        &["component"]
    ).unwrap();

    static ref TPS_PEAK: GaugeVec = register_gauge_vec!(
        "siput_tps_peak",
        "Peak transactions per second in the last hour",
        &["component"]
    ).unwrap();

    // Block metrics
    static ref BLOCKS_MINED_TOTAL: IntCounterVec = register_int_counter_vec!(
        "siput_blocks_mined_total",
        "Total number of blocks mined",
        &["miner"]
    ).unwrap();

    static ref BLOCK_MINING_DURATION: HistogramVec = register_histogram_vec!(
        "siput_block_mining_duration_seconds",
        "Block mining duration in seconds",
        &["algorithm"]
    ).unwrap();

    // Network metrics
    static ref NETWORK_CONNECTIONS_ACTIVE: GaugeVec = register_gauge_vec!(
        "siput_network_connections_active",
        "Number of active network connections",
        &["direction"]
    ).unwrap();

    static ref NETWORK_MESSAGES_TOTAL: IntCounterVec = register_int_counter_vec!(
        "siput_network_messages_total",
        "Total number of network messages",
        &["type", "direction"]
    ).unwrap();

    static ref NETWORK_LATENCY: HistogramVec = register_histogram_vec!(
        "siput_network_latency_seconds",
        "Network message latency in seconds",
        &["peer_type"],
        vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
    ).unwrap();

    // RPC metrics
    static ref RPC_REQUESTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "siput_rpc_requests_total",
        "Total number of RPC requests",
        &["method", "status"]
    ).unwrap();

    static ref RPC_REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "siput_rpc_request_duration_seconds",
        "RPC request duration in seconds",
        &["method"],
        vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0]
    ).unwrap();

    // Consensus metrics
    static ref CONSENSUS_ROUNDS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "siput_consensus_rounds_total",
        "Total number of consensus rounds",
        &["outcome"]
    ).unwrap();

    static ref CONSENSUS_LATENCY: HistogramVec = register_histogram_vec!(
        "siput_consensus_latency_seconds",
        "Consensus round latency in seconds",
        &["algorithm"],
        vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]
    ).unwrap();

    // System metrics
    static ref MEMORY_USAGE_BYTES: GaugeVec = register_gauge_vec!(
        "siput_memory_usage_bytes",
        "Memory usage in bytes",
        &["component"]
    ).unwrap();

    static ref CPU_USAGE_PERCENT: GaugeVec = register_gauge_vec!(
        "siput_cpu_usage_percent",
        "CPU usage percentage",
        &["component"]
    ).unwrap();

    // Error metrics
    static ref ERRORS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "siput_errors_total",
        "Total number of errors",
        &["component", "error_type"]
    ).unwrap();
}

/// TPS Tracker for real-time TPS calculation
#[derive(Debug)]
pub struct TpsTracker {
    component: String,
    window_start: Instant,
    transaction_count: u64,
    peak_tps: f64,
    last_update: Instant,
}

impl TpsTracker {
    pub fn new(component: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            window_start: Instant::now(),
            transaction_count: 0,
            peak_tps: 0.0,
            last_update: Instant::now(),
        }
    }

    pub fn record_transaction(&mut self) {
        self.transaction_count += 1;
        self.update_metrics();
    }

    pub fn record_transactions(&mut self, count: u64) {
        self.transaction_count += count;
        self.update_metrics();
    }

    fn update_metrics(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.window_start).as_secs_f64();

        if elapsed >= 1.0 {
            let current_tps = self.transaction_count as f64 / elapsed;

            // Update current TPS
            TPS_CURRENT
                .with_label_values(&[&self.component])
                .set(current_tps);

            // Update peak TPS
            if current_tps > self.peak_tps {
                self.peak_tps = current_tps;
                TPS_PEAK
                    .with_label_values(&[&self.component])
                    .set(self.peak_tps);
            }

            // Reset for next window
            self.window_start = now;
            self.transaction_count = 0;
        }
    }
}

/// Global metrics registry
lazy_static! {
    static ref METRICS_REGISTRY: prometheus::Registry = prometheus::Registry::new();
    static ref TPS_TRACKERS: Arc<RwLock<HashMap<String, TpsTracker>>> = Arc::new(RwLock::new(HashMap::new()));
}

/// Initialize metrics collection
pub async fn init_metrics(service_name: &str, version: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Register custom metrics with registry
    METRICS_REGISTRY.register(Box::new(TX_PROCESSED_TOTAL.clone()))?;
    METRICS_REGISTRY.register(Box::new(TX_PROCESSING_DURATION.clone()))?;
    METRICS_REGISTRY.register(Box::new(TPS_CURRENT.clone()))?;
    METRICS_REGISTRY.register(Box::new(TPS_PEAK.clone()))?;
    METRICS_REGISTRY.register(Box::new(BLOCKS_MINED_TOTAL.clone()))?;
    METRICS_REGISTRY.register(Box::new(BLOCK_MINING_DURATION.clone()))?;
    METRICS_REGISTRY.register(Box::new(NETWORK_CONNECTIONS_ACTIVE.clone()))?;
    METRICS_REGISTRY.register(Box::new(NETWORK_MESSAGES_TOTAL.clone()))?;
    METRICS_REGISTRY.register(Box::new(NETWORK_LATENCY.clone()))?;
    METRICS_REGISTRY.register(Box::new(RPC_REQUESTS_TOTAL.clone()))?;
    METRICS_REGISTRY.register(Box::new(RPC_REQUEST_DURATION.clone()))?;
    METRICS_REGISTRY.register(Box::new(CONSENSUS_ROUNDS_TOTAL.clone()))?;
    METRICS_REGISTRY.register(Box::new(CONSENSUS_LATENCY.clone()))?;
    METRICS_REGISTRY.register(Box::new(MEMORY_USAGE_BYTES.clone()))?;
    METRICS_REGISTRY.register(Box::new(CPU_USAGE_PERCENT.clone()))?;
    METRICS_REGISTRY.register(Box::new(ERRORS_TOTAL.clone()))?;

    // Set service info
    let _service_info = register_gauge_vec!(
        "siput_service_info",
        "Service information",
        &["service_name", "version"]
    )?;
    _service_info
        .with_label_values(&[service_name, version])
        .set(1.0);

    tracing::info!("Metrics collection initialized");
    Ok(())
}

/// Shutdown metrics collection
pub async fn shutdown_metrics() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Metrics collection shut down");
    Ok(())
}

/// Get metrics in Prometheus format
pub fn gather_metrics() -> Result<String, Box<dyn std::error::Error>> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer)?;
    Ok(String::from_utf8(buffer)?)
}

/// Transaction metrics
pub mod transaction {
    use super::*;

    pub fn record_transaction_processed(tx_type: &str, status: &str) {
        TX_PROCESSED_TOTAL
            .with_label_values(&[tx_type, status])
            .inc();
    }

    pub fn observe_processing_duration(tx_type: &str, duration: Duration) {
        TX_PROCESSING_DURATION
            .with_label_values(&[tx_type])
            .observe(duration.as_secs_f64());
    }

    pub fn get_tps_tracker(component: &str) -> TpsTracker {
        let mut trackers = TPS_TRACKERS.try_write().unwrap();
        trackers
            .entry(component.to_string())
            .or_insert_with(|| TpsTracker::new(component))
            .clone()
    }

    pub fn record_tps(component: &str) {
        let mut trackers = TPS_TRACKERS.try_write().unwrap();
        let tracker = trackers
            .entry(component.to_string())
            .or_insert_with(|| TpsTracker::new(component));
        tracker.record_transaction();
    }
}

/// Block metrics
pub mod block {
    use super::*;

    pub fn record_block_mined(miner: &str) {
        BLOCKS_MINED_TOTAL.with_label_values(&[miner]).inc();
    }

    pub fn observe_mining_duration(algorithm: &str, duration: Duration) {
        BLOCK_MINING_DURATION
            .with_label_values(&[algorithm])
            .observe(duration.as_secs_f64());
    }
}

/// Network metrics
pub mod network {
    use super::*;

    pub fn set_active_connections(direction: &str, count: i64) {
        NETWORK_CONNECTIONS_ACTIVE
            .with_label_values(&[direction])
            .set(count as f64);
    }

    pub fn record_message(msg_type: &str, direction: &str) {
        NETWORK_MESSAGES_TOTAL
            .with_label_values(&[msg_type, direction])
            .inc();
    }

    pub fn observe_latency(peer_type: &str, duration: Duration) {
        NETWORK_LATENCY
            .with_label_values(&[peer_type])
            .observe(duration.as_secs_f64());
    }
}

/// RPC metrics
pub mod rpc {
    use super::*;

    pub fn record_request(method: &str, status: &str) {
        RPC_REQUESTS_TOTAL
            .with_label_values(&[method, status])
            .inc();
    }

    pub fn observe_request_duration(method: &str, duration: Duration) {
        RPC_REQUEST_DURATION
            .with_label_values(&[method])
            .observe(duration.as_secs_f64());
    }
}

/// Consensus metrics
pub mod consensus {
    use super::*;

    pub fn record_round(outcome: &str) {
        CONSENSUS_ROUNDS_TOTAL.with_label_values(&[outcome]).inc();
    }

    pub fn observe_latency(algorithm: &str, duration: Duration) {
        CONSENSUS_LATENCY
            .with_label_values(&[algorithm])
            .observe(duration.as_secs_f64());
    }
}

/// System metrics
pub mod system {
    use super::*;

    pub fn set_memory_usage(component: &str, bytes: u64) {
        MEMORY_USAGE_BYTES
            .with_label_values(&[component])
            .set(bytes as f64);
    }

    pub fn set_cpu_usage(component: &str, percent: f64) {
        CPU_USAGE_PERCENT
            .with_label_values(&[component])
            .set(percent);
    }
}

/// Error metrics
pub mod error {
    use super::*;

    pub fn record_error(component: &str, error_type: &str) {
        ERRORS_TOTAL
            .with_label_values(&[component, error_type])
            .inc();
    }
}

/// Generic metrics functions
pub fn increment_counter(name: &str) {
    // This would be a generic counter registry
    // For now, we'll use a simple approach
    tracing::debug!("Counter incremented: {}", name);
}

pub fn observe_duration(operation: &str, duration: Duration) {
    // This would record duration metrics
    // For now, we'll use a simple approach
    tracing::debug!(
        operation = %operation,
        duration_ms = duration.as_millis(),
        "Duration observed"
    );
}

/// Metrics server for Prometheus scraping
pub struct MetricsServer {
    port: u16,
}

impl MetricsServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn start(self) -> Result<(), Box<dyn std::error::Error>> {
        use axum::{routing::get, Router};
        use std::net::SocketAddr;

        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .route("/health", get(health_handler));

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        tracing::info!("Metrics server listening on {}", addr);

        axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;

        Ok(())
    }
}

async fn metrics_handler() -> String {
    match gather_metrics() {
        Ok(metrics) => metrics,
        Err(e) => {
            tracing::error!("Failed to gather metrics: {}", e);
            "# Error gathering metrics\n".to_string()
        }
    }
}

async fn health_handler() -> &'static str {
    "OK"
}
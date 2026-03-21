//! Distributed Tracing
//!
//! Provides distributed tracing capabilities using OpenTelemetry:
//! - Trace spans for operations
//! - Context propagation
//! - Integration with Jaeger/Zipkin
//! - Performance tracing

use opentelemetry::{
    global,
    trace::{Span, SpanBuilder, Tracer, TracerProvider},
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    trace::{self, Sampler},
    Resource,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::layer::SubscriberExt;

/// Initialize distributed tracing
pub async fn init_tracing(
    service_name: &str,
    service_version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Set up OTLP exporter (for Jaeger, Zipkin, etc.)
    let otlp_exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint("http://localhost:4317"); // Default OTLP endpoint

    let tracer_provider = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_batch_exporter(otlp_exporter, opentelemetry_sdk::runtime::Tokio)
        .with_sampler(Sampler::AlwaysOn)
        .with_resource(Resource::new(vec![
            KeyValue::new("service.name", service_name.to_string()),
            KeyValue::new("service.version", service_version.to_string()),
            KeyValue::new("service.instance.id", generate_instance_id()),
        ]))
        .build();

    global::set_tracer_provider(tracer_provider);

    // Create OpenTelemetry layer for tracing
    let tracer = global::tracer(service_name);
    let otel_layer = OpenTelemetryLayer::new(tracer);

    // Add to existing subscriber
    tracing::subscriber::with_default(tracing::subscriber::Registry::default().with(otel_layer), || {});

    tracing::info!("Distributed tracing initialized");
    Ok(())
}

/// Shutdown tracing
pub async fn shutdown_tracing() -> Result<(), Box<dyn std::error::Error>> {
    opentelemetry::global::shutdown_tracer_provider();
    tracing::info!("Distributed tracing shut down");
    Ok(())
}

/// Generate unique instance ID
fn generate_instance_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", timestamp)
}

/// Tracing context for operations
#[derive(Debug, Clone)]
pub struct TracingContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub attributes: HashMap<String, String>,
}

impl TracingContext {
    pub fn new() -> Self {
        Self {
            trace_id: generate_trace_id(),
            span_id: generate_span_id(),
            parent_span_id: None,
            attributes: HashMap::new(),
        }
    }

    pub fn with_parent(parent_trace_id: String, parent_span_id: String) -> Self {
        Self {
            trace_id: parent_trace_id,
            span_id: generate_span_id(),
            parent_span_id: Some(parent_span_id),
            attributes: HashMap::new(),
        }
    }

    pub fn add_attribute(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.attributes.insert(key.into(), value.into());
    }
}

/// Generate a new trace ID
fn generate_trace_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    hex::encode(bytes)
}

/// Generate a new span ID
fn generate_span_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 8] = rng.gen();
    hex::encode(bytes)
}

/// Span manager for creating and managing spans
pub struct SpanManager {
    tracer: opentelemetry::trace::Tracer,
}

impl SpanManager {
    pub fn new(service_name: &str) -> Self {
        let tracer = global::tracer(service_name);
        Self { tracer }
    }

    pub fn create_span(&self, name: &str) -> opentelemetry::trace::Span {
        self.tracer.start(name)
    }

    pub fn create_span_with_context(
        &self,
        name: &str,
        context: &TracingContext,
    ) -> opentelemetry::trace::Span {
        let mut span_builder = SpanBuilder::from_name(name.to_string());

        // Add attributes from context
        for (key, value) in &context.attributes {
            span_builder = span_builder.with_attributes(vec![KeyValue::new(key.clone(), value.clone())]);
        }

        self.tracer.build(span_builder)
    }

    pub fn create_child_span(
        &self,
        name: &str,
        parent_span: &opentelemetry::trace::Span,
    ) -> opentelemetry::trace::Span {
        let mut span_builder = SpanBuilder::from_name(name.to_string());
        span_builder = span_builder.with_parent_context(parent_span.span_context().clone());
        self.tracer.build(span_builder)
    }
}

/// Tracing macros for easy span creation
#[macro_export]
macro_rules! trace_operation {
    ($operation:expr) => {{
        let span = $crate::observability::tracing::create_operation_span(stringify!($operation));
        let _enter = span.enter();
        let result = $operation;
        span.set_status(opentelemetry::trace::Status::Ok);
        result
    }};
}

#[macro_export]
macro_rules! trace_operation_async {
    ($operation:expr) => {{
        async {
            let span = $crate::observability::tracing::create_operation_span(stringify!($operation));
            let _enter = span.enter();
            let result = $operation.await;
            span.set_status(opentelemetry::trace::Status::Ok);
            result
        }
    }};
}

#[macro_export]
macro_rules! trace_blockchain_operation {
    ($operation:expr, $tx_hash:expr) => {{
        let span = $crate::observability::tracing::create_blockchain_span(
            stringify!($operation),
            $tx_hash
        );
        let _enter = span.enter();
        let result = $operation;
        span.set_status(opentelemetry::trace::Status::Ok);
        result
    }};
}

/// Create a span for a generic operation
pub fn create_operation_span(operation_name: &str) -> opentelemetry::trace::Span {
    let tracer = global::tracer("siput");
    let span = tracer.start(operation_name);
    span.set_attribute(KeyValue::new("operation.type", "generic"));
    span
}

/// Create a span for blockchain operations
pub fn create_blockchain_span(operation_name: &str, tx_hash: &str) -> opentelemetry::trace::Span {
    let tracer = global::tracer("siput");
    let span = tracer.start(operation_name);
    span.set_attribute(KeyValue::new("operation.type", "blockchain"));
    span.set_attribute(KeyValue::new("transaction.hash", tx_hash.to_string()));
    span
}

/// Create a span for network operations
pub fn create_network_span(operation_name: &str, peer_id: &str) -> opentelemetry::trace::Span {
    let tracer = global::tracer("siput");
    let span = tracer.start(operation_name);
    span.set_attribute(KeyValue::new("operation.type", "network"));
    span.set_attribute(KeyValue::new("peer.id", peer_id.to_string()));
    span
}

/// Create a span for RPC operations
pub fn create_rpc_span(method: &str, params: Option<&str>) -> opentelemetry::trace::Span {
    let tracer = global::tracer("siput");
    let span = tracer.start(format!("rpc.{}", method));
    span.set_attribute(KeyValue::new("operation.type", "rpc"));
    span.set_attribute(KeyValue::new("rpc.method", method.to_string()));
    if let Some(p) = params {
        span.set_attribute(KeyValue::new("rpc.params", p.to_string()));
    }
    span
}

/// Create a span for consensus operations
pub fn create_consensus_span(operation_name: &str, round: u64) -> opentelemetry::trace::Span {
    let tracer = global::tracer("siput");
    let span = tracer.start(operation_name);
    span.set_attribute(KeyValue::new("operation.type", "consensus"));
    span.set_attribute(KeyValue::new("consensus.round", round.to_string()));
    span
}

/// Tracing context manager
pub struct TracingContextManager {
    contexts: Arc<std::sync::Mutex<HashMap<String, TracingContext>>>,
}

impl TracingContextManager {
    pub fn new() -> Self {
        Self {
            contexts: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn create_context(&self, operation_id: &str) -> TracingContext {
        let context = TracingContext::new();
        let mut contexts = self.contexts.lock().unwrap();
        contexts.insert(operation_id.to_string(), context.clone());
        context
    }

    pub fn get_context(&self, operation_id: &str) -> Option<TracingContext> {
        let contexts = self.contexts.lock().unwrap();
        contexts.get(operation_id).cloned()
    }

    pub fn remove_context(&self, operation_id: &str) {
        let mut contexts = self.contexts.lock().unwrap();
        contexts.remove(operation_id);
    }

    pub fn create_child_context(&self, operation_id: &str, parent_operation_id: &str) -> Option<TracingContext> {
        let contexts = self.contexts.lock().unwrap();
        if let Some(parent) = contexts.get(parent_operation_id) {
            let child = TracingContext::with_parent(
                parent.trace_id.clone(),
                parent.span_id.clone(),
            );
            drop(contexts);
            let mut contexts = self.contexts.lock().unwrap();
            contexts.insert(operation_id.to_string(), child.clone());
            Some(child)
        } else {
            None
        }
    }
}

/// Global context manager
lazy_static::lazy_static! {
    static ref GLOBAL_CONTEXT_MANAGER: TracingContextManager = TracingContextManager::new();
}

pub fn get_global_context_manager() -> &'static TracingContextManager {
    &GLOBAL_CONTEXT_MANAGER
}

/// Performance tracing utilities
pub mod performance {
    use super::*;
    use std::time::Instant;

    pub struct PerformanceSpan {
        span: opentelemetry::trace::Span,
        start_time: Instant,
        operation_name: String,
    }

    impl PerformanceSpan {
        pub fn new(operation_name: &str) -> Self {
            let span = create_operation_span(operation_name);
            let start_time = Instant::now();

            Self {
                span,
                start_time,
                operation_name: operation_name.to_string(),
            }
        }

        pub fn add_attribute(&mut self, key: &str, value: &str) {
            self.span.set_attribute(KeyValue::new(key.to_string(), value.to_string()));
        }

        pub fn record_error(&mut self, error: &str) {
            self.span.set_status(opentelemetry::trace::Status::error(error));
            self.span.set_attribute(KeyValue::new("error", true));
            self.span.set_attribute(KeyValue::new("error.message", error.to_string()));
        }
    }

    impl Drop for PerformanceSpan {
        fn drop(&mut self) {
            let duration = self.start_time.elapsed();
            self.span.set_attribute(KeyValue::new("duration_ms", duration.as_millis() as i64));
            self.span.set_attribute(KeyValue::new("duration_ns", duration.as_nanos() as i64));

            // Record performance metrics
            crate::observability::metrics::observe_duration(&self.operation_name, duration);

            // End the span
            self.span.end();
        }
    }

    #[macro_export]
    macro_rules! trace_performance {
        ($operation:expr) => {{
            let mut perf_span = $crate::observability::tracing::performance::PerformanceSpan::new(stringify!($operation));
            let result = $operation;
            result
        }};
    }

    #[macro_export]
    macro_rules! trace_performance_async {
        ($operation:expr) => {{
            async {
                let mut perf_span = $crate::observability::tracing::performance::PerformanceSpan::new(stringify!($operation));
                let result = $operation.await;
                result
            }
        }};
    }
}

/// Blockchain-specific tracing
pub mod blockchain {
    use super::*;

    pub fn trace_transaction_lifecycle(tx_hash: &str) -> PerformanceSpan {
        let mut span = performance::PerformanceSpan::new("transaction_lifecycle");
        span.add_attribute("transaction.hash", tx_hash);
        span.add_attribute("transaction.phase", "received");
        span
    }

    pub fn trace_block_production(block_height: u64) -> PerformanceSpan {
        let mut span = performance::PerformanceSpan::new("block_production");
        span.add_attribute("block.height", &block_height.to_string());
        span
    }

    pub fn trace_consensus_round(round: u64, algorithm: &str) -> PerformanceSpan {
        let mut span = performance::PerformanceSpan::new("consensus_round");
        span.add_attribute("consensus.round", &round.to_string());
        span.add_attribute("consensus.algorithm", algorithm);
        span
    }

    pub fn trace_network_message(peer_id: &str, message_type: &str) -> PerformanceSpan {
        let mut span = performance::PerformanceSpan::new("network_message");
        span.add_attribute("peer.id", peer_id);
        span.add_attribute("message.type", message_type);
        span
    }
}

/// Error tracing utilities
pub mod error {
    use super::*;

    pub fn trace_error(error: &impl std::error::Error, operation: &str) -> opentelemetry::trace::Span {
        let span = create_operation_span(&format!("error.{}", operation));
        span.set_status(opentelemetry::trace::Status::error(error.to_string()));
        span.set_attribute(KeyValue::new("error", true));
        span.set_attribute(KeyValue::new("error.type", std::any::type_name_of_val(error)));
        span.set_attribute(KeyValue::new("error.message", error.to_string()));
        span
    }

    pub fn trace_panic(operation: &str, panic_info: &std::panic::PanicInfo) -> opentelemetry::trace::Span {
        let span = create_operation_span(&format!("panic.{}", operation));
        span.set_status(opentelemetry::trace::Status::error("panic occurred"));
        span.set_attribute(KeyValue::new("panic", true));
        span.set_attribute(KeyValue::new("panic.location", format!("{:?}", panic_info.location())));
        span.set_attribute(KeyValue::new("panic.message", format!("{:?}", panic_info.payload())));
        span
    }
}
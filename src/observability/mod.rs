//! Observability Module
//!
//! Comprehensive observability system providing:
//! - Structured logging with tracing
//! - Metrics collection (TPS, latency, counters)
//! - Distributed tracing with OpenTelemetry
//! - Health checks and monitoring endpoints

pub mod logging;
pub mod metrics;
pub mod tracing;
pub mod tracing;
pub mod health;
pub mod middleware;

pub use logging::*;
pub use metrics::*;
pub use tracing::*;
pub use tracing::*;
pub use health::*;
pub use middleware::*;

/// Initialize the complete observability stack
pub async fn init_observability(
    service_name: &str,
    version: &str,
    log_level: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging
    init_logging(service_name, log_level)?;

    // Initialize metrics
    init_metrics(service_name, version).await?;

    // Initialize tracing
    init_tracing(service_name, version).await?;

    tracing::info!(
        service_name = %service_name,
        version = %version,
        "Observability system initialized"
    );

    Ok(())
}

/// Shutdown observability systems gracefully
pub async fn shutdown_observability() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Shutting down observability systems");

    // Shutdown tracing
    shutdown_tracing().await?;

    // Shutdown metrics
    shutdown_metrics().await?;

    tracing::info!("Observability systems shut down");
    Ok(())
}

/// Observability configuration
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub service_name: String,
    pub service_version: String,
    pub log_level: String,
    pub metrics_port: u16,
    pub jaeger_endpoint: Option<String>,
    pub enable_tracing: bool,
    pub enable_metrics: bool,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            service_name: "siput-node".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            log_level: "info".to_string(),
            metrics_port: 9090,
            jaeger_endpoint: None,
            enable_tracing: true,
            enable_metrics: true,
        }
    }
}

impl ObservabilityConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = name.into();
        self
    }

    pub fn service_version(mut self, version: impl Into<String>) -> Self {
        self.service_version = version.into();
        self
    }

    pub fn log_level(mut self, level: impl Into<String>) -> Self {
        self.log_level = level.into();
        self
    }

    pub fn metrics_port(mut self, port: u16) -> Self {
        self.metrics_port = port;
        self
    }

    pub fn jaeger_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.jaeger_endpoint = Some(endpoint.into());
        self
    }

    pub fn enable_tracing(mut self, enable: bool) -> Self {
        self.enable_tracing = enable;
        self
    }

    pub fn enable_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        self
    }

    /// Initialize observability with this configuration
    pub async fn init(self) -> Result<(), Box<dyn std::error::Error>> {
        if self.enable_metrics {
            init_metrics(&self.service_name, &self.service_version).await?;
        }

        if self.enable_tracing {
            init_tracing(&self.service_name, &self.service_version).await?;
        }

        init_logging(&self.service_name, &self.log_level)?;

        tracing::info!(
            service_name = %self.service_name,
            service_version = %self.service_version,
            "Observability initialized with config"
        );

        Ok(())
    }
}

/// Global observability macros for easy instrumentation
#[macro_export]
macro_rules! trace_span {
    ($name:expr) => {
        tracing::span!(tracing::Level::TRACE, $name)
    };
    ($name:expr, $($field:tt)*) => {
        tracing::span!(tracing::Level::TRACE, $name, $($field)*)
    };
}

#[macro_export]
macro_rules! debug_span {
    ($name:expr) => {
        tracing::span!(tracing::Level::DEBUG, $name)
    };
    ($name:expr, $($field:tt)*) => {
        tracing::span!(tracing::Level::DEBUG, $name, $($field)*)
    };
}

#[macro_export]
macro_rules! info_span {
    ($name:expr) => {
        tracing::span!(tracing::Level::INFO, $name)
    };
    ($name:expr, $($field:tt)*) => {
        tracing::span!(tracing::Level::INFO, $name, $($field)*)
    };
}

#[macro_export]
macro_rules! warn_span {
    ($name:expr) => {
        tracing::span!(tracing::Level::WARN, $name)
    };
    ($name:expr, $($field:tt)*) => {
        tracing::span!(tracing::Level::WARN, $name, $($field)*)
    };
}

#[macro_export]
macro_rules! error_span {
    ($name:expr) => {
        tracing::span!(tracing::Level::ERROR, $name)
    };
    ($name:expr, $($field:tt)*) => {
        tracing::span!(tracing::Level::ERROR, $name, $($field)*)
    };
}

#[macro_export]
macro_rules! measure_time {
    ($operation:expr) => {{
        let start = std::time::Instant::now();
        let result = $operation;
        let duration = start.elapsed();
        tracing::debug!(
            operation = stringify!($operation),
            duration_ms = duration.as_millis(),
            "Operation completed"
        );
        $crate::observability::metrics::observe_duration(stringify!($operation), duration);
        result
    }};
}

#[macro_export]
macro_rules! measure_time_async {
    ($operation:expr) => {{
        async {
            let start = std::time::Instant::now();
            let result = $operation.await;
            let duration = start.elapsed();
            tracing::debug!(
                operation = stringify!($operation),
                duration_ms = duration.as_millis(),
                "Async operation completed"
            );
            $crate::observability::metrics::observe_duration(stringify!($operation), duration);
            result
        }
    }};
}

#[macro_export]
macro_rules! count_operation {
    ($operation:expr) => {
        $crate::observability::metrics::increment_counter(stringify!($operation));
        $operation
    };
}

#[macro_export]
macro_rules! count_operation_async {
    ($operation:expr) => {
        async {
            $crate::observability::metrics::increment_counter(stringify!($operation));
            $operation.await
        }
    };
}
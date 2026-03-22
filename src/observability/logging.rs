//! Structured Logging
//!
//! Provides structured logging with JSON output, log levels, and contextual information.
//! Integrates with tracing for distributed tracing correlation.

use serde_json::json;
use std::io;
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    fmt::{self, format::Writer},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer, Registry,
};

/// Initialize structured logging
pub fn init_logging(service_name: &str, log_level: &str) -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    let json_layer = fmt::layer()
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .event_format(JsonFormat {
            service_name: service_name.to_string(),
        });

    let console_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(true)
        .with_line_number(true);

    tracing::subscriber::set_global_default(
        Registry::default()
            .with(env_filter)
            .with(console_layer)
            .with(json_layer),
    )?;

    Ok(())
}

/// Custom JSON formatter for structured logging
struct JsonFormat {
    service_name: String,
}

impl<S, N> fmt::FormatEvent<S, N> for JsonFormat
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &fmt::FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let metadata = event.metadata();

        // Build structured log entry
        let mut log_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "level": metadata.level().to_string(),
            "service": self.service_name,
            "target": metadata.target(),
            "file": metadata.file().unwrap_or("unknown"),
            "line": metadata.line().unwrap_or(0),
        });

        // Add span context if available
        if let Some(span) = ctx.lookup_current() {
            let span_id = span.id().into_u64();
            log_entry["span_id"] = json!(span_id);

            // Add span fields
            let extensions = span.extensions();
            if let Some(fields) = extensions.get::<tracing_subscriber::fmt::FormattedFields<N>>() {
                if let Ok(fields_json) = serde_json::from_str::<serde_json::Value>(&fields.to_string()) {
                    if let serde_json::Value::Object(fields_obj) = fields_json {
                        for (key, value) in fields_obj {
                            log_entry[key] = value;
                        }
                    }
                }
            }
        }

        // Add trace_id if available (from OpenTelemetry)
        if let Some(trace_id) = get_trace_id() {
            log_entry["trace_id"] = json!(trace_id);
        }

        // Add event fields
        let mut fields_buffer = String::new();
        let mut serializer = serde_json::Serializer::new(&mut fields_buffer);
        event.record(&mut JsonVisitor(&mut serializer));
        drop(serializer); // Ensure buffer is flushed

        if let Ok(fields_json) = serde_json::from_str::<serde_json::Value>(&fields_buffer) {
            if let serde_json::Value::Object(fields_obj) = fields_json {
                for (key, value) in fields_obj {
                    log_entry[key] = value;
                }
            }
        }

        // Write JSON to output
        write!(writer, "{}", log_entry)
    }
}

/// JSON visitor for event fields
struct JsonVisitor<'a, W: io::Write>(&'a mut serde_json::Serializer<W>);

impl<'a, W: io::Write> tracing::field::Visit for JsonVisitor<'a, W> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        // Simplified implementation - just write the field directly
        let _ = write!(self.0.into_inner(), "\"{}\":\"{:?}\"", field.name(), value);
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let _ = write!(self.0.into_inner(), "\"{}\":\"{}\"", field.name(), value);
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        let _ = write!(self.0.into_inner(), "\"{}\":{}", field.name(), value);
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        let _ = write!(self.0.into_inner(), "\"{}\":{}", field.name(), value);
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        let _ = write!(self.0.into_inner(), "\"{}\":{}", field.name(), value);
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        let _ = write!(self.0.into_inner(), "\"{}\":{}", field.name(), value);
    }
}

/// Get current trace ID from OpenTelemetry context
fn get_trace_id() -> Option<String> {
    // This would integrate with OpenTelemetry to get the current trace ID
    // For now, return None - will be implemented when tracing is set up
    None
}

/// Logging macros for different log levels with structured data
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        tracing::error!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        tracing::warn!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        tracing::info!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        tracing::debug!($($arg)*);
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        tracing::trace!($($arg)*);
    };
}

/// Structured logging for blockchain operations
pub mod blockchain {
    use super::*;

    pub fn log_transaction_received(tx_hash: &str, from: &str, to: Option<&str>, amount: Option<u64>) {
        let mut fields = json!({
            "event": "transaction_received",
            "tx_hash": tx_hash,
            "from": from,
        });

        if let Some(to_addr) = to {
            fields["to"] = json!(to_addr);
        }
        if let Some(amt) = amount {
            fields["amount"] = json!(amt);
        }

        tracing::info!(
            event = "transaction_received",
            tx_hash = %tx_hash,
            from = %from,
            to = ?to,
            amount = ?amount,
            "Transaction received"
        );
    }

    pub fn log_block_mined(block_hash: &str, height: u64, tx_count: usize, mining_time_ms: u128) {
        tracing::info!(
            event = "block_mined",
            block_hash = %block_hash,
            height = height,
            tx_count = tx_count,
            mining_time_ms = mining_time_ms,
            "Block mined successfully"
        );
    }

    pub fn log_consensus_reached(block_hash: &str, round: u64, votes: usize) {
        tracing::info!(
            event = "consensus_reached",
            block_hash = %block_hash,
            round = round,
            votes = votes,
            "Consensus reached"
        );
    }

    pub fn log_network_message_received(peer_id: &str, message_type: &str, size_bytes: usize) {
        tracing::debug!(
            event = "network_message_received",
            peer_id = %peer_id,
            message_type = %message_type,
            size_bytes = size_bytes,
            "Network message received"
        );
    }

    pub fn log_rpc_request(method: &str, params: Option<&str>, response_time_ms: u128) {
        tracing::info!(
            event = "rpc_request",
            method = %method,
            params = ?params,
            response_time_ms = response_time_ms,
            "RPC request processed"
        );
    }
}

/// Performance logging
pub mod performance {
    use super::*;

    pub fn log_slow_operation(operation: &str, duration_ms: u128, threshold_ms: u128) {
        if duration_ms > threshold_ms {
            tracing::warn!(
                event = "slow_operation",
                operation = %operation,
                duration_ms = duration_ms,
                threshold_ms = threshold_ms,
                "Operation exceeded performance threshold"
            );
        }
    }

    pub fn log_high_memory_usage(component: &str, memory_mb: f64, threshold_mb: f64) {
        if memory_mb > threshold_mb {
            tracing::warn!(
                event = "high_memory_usage",
                component = %component,
                memory_mb = memory_mb,
                threshold_mb = threshold_mb,
                "High memory usage detected"
            );
        }
    }

    pub fn log_high_cpu_usage(component: &str, cpu_percent: f64, threshold_percent: f64) {
        if cpu_percent > threshold_percent {
            tracing::warn!(
                event = "high_cpu_usage",
                component = %component,
                cpu_percent = cpu_percent,
                threshold_percent = threshold_percent,
                "High CPU usage detected"
            );
        }
    }
}

/// Security logging
pub mod security {
    use super::*;

    pub fn log_authentication_attempt(user: &str, success: bool, ip: Option<&str>) {
        if success {
            tracing::event!(
                tracing::Level::INFO,
                event = "auth_success",
                user = %user,
                ip = ?ip,
                success = success,
                "Authentication attempt"
            );
        } else {
            tracing::event!(
                tracing::Level::WARN,
                event = "auth_failure",
                user = %user,
                ip = ?ip,
                success = success,
                "Authentication attempt"
            );
        }
    }

    pub fn log_unauthorized_access(resource: &str, user: Option<&str>, ip: &str) {
        tracing::warn!(
            event = "unauthorized_access",
            resource = %resource,
            user = ?user,
            ip = %ip,
            "Unauthorized access attempt"
        );
    }

    pub fn log_rate_limit_exceeded(endpoint: &str, ip: &str, requests_per_minute: u32) {
        tracing::warn!(
            event = "rate_limit_exceeded",
            endpoint = %endpoint,
            ip = %ip,
            requests_per_minute = requests_per_minute,
            "Rate limit exceeded"
        );
    }

    pub fn log_suspicious_activity(activity: &str, details: &str, ip: &str) {
        tracing::warn!(
            event = "suspicious_activity",
            activity = %activity,
            details = %details,
            ip = %ip,
            "Suspicious activity detected"
        );
    }
}
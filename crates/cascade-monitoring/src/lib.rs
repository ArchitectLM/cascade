//! Simplified monitoring module for Cascade platform.

use std::time::Duration;
use tracing::info;
use std::collections::HashMap;
use std::any::Any;
use futures::future::BoxFuture;
use serde_json::Value;

pub mod metrics;

/// Type of metric for collection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    /// Counter metrics accumulate values
    Counter,
    /// Gauge metrics record current values
    Gauge,
    /// Histogram metrics observe distributions
    Histogram,
    /// Summary metrics collect observations with quantiles
    Summary,
}

/// Interface for collecting metrics
pub trait MetricsCollector: Send + Sync {
    /// Record a metric with the given name, value, type, and labels
    fn record_metric(&self, name: &str, value: f64, metric_type: MetricType, labels: HashMap<String, String>);
    
    /// Flush metrics to the backend
    fn flush(&self) -> BoxFuture<'static, Result<(), String>>;
    
    /// Convert to Any for downcasting in tests
    fn as_any(&self) -> &dyn Any;
}

/// Interface for collecting logs
pub trait LogCollector: Send + Sync {
    /// Record a log with the given level, message, and metadata
    fn record_log(&self, level: &str, message: &str, metadata: HashMap<String, Value>);
    
    /// Flush logs to the backend
    fn flush(&self) -> BoxFuture<'static, Result<(), String>>;
    
    /// Convert to Any for downcasting in tests
    fn as_any(&self) -> &dyn Any;
}

/// Interface for collecting traces
pub trait TraceCollector: Send + Sync {
    /// Record a span with the given name, attributes, start time, and end time
    fn record_span(&self, name: &str, attributes: HashMap<String, String>, start_time: u64, end_time: u64);
    
    /// Flush traces to the backend
    fn flush(&self) -> BoxFuture<'static, Result<(), String>>;
    
    /// Convert to Any for downcasting in tests
    fn as_any(&self) -> &dyn Any;
}

/// Combined collector interface
pub trait Collector: Send + Sync {
    /// Get the metrics collector
    fn metrics(&self) -> &dyn MetricsCollector;
    
    /// Get the log collector
    fn logs(&self) -> &dyn LogCollector;
    
    /// Get the trace collector
    fn traces(&self) -> &dyn TraceCollector;
    
    /// Flush all collectors
    fn flush(&self) -> BoxFuture<'static, Result<(), String>>;
}

/// Configuration for initializing the monitoring system
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    /// Service name used for tracing and metrics
    pub service_name: String,
    /// Enable metrics
    pub enable_metrics: bool,
    /// Log level filter (e.g., "info,cascade=debug")
    pub log_filter: String,
    /// How often to export metrics
    pub metrics_interval: Duration,
    /// Environment (dev, staging, prod)
    pub environment: String,
    /// Interval for flushing metrics
    pub flush_interval: Duration,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            service_name: "cascade".to_string(),
            enable_metrics: true,
            log_filter: "info".to_string(),
            metrics_interval: Duration::from_secs(10),
            environment: "dev".to_string(),
            flush_interval: Duration::from_secs(60),
        }
    }
}

/// Initialize monitoring system
pub fn init(config: MonitoringConfig) -> anyhow::Result<()> {
    info!("Initializing monitoring system");
    
    info!("Monitoring initialized with config: {:?}", config);
    
    Ok(())
}

/// Shutdown the monitoring system
pub fn shutdown() {
    info!("Shutting down monitoring system");
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_defaults() {
        let config = MonitoringConfig::default();
        assert_eq!(config.service_name, "cascade");
        assert!(config.enable_metrics);
    }
}

// Exported types
pub use crate::metrics::*; 
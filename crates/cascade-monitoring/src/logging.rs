//! Structured logging module using tracing.
//!
//! This module provides structured logging functionality using the tracing crate
//! with JSON formatting for log aggregation.

use anyhow::Context;
use std::io;
use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    prelude::*,
    EnvFilter,
};

use crate::MonitoringConfig;

/// Initialize structured logging
pub fn init_logging(config: &MonitoringConfig) -> anyhow::Result<()> {
    info!("Initializing structured logging");
    
    // Configure a subscriber to collect and format logs
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_filter));
    
    // Build up the subscriber
    let mut subscriber = tracing_subscriber::registry().with(env_filter);
    
    // Add stdout/stderr layer
    if config.enable_json_logging {
        // JSON logs for production
        let json_layer = fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);
        
        subscriber = subscriber.with(json_layer);
    } else {
        // Pretty logs for development
        let fmt_layer = fmt::layer()
            .pretty()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);
        
        subscriber = subscriber.with(fmt_layer);
    }
    
    // Add file logging if configured
    if let Some(log_file) = &config.log_file {
        // Setup log file with daily rotation
        let file_appender = RollingFileAppender::new(
            Rotation::DAILY,
            std::path::Path::new(log_file).parent().unwrap_or_else(|| std::path::Path::new(".")),
            log_file.clone(),
        );
        
        // Create a JSON layer that writes to file
        let file_layer = fmt::layer()
            .json()
            .with_current_span(true)
            .with_ansi(false)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_writer(file_appender);
        
        subscriber = subscriber.with(file_layer);
    }
    
    // Set the subscriber as the default
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set global default subscriber")?;
    
    info!(
        service_name = %config.service_name,
        log_format = if config.enable_json_logging { "json" } else { "pretty" },
        log_file = ?config.log_file,
        "Logging initialized"
    );
    
    Ok(())
}

/// Log correlation ID middleware
#[cfg(feature = "axum")]
pub mod axum_integration {
    use axum::{
        extract::Request,
        http::HeaderMap,
        middleware::Next,
        response::Response,
    };
    use std::time::Instant;
    use tracing::{Instrument, info_span};
    use uuid::Uuid;
    
    /// Get correlation ID from headers or generate a new one
    pub fn get_correlation_id(headers: &HeaderMap) -> String {
        headers.get("x-correlation-id")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string())
    }
    
    /// Middleware for adding correlation ID to logs
    pub async fn correlation_id_middleware<B>(req: Request<B>, next: Next<B>) -> Response {
        let correlation_id = get_correlation_id(req.headers());
        
        // Create a span with the correlation ID
        let span = info_span!(
            "request",
            correlation_id = %correlation_id,
            method = %req.method(),
            uri = %req.uri(),
        );
        
        // Process the request with the span
        next.run(req).instrument(span).await
    }
}

/// Trait to add log context to results
pub trait LogExt<T, E> {
    /// Log error with additional context before returning
    fn log_err(self, message: &str) -> Result<T, E>;
    
    /// Log success with additional context before returning
    fn log_ok(self, message: &str) -> Result<T, E>;
}

impl<T, E: std::fmt::Display> LogExt<T, E> for Result<T, E> {
    fn log_err(self, message: &str) -> Result<T, E> {
        if let Err(ref e) = self {
            tracing::error!("{}: {}", message, e);
        }
        self
    }
    
    fn log_ok(self, message: &str) -> Result<T, E> {
        if let Ok(_) = self {
            tracing::info!("{}", message);
        }
        self
    }
}

/// Initializes test tracing for unit tests
#[cfg(test)]
pub fn init_test_tracing() {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .with_test_writer()
        .finish();
        
    let _ = tracing::subscriber::set_global_default(subscriber);
} 
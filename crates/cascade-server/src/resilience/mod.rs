//! Resilience module for Cascade server
//! Provides resilience patterns like rate limiting, circuit breaking, retries, etc.

use std::time::{SystemTime, UNIX_EPOCH};
use serde::Deserialize;

pub mod rate_limiter;
pub mod idempotency;
pub mod bulkhead;
pub mod circuit_breaker;

pub use rate_limiter::RateLimiter;
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerKey, CircuitStatus};
pub use bulkhead::{Bulkhead, BulkheadConfig, BulkheadKey, BulkheadMetrics};
pub use idempotency::IdempotencyHandler;

/// Common configuration for resilience patterns
#[derive(Debug, Clone, Deserialize)]
pub struct ResilienceConfig {
    /// Optional shared state scope for distributed patterns
    pub shared_state_scope: Option<String>,
    
    /// Maximum retries for retry patterns
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    
    /// Backoff factor for retry backoff (exponential)
    #[serde(default = "default_backoff_factor")]
    pub backoff_factor: f64,
    
    /// Maximum backoff duration in milliseconds
    #[serde(default = "default_max_backoff_ms")]
    pub max_backoff_ms: u64,
    
    /// Rate limit max requests
    #[serde(default = "default_rate_limit_max")]
    pub rate_limit_max: u32,
    
    /// Rate limit window in milliseconds
    #[serde(default = "default_rate_limit_window_ms")]
    pub rate_limit_window_ms: u64,
    
    /// Circuit breaker failure threshold
    #[serde(default = "default_circuit_threshold")]
    pub circuit_threshold: u32,
    
    /// Circuit breaker reset timeout in milliseconds
    #[serde(default = "default_circuit_reset_ms")]
    pub circuit_reset_ms: u64,
    
    /// Bulkhead max concurrent requests
    #[serde(default = "default_bulkhead_max_concurrent")]
    pub bulkhead_max_concurrent: u32,
    
    /// Bulkhead max queue size
    #[serde(default = "default_bulkhead_max_queue_size")]
    pub bulkhead_max_queue_size: u32,
    
    /// Bulkhead queue timeout in milliseconds
    #[serde(default = "default_bulkhead_queue_timeout_ms")]
    pub bulkhead_queue_timeout_ms: u64,
}

// Default values
fn default_max_retries() -> u32 { 3 }
fn default_backoff_factor() -> f64 { 2.0 }
fn default_max_backoff_ms() -> u64 { 30000 }
fn default_rate_limit_max() -> u32 { 100 }
fn default_rate_limit_window_ms() -> u64 { 60000 }
fn default_circuit_threshold() -> u32 { 5 }
fn default_circuit_reset_ms() -> u64 { 30000 }
fn default_bulkhead_max_concurrent() -> u32 { 10 }
fn default_bulkhead_max_queue_size() -> u32 { 20 }
fn default_bulkhead_queue_timeout_ms() -> u64 { 1000 }

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            shared_state_scope: None,
            max_retries: default_max_retries(),
            backoff_factor: default_backoff_factor(),
            max_backoff_ms: default_max_backoff_ms(),
            rate_limit_max: default_rate_limit_max(),
            rate_limit_window_ms: default_rate_limit_window_ms(),
            circuit_threshold: default_circuit_threshold(),
            circuit_reset_ms: default_circuit_reset_ms(),
            bulkhead_max_concurrent: default_bulkhead_max_concurrent(),
            bulkhead_max_queue_size: default_bulkhead_max_queue_size(),
            bulkhead_queue_timeout_ms: default_bulkhead_queue_timeout_ms(),
        }
    }
}

/// Current timestamp in milliseconds
pub(crate) fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
} 
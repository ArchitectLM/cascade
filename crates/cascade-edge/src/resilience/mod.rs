//! Resilience patterns for edge components
//!
//! This module provides implementations of common resilience patterns
//! to improve the robustness of edge services

use cascade_core::CoreError;
use serde_json::Value;
use async_trait::async_trait;

pub mod circuit_breaker;
pub mod bulkhead;
pub mod throttling;
pub mod dlq;
pub mod mocks;
pub mod adapters;

// Re-export common components
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState};
pub use bulkhead::{Bulkhead, BulkheadConfig};
pub use throttling::{RateLimiter, RateLimiterConfig, TimeWindow};
pub use dlq::{DeadLetterQueue, DlqConfig};
pub use adapters::{InMemoryStateStore, RepositoryStateStore};

/// A simplified state store interface for resilience patterns
///
/// This trait defines a minimal interface for storing and retrieving state
/// that resilience patterns can use to persist their state.
#[async_trait]
pub trait StateStore: Send + Sync {
    /// Get the state of a flow instance
    async fn get_flow_instance_state(&self, flow_id: &str, instance_id: &str) -> Result<Option<Value>, CoreError>;

    /// Set the state of a flow instance
    async fn set_flow_instance_state(&self, flow_id: &str, instance_id: &str, state: Value) -> Result<(), CoreError>;
    
    /// Get the state of a component
    async fn get_component_state(&self, component_id: &str) -> Result<Option<Value>, CoreError>;
    
    /// Set the state of a component
    async fn set_component_state(&self, component_id: &str, state: Value) -> Result<(), CoreError>;
}

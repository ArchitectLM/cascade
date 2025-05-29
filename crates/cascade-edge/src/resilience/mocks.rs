use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::Value;
use cascade_core::CoreError;

use super::StateStore;

/// Mock implementation of `StateStore` for testing resilience patterns
#[derive(Debug, Clone)]
pub struct MockStateStore {
    flow_states: Arc<Mutex<HashMap<String, Value>>>,
    component_states: Arc<Mutex<HashMap<String, Value>>>,
}

impl MockStateStore {
    /// Create a new `MockStateStore` for testing
    pub fn new() -> Self {
        Self {
            flow_states: Arc::new(Mutex::new(HashMap::new())),
            component_states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Clear all stored state
    pub async fn clear(&self) {
        self.flow_states.lock().await.clear();
        self.component_states.lock().await.clear();
    }
}

#[async_trait::async_trait]
impl StateStore for MockStateStore {
    async fn get_flow_instance_state(&self, flow_id: &str, instance_id: &str) -> Result<Option<Value>, CoreError> {
        let key = format!("{}/{}", flow_id, instance_id);
        let states = self.flow_states.lock().await;
        Ok(states.get(&key).cloned())
    }

    async fn set_flow_instance_state(&self, flow_id: &str, instance_id: &str, state: Value) -> Result<(), CoreError> {
        let key = format!("{}/{}", flow_id, instance_id);
        self.flow_states.lock().await.insert(key, state);
        Ok(())
    }

    async fn get_component_state(&self, component_id: &str) -> Result<Option<Value>, CoreError> {
        let states = self.component_states.lock().await;
        Ok(states.get(component_id).cloned())
    }

    async fn set_component_state(&self, component_id: &str, state: Value) -> Result<(), CoreError> {
        self.component_states.lock().await.insert(component_id.to_string(), state);
        Ok(())
    }
}

/// Utility factory functions for resilience pattern testing
pub mod test_utils {
    use super::*;
    use crate::resilience::{
        circuit_breaker::{CircuitBreaker, CircuitBreakerConfig},
        bulkhead::{Bulkhead, BulkheadConfig},
        throttling::{RateLimiter, RateLimiterConfig},
        dlq::{DeadLetterQueue, DlqConfig, RetryPolicy},
    };

    /// Create a test circuit breaker with custom configuration
    pub fn create_test_circuit_breaker(
        id: &str, 
        config: Option<CircuitBreakerConfig>
    ) -> (CircuitBreaker, Arc<MockStateStore>) {
        let state_store = Arc::new(MockStateStore::new());
        let config = config.unwrap_or_default();
        
        (CircuitBreaker::new(id.to_string(), config, state_store.clone()), state_store)
    }

    /// Create a test bulkhead with custom configuration
    pub fn create_test_bulkhead(
        id: &str,
        config: Option<BulkheadConfig>
    ) -> Bulkhead {
        let config = config.unwrap_or_default();
        Bulkhead::new(id.to_string(), config)
    }

    /// Create a test rate limiter with custom configuration
    pub fn create_test_rate_limiter(
        id: &str,
        config: Option<RateLimiterConfig>
    ) -> RateLimiter {
        let config = config.unwrap_or_default();
        RateLimiter::new(id.to_string(), config)
    }

    /// Create a test DLQ with custom configuration
    pub fn create_test_dlq(
        id: &str,
        config: Option<DlqConfig>,
        retry_policy: Option<RetryPolicy>
    ) -> (DeadLetterQueue, Arc<MockStateStore>) {
        let state_store = Arc::new(MockStateStore::new());
        let mut config = config.unwrap_or_default();
        
        if let Some(retry) = retry_policy {
            config.retry_policy = retry;
        }
        
        (DeadLetterQueue::new(id.to_string(), config, state_store.clone()), state_store)
    }
} 
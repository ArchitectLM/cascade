//! Resilience patterns for testing

pub mod circuit_breaker;

pub use circuit_breaker::CircuitBreaker;

use serde_json::Value;
use crate::mocks::component::ComponentRuntimeAPI;
use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;

/// A simplified state store interface for resilience patterns
#[async_trait]
pub trait StateStore: Send + Sync {
    /// Get the state of a flow instance
    async fn get_flow_instance_state(&self, flow_id: &str, instance_id: &str) -> Result<Option<Value>>;

    /// Set the state of a flow instance
    async fn set_flow_instance_state(&self, flow_id: &str, instance_id: &str, state: Value) -> Result<()>;
    
    /// Get the state of a component
    async fn get_component_state(&self, component_id: &str) -> Result<Option<Value>>;
    
    /// Set the state of a component
    async fn set_component_state(&self, component_id: &str, state: Value) -> Result<()>;
}

/// Mock implementation of StateStore for tests
pub struct MockRuntimeStateStore {
    runtime_api: Arc<dyn ComponentRuntimeAPI>,
}

impl MockRuntimeStateStore {
    /// Create a new MockRuntimeStateStore
    pub fn new(runtime_api: Arc<dyn ComponentRuntimeAPI>) -> Self {
        Self { runtime_api }
    }
}

#[async_trait]
impl StateStore for MockRuntimeStateStore {
    async fn get_flow_instance_state(&self, flow_id: &str, instance_id: &str) -> Result<Option<Value>> {
        let key = format!("flow:{}:instance:{}_resilience_state", flow_id, instance_id);
        match self.runtime_api.get_state(&key).await {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Failed to get flow instance state: {}", e)),
        }
    }

    async fn set_flow_instance_state(&self, flow_id: &str, instance_id: &str, state: Value) -> Result<()> {
        let key = format!("flow:{}:instance:{}_resilience_state", flow_id, instance_id);
        match self.runtime_api.set_state(&key, state).await {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Failed to set flow instance state: {}", e)),
        }
    }

    async fn get_component_state(&self, component_id: &str) -> Result<Option<Value>> {
        let key = format!("component:{}_resilience_state", component_id);
        match self.runtime_api.get_state(&key).await {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Failed to get component state: {}", e)),
        }
    }

    async fn set_component_state(&self, component_id: &str, state: Value) -> Result<()> {
        let key = format!("component:{}_resilience_state", component_id);
        match self.runtime_api.set_state(&key, state).await {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Failed to set component state: {}", e)),
        }
    }
} 
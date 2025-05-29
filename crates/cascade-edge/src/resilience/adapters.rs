//! StateStore adapters for resilience patterns
//!
//! This module provides adapters for integrating the resilience patterns with
//! different state storage mechanisms

use super::StateStore;
use async_trait::async_trait;
use cascade_core::{
    CoreError,
    domain::repository::{
        ComponentStateRepository, FlowInstanceRepository, 
        memory::{MemoryComponentStateRepository, MemoryFlowInstanceRepository}
    },
    domain::flow_instance::{FlowInstanceId, StepId},
    application::runtime_interface::RuntimeInterface,
};
use serde_json::Value;
use std::sync::Arc;

/// StateStore implementation that uses memory repositories
///
/// This adapter is primarily useful for testing or standalone usage
/// of resilience patterns without requiring an external database.
pub struct InMemoryStateStore {
    component_state_repo: Arc<MemoryComponentStateRepository>,
    #[allow(dead_code)]
    flow_instance_repo: Arc<MemoryFlowInstanceRepository>,
}

impl InMemoryStateStore {
    /// Create a new in-memory state store
    pub fn new() -> Self {
        Self {
            component_state_repo: Arc::new(MemoryComponentStateRepository::new()),
            flow_instance_repo: Arc::new(MemoryFlowInstanceRepository::new()),
        }
    }
}

impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateStore for InMemoryStateStore {
    async fn get_flow_instance_state(&self, flow_id: &str, instance_id: &str) -> Result<Option<Value>, CoreError> {
        self.component_state_repo.get_state(
            &FlowInstanceId(instance_id.to_string()),
            &StepId(format!("{}_resilience_state", flow_id))
        ).await
    }

    async fn set_flow_instance_state(&self, flow_id: &str, instance_id: &str, state: Value) -> Result<(), CoreError> {
        self.component_state_repo.save_state(
            &FlowInstanceId(instance_id.to_string()),
            &StepId(format!("{}_resilience_state", flow_id)),
            state
        ).await
    }
    
    async fn get_component_state(&self, component_id: &str) -> Result<Option<Value>, CoreError> {
        self.component_state_repo.get_state(
            &FlowInstanceId("global".to_string()),
            &StepId(format!("{}_resilience_state", component_id))
        ).await
    }
    
    async fn set_component_state(&self, component_id: &str, state: Value) -> Result<(), CoreError> {
        self.component_state_repo.save_state(
            &FlowInstanceId("global".to_string()),
            &StepId(format!("{}_resilience_state", component_id)),
            state
        ).await
    }
}

/// StateStore implementation that uses repository interfaces
///
/// This adapter bridges the simplified StateStore interface with
/// the core repository interfaces used throughout the Cascade platform.
pub struct RepositoryStateStore {
    component_state_repo: Arc<dyn ComponentStateRepository>,
    #[allow(dead_code)]
    flow_instance_repo: Arc<dyn FlowInstanceRepository>,
}

impl RepositoryStateStore {
    /// Create a new repository-based state store
    pub fn new(
        component_state_repo: Arc<dyn ComponentStateRepository>,
        flow_instance_repo: Arc<dyn FlowInstanceRepository>,
    ) -> Self {
        Self {
            component_state_repo,
            flow_instance_repo,
        }
    }
    
    /// Create a state store from runtime interface
    ///
    /// This extracts the repositories from a RuntimeInterface, making
    /// it easy to connect resilience patterns to the existing state storage.
    pub fn from_runtime(_runtime: &RuntimeInterface) -> Self {
        // TODO: When RuntimeInterface exposes repositories, extract them here
        // For now, we'll have to create repositories separately
        
        // This will be implemented when the RuntimeInterface is updated
        // to expose its repositories
        unimplemented!("Runtime interface does not yet expose repositories")
    }
}

#[async_trait]
impl StateStore for RepositoryStateStore {
    async fn get_flow_instance_state(&self, flow_id: &str, instance_id: &str) -> Result<Option<Value>, CoreError> {
        self.component_state_repo.get_state(
            &FlowInstanceId(instance_id.to_string()),
            &StepId(format!("{}_resilience_state", flow_id))
        ).await
    }

    async fn set_flow_instance_state(&self, flow_id: &str, instance_id: &str, state: Value) -> Result<(), CoreError> {
        self.component_state_repo.save_state(
            &FlowInstanceId(instance_id.to_string()),
            &StepId(format!("{}_resilience_state", flow_id)),
            state
        ).await
    }
    
    async fn get_component_state(&self, component_id: &str) -> Result<Option<Value>, CoreError> {
        self.component_state_repo.get_state(
            &FlowInstanceId("global".to_string()),
            &StepId(format!("{}_resilience_state", component_id))
        ).await
    }
    
    async fn set_component_state(&self, component_id: &str, state: Value) -> Result<(), CoreError> {
        self.component_state_repo.save_state(
            &FlowInstanceId("global".to_string()),
            &StepId(format!("{}_resilience_state", component_id)),
            state
        ).await
    }
} 
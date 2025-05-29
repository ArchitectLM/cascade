//! Mock implementation of the CoreRuntimeAPI trait.

use async_trait::async_trait;
use mockall::mock;
use mockall::predicate::*;
use serde_json::Value;
use std::collections::HashMap;

/// FlowState represents the current state of a flow instance
#[derive(Debug, Clone, PartialEq)]
pub enum FlowState {
    Created,
    Running,
    Waiting,
    Completed,
    Failed(String),
}

/// FlowInstance represents a running or completed flow instance
#[derive(Debug, Clone)]
pub struct FlowInstance {
    pub instance_id: String,
    pub flow_id: String,
    pub state: FlowState,
    pub data: HashMap<String, Value>,
}

/// Define the CoreRuntimeAPI trait interface based on the actual trait
#[async_trait]
pub trait CoreRuntimeAPI: Send + Sync {
    async fn deploy_dsl(&self, flow_id: &str, dsl: &str) -> Result<(), CoreError>;
    async fn undeploy_flow(&self, flow_id: &str) -> Result<(), CoreError>;
    async fn get_flow_state(&self, instance_id: &str) -> Result<Option<FlowInstance>, CoreError>;
    async fn trigger_flow(&self, flow_id: &str, input: Value) -> Result<String, CoreError>;
    async fn execute_server_step(&self, instance_id: &str, step_id: &str) -> Result<(), CoreError>;
    async fn resume_from_timer(&self, instance_id: &str, timer_id: &str) -> Result<(), CoreError>;
}

/// Error type for core operations
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Invalid DSL: {0}")]
    InvalidDsl(String),
    #[error("Flow not found: {0}")]
    FlowNotFound(String),
    #[error("Instance not found: {0}")]
    InstanceNotFound(String),
    #[error("Step not found: {0}")]
    StepNotFound(String),
    #[error("Execution error: {0}")]
    ExecutionError(String),
    #[error("Core error: {0}")]
    Other(String),
}

// Generate the mock implementation
mock! {
    pub CoreRuntimeAPI {}
    
    #[async_trait]
    impl CoreRuntimeAPI for CoreRuntimeAPI {
        async fn deploy_dsl(&self, flow_id: &str, dsl: &str) -> Result<(), CoreError>;
        async fn undeploy_flow(&self, flow_id: &str) -> Result<(), CoreError>;
        async fn get_flow_state(&self, instance_id: &str) -> Result<Option<FlowInstance>, CoreError>;
        async fn trigger_flow(&self, flow_id: &str, input: Value) -> Result<String, CoreError>;
        async fn execute_server_step(&self, instance_id: &str, step_id: &str) -> Result<(), CoreError>;
        async fn resume_from_timer(&self, instance_id: &str, timer_id: &str) -> Result<(), CoreError>;
    }
}

/// Creates a new mock CoreRuntimeAPI instance with default expectations.
pub fn create_mock_core_runtime_api() -> MockCoreRuntimeAPI {
    let mut mock = MockCoreRuntimeAPI::new();
    
    // Set up default behaviors for common methods
    mock.expect_deploy_dsl()
        .returning(|_, _| Ok(()));
    
    mock.expect_trigger_flow()
        .returning(|flow_id, _| Ok(format!("{}-instance-{}", flow_id, uuid::Uuid::new_v4())));
    
    mock
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_mock_core_runtime_default_behavior() {
        let mock = create_mock_core_runtime_api();
        
        let result = mock.deploy_dsl("test-flow", "flow: definition").await;
        assert!(result.is_ok());
        
        let instance_id = mock.trigger_flow("test-flow", json!({})).await.unwrap();
        assert!(instance_id.starts_with("test-flow-instance-"));
    }
    
    #[tokio::test]
    async fn test_mock_core_runtime_custom_behavior() {
        let mut mock = create_mock_core_runtime_api();
        
        // Configure custom behavior
        mock.expect_get_flow_state()
            .returning(|instance_id| {
                Ok(Some(FlowInstance {
                    instance_id: instance_id.to_string(),
                    flow_id: "test-flow".to_string(),
                    state: FlowState::Completed,
                    data: HashMap::new(),
                }))
            });
        
        // Test the behavior
        let state = mock.get_flow_state("test-instance").await.unwrap().unwrap();
        assert_eq!(state.instance_id, "test-instance");
        assert_eq!(state.state, FlowState::Completed);
    }
} 
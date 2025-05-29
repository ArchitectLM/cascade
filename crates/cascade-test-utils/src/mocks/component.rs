//! Mock implementation of the ComponentExecutor trait.

use async_trait::async_trait;
use mockall::mock;
use mockall::predicate::*;
use serde_json::Value;
use std::sync::Arc;
use std::collections::HashMap;
use std::fmt;

/// A mock implementation of the DataPacket from cascade-core
#[derive(Debug, Clone, PartialEq)]
pub struct DataPacket {
    pub value: Value,
}

impl DataPacket {
    /// Create a new data packet from a JSON value
    pub fn new(value: Value) -> Self {
        Self { value }
    }
}

/// LogLevel used for logging in components
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Define the ComponentRuntimeAPI trait interface
#[async_trait]
pub trait ComponentRuntimeAPI: Send + Sync {
    async fn log(&self, level: LogLevel, message: &str) -> Result<(), ComponentError>;
    async fn get_input(&self, name: &str) -> Result<Option<DataPacket>, ComponentError>;
    async fn set_output(&self, name: &str, data: DataPacket) -> Result<(), ComponentError>;
    async fn get_state(&self, key: &str) -> Result<Option<Value>, ComponentError>;
    async fn set_state(&self, key: &str, value: Value) -> Result<(), ComponentError>;
    fn name(&self) -> &str;
}

/// Define the ComponentExecutor trait interface
#[async_trait]
pub trait ComponentExecutor: Send + Sync {
    async fn execute(&self, runtime_api: Arc<dyn ComponentRuntimeAPI>) -> Result<(), ComponentError>;
}

/// Error type for component operations
#[derive(Debug, thiserror::Error)]
pub enum ComponentError {
    #[error("Missing required input: {0}")]
    MissingInput(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Execution error: {0}")]
    ExecutionError(String),
    #[error("State error: {0}")]
    StateError(String),
    #[error("Component error: {0}")]
    Other(String),
}

// Generate the mock implementation for ComponentExecutor
mock! {
    pub ComponentExecutor {}
    
    #[async_trait]
    impl ComponentExecutor for ComponentExecutor {
        async fn execute(&self, runtime_api: Arc<dyn ComponentRuntimeAPI>) -> Result<(), ComponentError>;
    }
}

// Generate the mock implementation for ComponentRuntimeAPI
mock! {
    pub ComponentRuntimeAPI {}
    
    #[async_trait]
    impl ComponentRuntimeAPI for ComponentRuntimeAPI {
        async fn log(&self, level: LogLevel, message: &str) -> Result<(), ComponentError>;
        async fn get_input(&self, name: &str) -> Result<Option<DataPacket>, ComponentError>;
        async fn set_output(&self, name: &str, data: DataPacket) -> Result<(), ComponentError>;
        async fn get_state(&self, key: &str) -> Result<Option<Value>, ComponentError>;
        async fn set_state(&self, key: &str, value: Value) -> Result<(), ComponentError>;
        fn name(&self) -> &str;
    }
}

/// Creates a new mock ComponentExecutor instance with default expectations.
pub fn create_mock_component_executor() -> MockComponentExecutor {
    let mut mock = MockComponentExecutor::new();
    
    // Default expectation just succeeds
    mock.expect_execute()
        .returning(|_| Ok(()));
    
    mock
}

/// Creates a new mock ComponentRuntimeAPI instance with default expectations.
pub fn create_mock_component_runtime_api(name: &str) -> MockComponentRuntimeAPI {
    let mut mock = MockComponentRuntimeAPI::new();
    
    let component_name = name.to_string();
    
    // Set up default behaviors for common methods
    mock.expect_name()
        .return_const(component_name);
    
    mock.expect_log()
        .returning(|_, _| Ok(()));
    
    mock.expect_get_input()
        .returning(|_| Ok(None));
    
    mock.expect_set_output()
        .returning(|_, _| Ok(()));
    
    mock.expect_get_state()
        .returning(|_| Ok(None));
    
    mock.expect_set_state()
        .returning(|_, _| Ok(()));
    
    mock
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_mock_component_executor_default_behavior() {
        let mock_executor = create_mock_component_executor();
        let mock_runtime = Arc::new(create_mock_component_runtime_api("test-component"));
        
        let result = mock_executor.execute(mock_runtime).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_mock_component_runtime_default_behavior() {
        let mock_runtime = create_mock_component_runtime_api("test-component");
        
        assert_eq!(mock_runtime.name(), "test-component");
        
        let result = mock_runtime.log(LogLevel::Info, "Test message").await;
        assert!(result.is_ok());
        
        let input = mock_runtime.get_input("test-input").await.unwrap();
        assert!(input.is_none());
    }
    
    #[tokio::test]
    async fn test_mock_component_custom_behavior() {
        let mock_runtime = create_mock_component_runtime_api("test-component");
        
        // Test default behaviors
        assert_eq!(mock_runtime.name(), "test-component");
        
        // Test logging
        let log_result = mock_runtime.log(LogLevel::Info, "Test message").await;
        assert!(log_result.is_ok());
        
        // Test setting an output
        let output_result = mock_runtime.set_output(
            "test-output", 
            DataPacket::new(json!({"result": true}))
        ).await;
        assert!(output_result.is_ok());
    }
} 
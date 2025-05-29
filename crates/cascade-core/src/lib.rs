//!
//! Cascade Core - Core runtime for the Cascade Platform
//!
//! This crate defines the core runtime, domain models, and interfaces
//! for the Cascade Platform. It is the foundation for all other crates
//! in the platform.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

/// Domain layer - core business models, entities, and rules
pub mod domain;

/// Application services - core application logic
pub mod application;

/// Core types and traits
pub mod types;

/// Error types
pub mod error;

// Re-export key types
pub use domain::shared_state::{SharedStateScope, SharedStateService, SharedStateServiceFactory};
pub use error::CoreError;
pub use types::DataPacket;
pub use types::LogLevel;

// Application interfaces
pub use application::runtime_interface::RuntimeInterface;
pub use application::runtime_interface::TimerProcessingRepository;

// Re-export main API types for easy use
pub use domain::flow_definition::FlowDefinition;
pub use domain::flow_instance::{CorrelationId, FlowId, FlowInstanceId, FlowStatus, StepId};
pub use domain::repository::{
    ComponentStateRepository, FlowDefinitionRepository, FlowInstanceRepository, TimerRepository,
};

/// Result of component execution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionResult {
    /// Execution completed successfully
    Success,
    /// Execution failed with an error
    Failure(CoreError),
    /// Execution paused waiting for an external event
    Pending(String),
}

/// Non-async base trait for component executors
/// This trait is object-safe and used as a marker trait
pub trait ComponentExecutorBase: Send + Sync {
    /// Get the component name
    fn component_type(&self) -> &str;
}

/// A component that can be executed as part of a flow
#[async_trait]
pub trait ComponentExecutor: ComponentExecutorBase {
    /// Execute the component
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult;
}

/// Non-async base trait for component runtime
/// This trait is object-safe and used as a marker trait
pub trait ComponentRuntimeApiBase: Send + Sync {
    /// Log information with the specified level
    fn log(&self, level: tracing::Level, message: &str);
}

// Add blanket implementation for Arc<dyn ComponentRuntimeApi>
impl ComponentRuntimeApiBase for Arc<dyn ComponentRuntimeApi> {
    fn log(&self, level: tracing::Level, message: &str) {
        // We can't directly call the async method, so just log with tracing directly
        match level {
            tracing::Level::ERROR => tracing::error!("{}", message),
            tracing::Level::WARN => tracing::warn!("{}", message),
            tracing::Level::INFO => tracing::info!("{}", message),
            tracing::Level::DEBUG => tracing::debug!("{}", message),
            tracing::Level::TRACE => tracing::trace!("{}", message),
        }
    }
}

/// Runtime API provided to components during execution
#[async_trait]
pub trait ComponentRuntimeApi: ComponentRuntimeApiBase {
    /// Get an input value by name
    async fn get_input(&self, name: &str) -> Result<DataPacket, CoreError>;

    /// Get configuration for the component by name
    async fn get_config(&self, name: &str) -> Result<serde_json::Value, CoreError>;

    /// Set an output value by name
    async fn set_output(&self, name: &str, value: DataPacket) -> Result<(), CoreError>;

    /// Get component state
    async fn get_state(&self) -> Result<Option<serde_json::Value>, CoreError>;

    /// Save component state
    async fn save_state(&self, state: serde_json::Value) -> Result<(), CoreError>;

    /// Schedule a timer to resume execution after the specified duration
    async fn schedule_timer(&self, duration: std::time::Duration) -> Result<(), CoreError>;

    /// Get correlation ID for the current flow instance
    async fn correlation_id(&self) -> Result<CorrelationId, CoreError>;

    /// Log message with specified log level
    async fn log(&self, level: LogLevel, message: &str) -> Result<(), CoreError>;

    /// Emit a metric with labels
    async fn emit_metric(
        &self,
        name: &str,
        value: f64,
        labels: HashMap<String, String>,
    ) -> Result<(), CoreError>;

    /// Get a value from shared state
    async fn get_shared_state(
        &self,
        _scope_key: &str,
        _key: &str,
    ) -> Result<Option<serde_json::Value>, CoreError> {
        // Default implementation returns None
        // Concrete implementations will override this to provide actual shared state access
        tracing::debug!("Using default get_shared_state implementation (returns None)");
        Ok(None)
    }

    /// Set a value in shared state
    async fn set_shared_state(
        &self,
        _scope_key: &str,
        _key: &str,
        _value: serde_json::Value,
    ) -> Result<(), CoreError> {
        // Default implementation does nothing
        // Concrete implementations will override this to provide actual shared state access
        tracing::debug!("Using default set_shared_state implementation (no-op)");
        Ok(())
    }

    /// Set a value in shared state with a time-to-live
    async fn set_shared_state_with_ttl(
        &self,
        scope_key: &str,
        key: &str,
        value: serde_json::Value,
        _ttl_ms: u64,
    ) -> Result<(), CoreError> {
        // Default implementation just calls set_shared_state and ignores TTL
        tracing::debug!("Using default set_shared_state_with_ttl implementation (ignores TTL)");
        self.set_shared_state(scope_key, key, value).await
    }

    /// Delete a value from shared state
    async fn delete_shared_state(&self, _scope_key: &str, _key: &str) -> Result<(), CoreError> {
        // Default implementation does nothing
        // Concrete implementations will override this to provide actual shared state access
        tracing::debug!("Using default delete_shared_state implementation (no-op)");
        Ok(())
    }

    /// List all keys in a shared state scope
    async fn list_shared_state_keys(&self, _scope_key: &str) -> Result<Vec<String>, CoreError> {
        // Default implementation returns empty list
        // Concrete implementations will override this to provide actual shared state access
        tracing::debug!("Using default list_shared_state_keys implementation (returns empty list)");
        Ok(Vec::new())
    }
}

/// Represents a timer identifier for scheduled callbacks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TimerId(pub String);

/// Example component that says hello
#[derive(Debug)]
pub struct HelloWorldComponent {
    /// Template string that can include {name} placeholder for personalization
    pub message_template: String,
}

impl HelloWorldComponent {
    /// Create a new HelloWorldComponent
    pub fn new(message_template: String) -> Self {
        Self { message_template }
    }
}

impl ComponentExecutorBase for HelloWorldComponent {
    fn component_type(&self) -> &str {
        "HelloWorld"
    }
}

#[async_trait]
impl ComponentExecutor for HelloWorldComponent {
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
        // Get the name input or use a default
        let name = match api.get_input("name").await {
            Ok(data) => {
                if let Some(name_str) = data.as_value().as_str() {
                    name_str.to_string()
                } else {
                    "World".to_string()
                }
            }
            Err(_) => "World".to_string(),
        };

        // Format the message
        let message = self.message_template.replace("{name}", &name);

        // Log the action
        let _ =
            ComponentRuntimeApi::log(&*api, LogLevel::Info, &format!("Greeting {}", name)).await;

        // Create the output
        let output = DataPacket::new(json!({
            "message": message
        }));

        // Set the output
        if let Err(e) = api.set_output("greeting", output).await {
            return ExecutionResult::Failure(e);
        }

        ExecutionResult::Success
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct MockComponentRuntime {
        inputs: HashMap<String, DataPacket>,
        config: HashMap<String, serde_json::Value>,
        outputs: Arc<Mutex<HashMap<String, DataPacket>>>,
        state: Option<serde_json::Value>,
        log_messages: Arc<Mutex<Vec<(LogLevel, String)>>>,
    }

    impl MockComponentRuntime {
        fn new() -> Self {
            Self {
                inputs: HashMap::new(),
                config: HashMap::new(),
                outputs: Arc::new(Mutex::new(HashMap::new())),
                state: None,
                log_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_input(mut self, name: &str, value: serde_json::Value) -> Self {
            self.inputs
                .insert(name.to_string(), DataPacket::new(value));
            self
        }
        
        fn get_outputs(&self) -> Arc<Mutex<HashMap<String, DataPacket>>> {
            self.outputs.clone()
        }
        
        fn get_log_messages(&self) -> Arc<Mutex<Vec<(LogLevel, String)>>> {
            self.log_messages.clone()
        }
    }

    impl ComponentRuntimeApiBase for MockComponentRuntime {
        fn log(&self, level: tracing::Level, message: &str) {
            // Convert tracing::Level to LogLevel and store the message
            let log_level = match level {
                tracing::Level::ERROR => LogLevel::Error,
                tracing::Level::WARN => LogLevel::Warn,
                tracing::Level::INFO => LogLevel::Info,
                tracing::Level::DEBUG => LogLevel::Debug,
                tracing::Level::TRACE => LogLevel::Trace,
            };
            
            println!("[{:?}] {}", log_level, message);
        }
    }

    #[async_trait]
    impl ComponentRuntimeApi for MockComponentRuntime {
        async fn get_input(&self, name: &str) -> Result<DataPacket, CoreError> {
            self.inputs
                .get(name)
                .cloned()
                .ok_or_else(|| CoreError::IOError(format!("Input not found: {}", name)))
        }

        async fn get_config(&self, name: &str) -> Result<serde_json::Value, CoreError> {
            self.config
                .get(name)
                .cloned()
                .ok_or_else(|| CoreError::ConfigurationError(format!("Config not found: {}", name)))
        }

        async fn set_output(&self, name: &str, data: DataPacket) -> Result<(), CoreError> {
            let mut outputs = self.outputs.lock().await;
            outputs.insert(name.to_string(), data);
            Ok(())
        }

        async fn get_state(&self) -> Result<Option<serde_json::Value>, CoreError> {
            Ok(self.state.clone())
        }

        async fn save_state(&self, _state: serde_json::Value) -> Result<(), CoreError> {
            Ok(())
        }

        async fn schedule_timer(&self, _duration: std::time::Duration) -> Result<(), CoreError> {
            Ok(())
        }

        async fn correlation_id(&self) -> Result<CorrelationId, CoreError> {
            Ok(CorrelationId("test-correlation-id".to_string()))
        }

        async fn log(&self, level: LogLevel, message: &str) -> Result<(), CoreError> {
            let mut logs = self.log_messages.lock().await;
            logs.push((level, message.to_string()));
            println!("[{:?}] {}", level, message);
            Ok(())
        }

        async fn emit_metric(
            &self,
            _name: &str,
            _value: f64,
            _labels: HashMap<String, String>,
        ) -> Result<(), CoreError> {
            Ok(())
        }

        // Default implementations of shared state methods are used
    }

    #[tokio::test]
    async fn test_hello_world_component() {
        // Create a runtime with a name input
        let runtime = Arc::new(
            MockComponentRuntime::new().with_input("name", json!("Test User")),
        );

        // Create the component with template
        let component = HelloWorldComponent::new("Hello, {name}!".to_string());

        // Execute the component
        let result = component.execute(runtime.clone()).await;

        // Verify it was successful
        assert!(matches!(result, ExecutionResult::Success));

        // Check the output
        let outputs_lock = runtime.get_outputs();
        let outputs = outputs_lock.lock().await;
        assert!(outputs.contains_key("greeting"));
        if let Some(greeting) = outputs.get("greeting") {
            assert_eq!(
                greeting.as_value()["message"].as_str().unwrap(),
                "Hello, Test User!"
            );
        }
    }
    
    #[tokio::test]
    async fn test_hello_world_component_default_name() {
        // Create a runtime with no inputs
        let runtime = Arc::new(MockComponentRuntime::new());

        // Create the component with template
        let component = HelloWorldComponent::new("Greetings, {name}!".to_string());

        // Execute the component
        let result = component.execute(runtime.clone()).await;

        // Verify it was successful
        assert!(matches!(result, ExecutionResult::Success));

        // Check the output
        let outputs_lock = runtime.get_outputs();
        let outputs = outputs_lock.lock().await;
        assert!(outputs.contains_key("greeting"));
        if let Some(greeting) = outputs.get("greeting") {
            assert_eq!(
                greeting.as_value()["message"].as_str().unwrap(),
                "Greetings, World!"
            );
        }
    }
    
    #[tokio::test]
    async fn test_hello_world_component_non_string_name() {
        // Create a runtime with a non-string name input
        let runtime = Arc::new(
            MockComponentRuntime::new().with_input("name", json!(42)),
        );

        // Create the component with template
        let component = HelloWorldComponent::new("Hello, {name}!".to_string());

        // Execute the component
        let result = component.execute(runtime.clone()).await;

        // Verify it was successful
        assert!(matches!(result, ExecutionResult::Success));

        // Check the output - should use "World" as default
        let outputs_lock = runtime.get_outputs();
        let outputs = outputs_lock.lock().await;
        assert!(outputs.contains_key("greeting"));
        if let Some(greeting) = outputs.get("greeting") {
            assert_eq!(
                greeting.as_value()["message"].as_str().unwrap(),
                "Hello, World!"
            );
        }
    }
    
    #[tokio::test]
    async fn test_hello_world_component_logs() {
        // Create a runtime
        let runtime = Arc::new(
            MockComponentRuntime::new().with_input("name", json!("Logger")),
        );

        // Create the component with template
        let component = HelloWorldComponent::new("Hello, {name}!".to_string());

        // Execute the component
        let _ = component.execute(runtime.clone()).await;

        // Check that logging occurred
        let log_messages_lock = runtime.get_log_messages();
        let log_messages = log_messages_lock.lock().await;
        assert!(!log_messages.is_empty());
        
        // Verify the exact log message content
        let found_greeting_log = log_messages.iter().any(|(level, msg)| {
            *level == LogLevel::Info && msg.contains("Greeting Logger")
        });
        
        assert!(found_greeting_log, "Expected greeting log message not found");
    }
    
    #[tokio::test]
    async fn test_component_type() {
        let component = HelloWorldComponent::new("Hello, {name}!".to_string());
        assert_eq!(component.component_type(), "HelloWorld");
    }
}

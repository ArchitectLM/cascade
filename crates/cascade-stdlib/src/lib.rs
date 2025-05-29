//! 
//! Standard library of components for the Cascade Platform
//!

use cascade_core::{
    ComponentExecutor, 
    ComponentRuntimeApi, 
    CoreError, 
    ExecutionResult,
    LogLevel,
    ComponentExecutorBase,
};
use cascade_core::types::DataPacket;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

pub mod components;

// Import components that are actually implemented
use crate::components::resilience::{CircuitBreaker, RateLimiter, Bulkhead};
use crate::components::retry::RetryWrapper;
use crate::components::uuid::UuidGenerator;

/// Module containing common components used in many workflows
pub mod common {
    use super::*;
    
    /// A simple NoOp component that just passes its input to output
    #[derive(Debug)]
    pub struct NoOp;
    
    impl NoOp {
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for NoOp {
        fn default() -> Self {
            Self::new()
        }
    }
    
    impl ComponentExecutorBase for NoOp {
        fn component_type(&self) -> &str {
            "NoOp"
        }
    }
    
    #[async_trait]
    impl ComponentExecutor for NoOp {
        async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
            match api.get_input("data").await {
                Ok(data) => {
                    if let Err(e) = api.set_output("result", data).await {
                        return ExecutionResult::Failure(e);
                    }
                    ExecutionResult::Success
                },
                Err(e) => ExecutionResult::Failure(e),
            }
        }
    }
}

/// Module containing String/Text handling components
pub mod text {
    use super::*;
    
    /// A HelloWorld component that greets a person
    #[derive(Debug)]
    pub struct HelloWorld {
        message_template: String,
    }
    
    impl HelloWorld {
        pub fn new(message_template: Option<String>) -> Self {
            Self {
                message_template: message_template.unwrap_or_else(|| "Hello, {name}!".to_string()),
            }
        }
    }
    
    impl ComponentExecutorBase for HelloWorld {
        fn component_type(&self) -> &str {
            "HelloWorld"
        }
    }
    
    #[async_trait]
    impl ComponentExecutor for HelloWorld {
        async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
            // Get the configured message template, or use default
            let template = match api.get_config("message").await {
                Ok(message) => {
                    if let Some(msg) = message.as_str() {
                        msg.to_string()
                    } else {
                        self.message_template.clone()
                    }
                },
                Err(_) => self.message_template.clone(),
            };
            
            // Get the name input or use a default
            let name = match api.get_input("name").await {
                Ok(data) => {
                    if let Some(name) = data.as_value().as_str() {
                        name.to_string()
                    } else {
                        "World".to_string()
                    }
                },
                Err(_) => "World".to_string(),
            };
            
            // Format the message
            let message = template.replace("{name}", &name);
            
            // Set the output
            let output = DataPacket::new(serde_json::json!({ "message": message }));
            if let Err(e) = api.set_output("result", output).await {
                return ExecutionResult::Failure(e);
            }
            
            // Log the action - use the async version from ComponentRuntimeApi trait
            let _ = ComponentRuntimeApi::log(&*api, LogLevel::Info, &format!("Greeted {}", name)).await;
            
            // Optional: emit a metric
            let mut labels = HashMap::new();
            labels.insert("component".to_string(), "HelloWorld".to_string());
            labels.insert("template".to_string(), template);
            let _ = api.emit_metric("greeting_count", 1.0, labels).await;
            
            ExecutionResult::Success
        }
    }
}

/// Factory module for creating Standard Library components
pub mod factory {
    use super::*;
    use crate::common::NoOp;
    use crate::text::HelloWorld;
    
    /// Creates a component instance based on the component type.
    pub fn create_component(component_type: &str) -> Result<Box<dyn ComponentExecutor>, CoreError> {
        match component_type {
            "RetryWrapper" => Ok(Box::new(RetryWrapper::new())),
            "CircuitBreaker" => Ok(Box::new(CircuitBreaker::new())),
            "RateLimiter" => Ok(Box::new(RateLimiter::new())),
            "Bulkhead" => Ok(Box::new(Bulkhead::new())),
            "UuidGenerator" => Ok(Box::new(UuidGenerator::new())),
            "NoOp" => Ok(Box::new(NoOp::new())),
            "HelloWorld" => Ok(Box::new(HelloWorld::new(None))),
            _ => Err(CoreError::ComponentError(format!("Unknown component type: {}", component_type)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factory::create_component;

    #[test]
    fn test_create_component() {
        assert!(create_component("CircuitBreaker").is_ok());
        assert!(create_component("RateLimiter").is_ok());
        assert!(create_component("RetryWrapper").is_ok());
        assert!(create_component("UuidGenerator").is_ok());

        // Unknown component should return an error
        assert!(create_component("Unknown").is_err());
    }
} 
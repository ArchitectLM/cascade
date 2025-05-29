use cascade_core::{
    ComponentExecutor, 
    ComponentExecutorBase,
    ComponentRuntimeApi, 
    CoreError, 
    ExecutionResult,
    ComponentRuntimeApiBase
};
use cascade_core::types::DataPacket;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// A component that publishes messages to Kafka
#[derive(Debug)]
pub struct KafkaProducer;

impl KafkaProducer {
    /// Create a new KafkaProducer component
    pub fn new() -> Self {
        Self
    }
    
    // In a real implementation, this would connect to Kafka
    async fn get_client(&self, _api: &dyn ComponentRuntimeApi) -> Result<(), CoreError> {
        // In a real implementation, this would create or return a cached Kafka client
        Ok(())
    }
}

impl Default for KafkaProducer {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentExecutorBase for KafkaProducer {
    fn component_type(&self) -> &'static str {
        "KafkaProducer"
    }
}

#[async_trait]
impl ComponentExecutor for KafkaProducer {
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
        // Ensure client is available
        if let Err(e) = self.get_client(api.as_ref()).await {
            return ExecutionResult::Failure(e);
        }
        
        // Get topic from config
        let topic = match api.get_config("topic").await {
            Ok(value) => {
                if let Some(topic_str) = value.as_str() {
                    topic_str.to_string()
                } else {
                    return ExecutionResult::Failure(CoreError::ValidationError(
                        "topic must be a string".to_string()
                    ));
                }
            },
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Get message from input
        let message = match api.get_input("message").await {
            Ok(data) => data,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Get key from input (optional)
        let key = match api.get_input("key").await {
            Ok(data) => Some(data.as_value().to_string()),
            Err(_) => None, // Key is optional
        };
        
        // Log message publishing
        ComponentRuntimeApiBase::log(&api, tracing::Level::DEBUG, &format!(
            "Publishing message to Kafka topic {}, key: {:?}, value: {}",
            topic,
            key,
            message.as_value()
        ));
        
        // Create result data
        let result = DataPacket::new(json!({
            "topic": topic,
            "messagePublished": true,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }));
        
        // Set output
        if let Err(e) = api.set_output("result", result).await {
            return ExecutionResult::Failure(e);
        }
        
        // Emit metric
        let mut labels = HashMap::new();
        labels.insert("component".to_string(), "KafkaProducer".to_string());
        labels.insert("topic".to_string(), topic.clone());
        let _ = api.emit_metric("kafka_messages_published", 1.0, labels).await;
        
        ExecutionResult::Success
    }
} 
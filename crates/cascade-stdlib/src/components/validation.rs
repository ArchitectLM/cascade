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
use jsonschema::{JSONSchema, Draft, error::ValidationError};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// A component that validates JSON data against a JSON Schema
#[derive(Debug)]
pub struct JsonSchemaValidator;

impl JsonSchemaValidator {
    /// Create a new JSON Schema validator component
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonSchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentExecutorBase for JsonSchemaValidator {
    fn component_type(&self) -> &'static str {
        "JsonSchemaValidator"
    }
}

#[async_trait]
impl ComponentExecutor for JsonSchemaValidator {
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
        // Get the data to validate
        let data = match api.get_input("data").await {
            Ok(data) => data,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Get the schema from config
        let schema_value = match api.get_config("schema").await {
            Ok(value) => value,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Compile the schema
        let schema = match JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema_value) 
        {
            Ok(schema) => schema,
            Err(e) => {
                return ExecutionResult::Failure(CoreError::ValidationError(
                    format!("Invalid JSON Schema: {}", e)
                ));
            }
        };
        
        // Log validation start
        ComponentRuntimeApiBase::log(&api, tracing::Level::DEBUG, "Validating data against schema");
        
        // Clone the data for validation to avoid borrowing issues
        let data_value = data.as_value().clone();
        
        // Validate the data
        let validation_result = schema.validate(&data_value);
        
        if let Err(errors) = validation_result {
            // Format validation errors
            let validation_errors: Vec<Value> = errors.into_iter()
                .map(|error: ValidationError| {
                    let path = error.instance_path.to_string();
                    
                    json!({
                        "field": path,
                        "message": error.to_string(),
                        "keyword": format!("{:?}", error.kind),
                    })
                })
                .collect();
            
            // Create error output
            let error_output = DataPacket::new(json!({
                "errorCode": "ERR_COMPONENT_INPUT_VALIDATION_FAILED",
                "errorMessage": "Data validation failed",
                "component": {
                    "type": "StdLib:JsonSchemaValidator"
                },
                "details": {
                    "validationErrors": validation_errors
                }
            }));
            
            // Set error output
            if let Err(e) = api.set_output("validationErrors", error_output).await {
                return ExecutionResult::Failure(e);
            }
            
            // Log validation results
            let _ = ComponentRuntimeApiBase::log(&api, tracing::Level::DEBUG, 
                     &format!("Validation failed with {} errors", validation_errors.len()));
            
            // Emit metric
            let mut labels = HashMap::new();
            labels.insert("component".to_string(), "JsonSchemaValidator".to_string());
            labels.insert("result".to_string(), "failed".to_string());
            let _ = api.emit_metric("validation_count", 1.0, labels).await;
        } else {
            // Validation succeeded, pass the data to the valid output
            if let Err(e) = api.set_output("validData", data).await {
                return ExecutionResult::Failure(e);
            }
            
            let _ = ComponentRuntimeApiBase::log(&api, tracing::Level::DEBUG, "Validation succeeded");
            
            // Emit metric
            let mut labels = HashMap::new();
            labels.insert("component".to_string(), "JsonSchemaValidator".to_string());
            labels.insert("result".to_string(), "success".to_string());
            let _ = api.emit_metric("validation_count", 1.0, labels).await;
        }
        
        ExecutionResult::Success
    }
} 
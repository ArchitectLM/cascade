use cascade_core::{
    ComponentExecutor, 
    ComponentExecutorBase,
    ComponentRuntimeApi, 
    ComponentRuntimeApiBase,
    CoreError, 
    ExecutionResult,
};
use cascade_core::types::DataPacket;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value as JsonValue};
use async_trait::async_trait;
use std::sync::Arc;

/// A data mapping component that transforms input data using user-defined expressions
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MapData {
    /// The component type identifier
    pub component_type: String,
}

impl MapData {
    /// Create a new MapData component
    pub fn new() -> Self {
        Self {
            component_type: "MapData".to_string()
        }
    }
}

#[async_trait]
impl ComponentExecutorBase for MapData {
    fn component_type(&self) -> &str {
        "MapData"
    }
}

#[async_trait]
impl ComponentExecutor for MapData {
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
        // Log execution start
        ComponentRuntimeApiBase::log(&api, tracing::Level::DEBUG, "Executing MapData component");
        
        // Get input data
        let input = match api.get_input("data").await {
            Ok(data) => data,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Get mapping configuration
        let mapping = match api.get_config("mapping").await {
            Ok(config) => config,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Get optional language configuration (default to jmespath)
        let language = match api.get_config("language").await {
            Ok(value) => value.as_str().unwrap_or("jmespath").to_string(),
            Err(_) => "jmespath".to_string(),
        };
        
        // Apply the mapping transformation
        let input_data = input.as_value();
        let result = match apply_mapping(input_data, &mapping, &language) {
            Ok(transformed) => transformed,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Set the output
        match api.set_output("result", DataPacket::new(result)).await {
            Ok(_) => ExecutionResult::Success,
            Err(e) => ExecutionResult::Failure(e),
        }
    }
}

/// Apply a mapping expression to transform input data
fn apply_mapping(
    input: &JsonValue,
    mapping_expr: &JsonValue,
    language: &str,
) -> Result<JsonValue, CoreError> {
    match language {
        "jmespath" => {
            if mapping_expr.is_object() {
                // Apply object templating with jmespath
                let mut result = json!({});
                
                for (key, value) in mapping_expr.as_object().unwrap() {
                    if value.is_string() {
                        let value_str = value.as_str().unwrap();
                        if value_str.starts_with("$.") {
                            // This is a jmespath expression, compile and evaluate it
                            if let Some(path) = value_str.strip_prefix("$.") {
                                match jmespath::compile(path) {
                                    Ok(compiled) => {
                                        match compiled.search(input) {
                                            Ok(result_value) => {
                                                // Convert to JsonValue
                                                let json_value = serde_json::to_value(result_value)
                                                    .unwrap_or(JsonValue::Null);
                                                
                                                if let Some(obj) = result.as_object_mut() {
                                                    obj.insert(key.clone(), json_value);
                                                }
                                            },
                                            Err(e) => return Err(CoreError::ExpressionError(
                                                format!("Failed to evaluate JMESPath expression: {}", e)
                                            )),
                                        }
                                    },
                                    Err(e) => return Err(CoreError::ExpressionError(
                                        format!("Failed to compile JMESPath expression: {}", e)
                                    )),
                                }
                            } else {
                                // Should never happen since we checked with starts_with
                                if let Some(obj) = result.as_object_mut() {
                                    obj.insert(key.clone(), value.clone());
                                }
                            }
                        } else {
                            // Just a literal string
                            if let Some(obj) = result.as_object_mut() {
                                obj.insert(key.clone(), value.clone());
                            }
                        }
                    } else {
                        // For non-string values, just copy them directly
                        if let Some(obj) = result.as_object_mut() {
                            obj.insert(key.clone(), value.clone());
                        }
                    }
                }
                
                Ok(result)
            } else if mapping_expr.is_string() {
                // Direct JMESPath expression on the entire input
                let expr = mapping_expr.as_str().unwrap();
                if expr.starts_with("$.") {
                    if let Some(path) = expr.strip_prefix("$.") {
                        match jmespath::compile(path) {
                            Ok(compiled) => {
                                match compiled.search(input) {
                                    Ok(result_value) => {
                                        // Convert to JsonValue
                                        Ok(serde_json::to_value(result_value).unwrap_or(JsonValue::Null))
                                    },
                                    Err(e) => Err(CoreError::ExpressionError(
                                        format!("Failed to evaluate JMESPath expression: {}", e)
                                    )),
                                }
                            },
                            Err(e) => Err(CoreError::ExpressionError(
                                format!("Failed to compile JMESPath expression: {}", e)
                            )),
                        }
                    } else {
                        // Should never happen since we checked with starts_with
                        Ok(JsonValue::String(expr.to_string()))
                    }
                } else {
                    // Just a literal string
                    Ok(JsonValue::String(expr.to_string()))
                }
            } else {
                // For any other type of mapping expression, return it as-is
                Ok(mapping_expr.clone())
            }
        },
        _ => Err(CoreError::ValidationError(
            format!("Unsupported mapping language: {}", language)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_apply_mapping_with_object() {
        let input = json!({
            "user": {
                "name": "John Doe",
                "email": "john@example.com",
                "age": 30
            },
            "order": {
                "id": "12345",
                "items": ["item1", "item2"]
            }
        });
        
        let mapping = json!({
            "userName": "$.user.name",
            "userEmail": "$.user.email",
            "orderId": "$.order.id",
            "static": "This is a static value"
        });
        
        let result = apply_mapping(&input, &mapping, "jmespath").unwrap();
        
        assert_eq!(result["userName"], "John Doe");
        assert_eq!(result["userEmail"], "john@example.com");
        assert_eq!(result["orderId"], "12345");
        assert_eq!(result["static"], "This is a static value");
    }
    
    #[test]
    fn test_apply_mapping_with_direct_expression() {
        let input = json!({
            "items": [
                {"id": 1, "name": "Item 1"},
                {"id": 2, "name": "Item 2"},
                {"id": 3, "name": "Item 3"}
            ]
        });
        
        let mapping = json!("$.items[*].name");
        
        let result = apply_mapping(&input, &mapping, "jmespath").unwrap();
        
        assert!(result.is_array());
        let array = result.as_array().unwrap();
        assert_eq!(array.len(), 3);
        assert_eq!(array[0], "Item 1");
        assert_eq!(array[1], "Item 2");
        assert_eq!(array[2], "Item 3");
    }
} 
use cascade_core::{
    ComponentExecutor, 
    ComponentExecutorBase,
    ComponentRuntimeApi, 
    CoreError,
    ExecutionResult,
    ComponentRuntimeApiBase
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;

/// A conditional branching component that can direct flows based on conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Switch {
    /// The component type identifier
    pub component_type: String,
}

impl Switch {
    /// Create a new Switch component
    pub fn new() -> Self {
        Self {
            component_type: "Switch".to_string()
        }
    }
}

impl Default for Switch {
    fn default() -> Self {
        Self::new()
    }
}

/// A case definition for the Switch component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchCase {
    /// The condition expression that determines if this case matches
    pub condition_expression: String,
    /// The output port to route the data to if this case matches
    pub output_port: String,
    /// The language used for the condition expression (default: "jmespath")
    pub language: String,
}

/// Default implementation of expression language
pub fn default_expression_language() -> String {
    "jmespath".to_string()
}

/// Evaluates condition expressions
#[allow(dead_code)]
fn evaluate_condition(
    expression: &str, 
    language: &str, 
    data: &JsonValue
) -> Result<bool, CoreError> {
    match language {
        "jexl" => {
            let context = json!({
                "data": data,
            });
            let evaluator = jexl_eval::Evaluator::new();
            match evaluator.eval_in_context(expression, context) {
                Ok(result) => match result {
                    JsonValue::Bool(b) => Ok(b),
                    _ => Ok(!result.is_null() && result != false),
                },
                Err(e) => Err(CoreError::ExpressionError(format!("Failed to evaluate jexl expression: {}", e))),
            }
        },
        "jmespath" => {
            // Existing jmespath implementation
            let keys: Vec<&str> = expression.split('.').collect();
            let mut current = data;
            
            for key in keys {
                if let Some(next) = current.get(key) {
                    current = next;
                } else {
                    return Ok(false); // Path doesn't exist
                }
            }
            
            // Check if the result is truthy
            Ok(!current.is_null() && 
               (!current.is_boolean() || current.as_bool().unwrap_or(false)))
        },
        _ => Err(CoreError::ValidationError(
            format!("Unsupported expression language: {}", language)
        )),
    }
}

#[async_trait]
impl ComponentExecutorBase for Switch {
    fn component_type(&self) -> &str {
        "Switch"
    }
}

#[async_trait]
impl ComponentExecutor for Switch {
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
        // Get the input data
        let input = match api.get_input("data").await {
            Ok(data) => data,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Get the cases configuration
        let case_config = match api.get_config("cases").await {
            Ok(cases) => cases,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Parse the cases
        let cases = match parse_switch_cases(&case_config) {
            Ok(cases) => cases,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Get default output (optional)
        let default_output = match api.get_config("default_output").await {
            Ok(value) => {
                if value.is_string() {
                    Some(value.as_str().unwrap().to_string())
                } else {
                    None
                }
            },
            Err(_) => None,
        };
        
        // Find the first matching case
        for case in cases {
            let data_value = input.as_value();
            let context = json!({
                "data": data_value,
            });
            
            // Using jexl-eval crate properly with Evaluator
            let condition_result = {
                let evaluator = jexl_eval::Evaluator::new();
                match evaluator.eval_in_context(&case.condition_expression, context) {
                    Ok(result) => {
                        match result {
                            JsonValue::Bool(b) => Ok(b),
                            _ => Ok(false),
                        }
                    },
                    Err(e) => Err(CoreError::ExpressionError(format!("Failed to evaluate condition: {}", e))),
                }
            };
            
            match condition_result {
                Ok(true) => {
                    // Condition matched - route to specified output
                    match api.set_output(&case.output_port, input.clone()).await {
                        Ok(_) => {
                            // Log the match
                            ComponentRuntimeApiBase::log(&api, tracing::Level::DEBUG, &format!(
                                "Switch condition matched: '{}', routing to '{}'",
                                case.condition_expression, case.output_port
                            ));
                            return ExecutionResult::Success;
                        },
                        Err(e) => {
                            ComponentRuntimeApiBase::log(&api, tracing::Level::ERROR, &format!(
                                "Failed to set output for port '{}': {}", 
                                case.output_port, e
                            ));
                            return ExecutionResult::Failure(e);
                        }
                    }
                },
                Ok(false) => {
                    // No match, continue to next case
                    continue;
                },
                Err(e) => {
                    // Error evaluating condition
                    ComponentRuntimeApiBase::log(&api, tracing::Level::ERROR, &format!(
                        "Error evaluating condition '{}': {}", 
                        case.condition_expression, e
                    ));
                    return ExecutionResult::Failure(e);
                }
            }
        }
        
        // No matching case found, use default output if specified
        if let Some(default_port) = default_output {
            match api.set_output(&default_port, input).await {
                Ok(_) => {
                    ComponentRuntimeApiBase::log(&api, tracing::Level::DEBUG, 
                        "No switch conditions matched, using default output");
                    ExecutionResult::Success
                },
                Err(e) => ExecutionResult::Failure(e),
            }
        } else {
            // No default output specified
            ComponentRuntimeApiBase::log(&api, tracing::Level::DEBUG, 
                "No switch conditions matched and no default output specified");
            ExecutionResult::Success
        }
    }
}

/// Filter component that passes data only if condition evaluates to true
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterData {
    /// The component type identifier
    pub component_type: String,
}

impl FilterData {
    /// Create a new FilterData component
    pub fn new() -> Self {
        Self {
            component_type: "FilterData".to_string()
        }
    }
}

impl Default for FilterData {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ComponentExecutorBase for FilterData {
    fn component_type(&self) -> &str {
        "FilterData"
    }
}

#[async_trait]
impl ComponentExecutor for FilterData {
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
        // Get the input data
        let input = match api.get_input("data").await {
            Ok(data) => data,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Get the condition expression
        let condition = match api.get_config("condition").await {
            Ok(value) => {
                match value.as_str() {
                    Some(condition_str) => condition_str.to_string(),
                    None => return ExecutionResult::Failure(CoreError::ConfigurationError(
                        "Condition must be a string".to_string()
                    )),
                }
            },
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Evaluate the condition
        let data_value = input.as_value();
        let context = json!({
            "data": data_value,
        });
        
        // Using jexl-eval crate properly with Evaluator
        let eval_result = {
            let evaluator = jexl_eval::Evaluator::new();
            match evaluator.eval_in_context(&condition, context) {
                Ok(result) => match result {
                    JsonValue::Bool(b) => Ok(b),
                    _ => Ok(false),
                },
                Err(e) => Err(CoreError::ExpressionError(format!("Failed to evaluate condition: {}", e))),
            }
        };
        
        match eval_result {
            Ok(true) => {
                // Condition is true, pass the data through
                match api.set_output("filtered", input).await {
                    Ok(_) => {
                        ComponentRuntimeApiBase::log(&api, tracing::Level::DEBUG, 
                            &format!("Condition '{}' evaluated to true, passing data", condition));
                        ExecutionResult::Success
                    },
                    Err(e) => ExecutionResult::Failure(e),
                }
            },
            Ok(false) => {
                // Condition is false, filter the data out
                ComponentRuntimeApiBase::log(&api, tracing::Level::DEBUG, 
                    &format!("Condition '{}' evaluated to false, filtering data", condition));
                ExecutionResult::Success
            },
            Err(e) => {
                // Error evaluating condition
                ComponentRuntimeApiBase::log(&api, tracing::Level::ERROR, 
                    &format!("Error evaluating condition '{}': {}", condition, e));
                ExecutionResult::Failure(e)
            }
        }
    }
}

/// Parses SwitchCase configurations from JSON
fn parse_switch_cases(cases_config: &JsonValue) -> Result<Vec<SwitchCase>, CoreError> {
    let mut parsed_cases = Vec::new();
    
    if let Some(cases_array) = cases_config.as_array() {
        for case in cases_array {
            if let (Some(condition), Some(output)) = (
                case.get("condition").and_then(|c| c.as_str()),
                case.get("output_port").and_then(|o| o.as_str())
            ) {
                // Get language if specified, default to jmespath
                let language = case.get("language")
                    .and_then(|l| l.as_str())
                    .unwrap_or("jmespath")
                    .to_string();
                
                parsed_cases.push(SwitchCase {
                    condition_expression: condition.to_string(),
                    output_port: output.to_string(),
                    language,
                });
            } else {
                return Err(CoreError::ConfigurationError(
                    "Switch cases must have 'condition' and 'output_port' fields".to_string()
                ));
            }
        }
    } else {
        return Err(CoreError::ConfigurationError(
            "Switch 'cases' config must be an array".to_string()
        ));
    }
    
    Ok(parsed_cases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_default_expression_language() {
        assert_eq!(default_expression_language(), "jmespath");
    }
    
    #[test]
    fn test_evaluate_simple_condition() {
        let data = json!({
            "user": {
                "name": "John",
                "active": true
            }
        });
        
        // Test simple property access
        let result = evaluate_condition("user.name", "jmespath", &data).unwrap();
        assert!(result);
        
        // Test boolean property
        let result = evaluate_condition("user.active", "jmespath", &data).unwrap();
        assert!(result);
        
        // Test non-existent property
        let result = evaluate_condition("user.missing", "jmespath", &data).unwrap();
        assert!(!result);
    }
} 
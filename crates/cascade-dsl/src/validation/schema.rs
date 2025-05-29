use std::collections::HashMap;
use jsonschema::JSONSchema;
use serde_json::Value;
use crate::flow::{ParsedDocument, ComponentDefinition, SchemaReference};
use crate::validation::{ValidationError, error_codes, Validator};

/// Validates schemas in the DSL document:
/// - Validates schema references are valid
/// - Validates config against schemas defined for component types
pub struct SchemaValidator {
    /// Registry of known schemas by schema_ref
    schema_registry: HashMap<String, Value>,
}

impl SchemaValidator {
    /// Create a new schema validator with default schema registry
    pub fn new() -> Self {
        let mut schema_registry = HashMap::with_capacity(10);
        
        // Add built-in schemas with improved validation rules
        schema_registry.insert(
            "schema:string".to_string(),
            serde_json::json!({ "type": "string" })
        );
        
        schema_registry.insert(
            "schema:number".to_string(),
            serde_json::json!({ "type": "number" })
        );
        
        schema_registry.insert(
            "schema:integer".to_string(),
            serde_json::json!({ "type": "integer" })
        );
        
        schema_registry.insert(
            "schema:boolean".to_string(),
            serde_json::json!({ "type": "boolean" })
        );
        
        schema_registry.insert(
            "schema:object".to_string(),
            serde_json::json!({ "type": "object" })
        );
        
        schema_registry.insert(
            "schema:array".to_string(),
            serde_json::json!({ "type": "array" })
        );
        
        schema_registry.insert(
            "schema:any".to_string(),
            serde_json::json!({})
        );
        
        schema_registry.insert(
            "schema:void".to_string(),
            serde_json::json!({ "type": "null" })
        );
        
        schema_registry.insert(
            "schema:error".to_string(),
            serde_json::json!({
                "type": "object",
                "required": ["code", "message"],
                "properties": {
                    "code": { "type": "string" },
                    "message": { "type": "string" },
                    "details": { "type": "object" }
                },
                "additionalProperties": true
            })
        );
        
        SchemaValidator { schema_registry }
    }
    
    /// Register a custom schema with the validator
    ///
    /// Adds a new schema to the registry, making it available for validation
    #[allow(dead_code)]
    pub fn register_schema(&mut self, schema_ref: String, schema: Value) {
        self.schema_registry.insert(schema_ref, schema);
    }
    
    /// Get the list of available schema references
    fn get_available_schema_refs(&self) -> Vec<String> {
        self.schema_registry.keys()
            .map(|k| k.to_string())
            .collect()
    }
    
    /// Validate that all schema references are valid
    ///
    /// Checks that:
    /// 1. Schema URIs reference a known schema
    /// 2. Inline schemas are valid JSON Schema
    fn validate_schema_references(
        &self,
        component: &ComponentDefinition,
        path: &str
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        
        // Get available schemas for error messages
        let available_schemas = self.get_available_schema_refs()
            .into_iter()
            .map(|s| format!("'{}'", s))
            .collect::<Vec<_>>()
            .join(", ");
        
        // Validate input schema references
        for (idx, input) in component.inputs.iter().enumerate() {
            if let Some(ref schema_ref) = input.schema_ref {
                match schema_ref {
                    SchemaReference::Uri(uri) => {
                        if !self.schema_registry.contains_key(uri) {
                            errors.push(ValidationError {
                                code: error_codes::INVALID_SCHEMA,
                                message: format!(
                                    "Unknown schema reference: '{}' for input '{}' in component '{}'. Available schemas: {}", 
                                    uri, input.name, component.name, available_schemas
                                ),
                                path: Some(format!("{}.inputs[{}].schema_ref", path, idx)),
                            });
                        }
                    },
                    SchemaReference::Inline(schema) => {
                        // Validate the inline schema is valid JSON Schema
                        if let Err(e) = self.validate_json_schema(schema) {
                            errors.push(ValidationError {
                                code: error_codes::INVALID_SCHEMA,
                                message: format!(
                                    "Invalid JSON Schema for input '{}' in component '{}': {}", 
                                    input.name, component.name, e
                                ),
                                path: Some(format!("{}.inputs[{}].schema_ref", path, idx)),
                            });
                        }
                    }
                }
            }
        }
        
        // Validate output schema references
        for (idx, output) in component.outputs.iter().enumerate() {
            if let Some(ref schema_ref) = output.schema_ref {
                match schema_ref {
                    SchemaReference::Uri(uri) => {
                        if !self.schema_registry.contains_key(uri) {
                            errors.push(ValidationError {
                                code: error_codes::INVALID_SCHEMA,
                                message: format!(
                                    "Unknown schema reference: '{}' for output '{}' in component '{}'. Available schemas: {}", 
                                    uri, output.name, component.name, available_schemas
                                ),
                                path: Some(format!("{}.outputs[{}].schema_ref", path, idx)),
                            });
                        }
                    },
                    SchemaReference::Inline(schema) => {
                        // Validate the inline schema is valid JSON Schema
                        if let Err(e) = self.validate_json_schema(schema) {
                            errors.push(ValidationError {
                                code: error_codes::INVALID_SCHEMA,
                                message: format!(
                                    "Invalid JSON Schema for output '{}' in component '{}': {}", 
                                    output.name, component.name, e
                                ),
                                path: Some(format!("{}.outputs[{}].schema_ref", path, idx)),
                            });
                        }
                    }
                }
            }
        }
        
        // Validate state schema reference if present
        if let Some(ref schema_ref) = component.state_schema {
            match schema_ref {
                SchemaReference::Uri(uri) => {
                    if !self.schema_registry.contains_key(uri) {
                        errors.push(ValidationError {
                            code: error_codes::INVALID_SCHEMA,
                            message: format!(
                                "Unknown schema reference: '{}' for state schema in component '{}'. Available schemas: {}", 
                                uri, component.name, available_schemas
                            ),
                            path: Some(format!("{}.state_schema", path)),
                        });
                    }
                },
                SchemaReference::Inline(schema) => {
                    // Validate the inline schema is valid JSON Schema
                    if let Err(e) = self.validate_json_schema(schema) {
                        errors.push(ValidationError {
                            code: error_codes::INVALID_SCHEMA,
                            message: format!(
                                "Invalid JSON Schema for state schema in component '{}': {}", 
                                component.name, e
                            ),
                            path: Some(format!("{}.state_schema", path)),
                        });
                    }
                }
            }
        }
        
        errors
    }
    
    /// Helper method to validate a JSON Schema
    fn validate_json_schema(&self, schema: &Value) -> Result<(), String> {
        match JSONSchema::compile(schema) {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
    
    /// Validate shared state scope and resilience configuration
    fn validate_resilience_config(
        &self,
        component: &ComponentDefinition,
        path: &str
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        
        // Validate shared state scope format if present
        if let Some(ref scope) = component.shared_state_scope {
            if scope.is_empty() {
                errors.push(ValidationError {
                    code: error_codes::INVALID_CONFIG,
                    message: format!(
                        "Shared state scope cannot be empty for component '{}'", 
                        component.name
                    ),
                    path: Some(format!("{}.shared_state_scope", path)),
                });
            }
            
            if scope.contains(' ') || scope.contains('\n') || scope.contains('\t') {
                errors.push(ValidationError {
                    code: error_codes::INVALID_CONFIG,
                    message: format!(
                        "Shared state scope cannot contain whitespace for component '{}'", 
                        component.name
                    ),
                    path: Some(format!("{}.shared_state_scope", path)),
                });
            }
        }
        
        // Validate resilience configuration if present
        if let Some(ref resilience) = component.resilience {
            // Validate retry configuration
            if resilience.retry_enabled {
                if resilience.retry_attempts == 0 {
                    errors.push(ValidationError {
                        code: error_codes::INVALID_CONFIG,
                        message: format!(
                            "Retry attempts must be greater than 0 for component '{}'", 
                            component.name
                        ),
                        path: Some(format!("{}.resilience.retry_attempts", path)),
                    });
                }
                
                if resilience.retry_delay_ms == 0 {
                    errors.push(ValidationError {
                        code: error_codes::INVALID_CONFIG,
                        message: format!(
                            "Retry delay must be greater than 0 for component '{}'", 
                            component.name
                        ),
                        path: Some(format!("{}.resilience.retry_delay_ms", path)),
                    });
                }
                
                if resilience.backoff_multiplier <= 0.0 {
                    errors.push(ValidationError {
                        code: error_codes::INVALID_CONFIG,
                        message: format!(
                            "Backoff multiplier must be greater than 0 for component '{}'", 
                            component.name
                        ),
                        path: Some(format!("{}.resilience.backoff_multiplier", path)),
                    });
                }
            }
            
            // Validate circuit breaker configuration
            if resilience.circuit_breaker_enabled {
                if resilience.failure_threshold == 0 {
                    errors.push(ValidationError {
                        code: error_codes::INVALID_CONFIG,
                        message: format!(
                            "Failure threshold must be greater than 0 for component '{}'", 
                            component.name
                        ),
                        path: Some(format!("{}.resilience.failure_threshold", path)),
                    });
                }
                
                if resilience.reset_timeout_ms == 0 {
                    errors.push(ValidationError {
                        code: error_codes::INVALID_CONFIG,
                        message: format!(
                            "Reset timeout must be greater than 0 for component '{}'", 
                            component.name
                        ),
                        path: Some(format!("{}.resilience.reset_timeout_ms", path)),
                    });
                }
            }
            
            // Validate rate limiter configuration
            if resilience.rate_limiter_enabled {
                if resilience.max_requests == 0 {
                    errors.push(ValidationError {
                        code: error_codes::INVALID_CONFIG,
                        message: format!(
                            "Max requests must be greater than 0 for component '{}'", 
                            component.name
                        ),
                        path: Some(format!("{}.resilience.max_requests", path)),
                    });
                }
                
                if resilience.window_ms == 0 {
                    errors.push(ValidationError {
                        code: error_codes::INVALID_CONFIG,
                        message: format!(
                            "Window must be greater than 0 for component '{}'", 
                            component.name
                        ),
                        path: Some(format!("{}.resilience.window_ms", path)),
                    });
                }
            }
        }
        
        errors
    }
}

impl Validator for SchemaValidator {
    fn validate(&self, document: &ParsedDocument) -> Vec<ValidationError> {
        let mut errors = Vec::with_capacity(document.definitions.components.len());
        
        // Validate component schemas
        for (comp_idx, component) in document.definitions.components.iter().enumerate() {
            let comp_path = format!("definitions.components[{}]", comp_idx);
            
            // Validate schema references
            errors.extend(self.validate_schema_references(component, &comp_path));
            
            // Validate resilience configuration
            errors.extend(self.validate_resilience_config(component, &comp_path));
        }
        
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::{InputDefinition, OutputDefinition};
    
    #[test]
    fn test_validate_valid_schema_references() {
        let validator = SchemaValidator::new();
        
        // Create a component with valid schema references
        let component = ComponentDefinition {
            name: "test-component".to_string(),
            component_type: "test-type".to_string(),
            description: None,
            config: HashMap::new(),
            inputs: vec![
                InputDefinition {
                    name: "input1".to_string(),
                    description: None,
                    schema_ref: Some(SchemaReference::Uri("schema:string".to_string())),
                    is_required: true,
                },
                InputDefinition {
                    name: "input2".to_string(),
                    description: None,
                    schema_ref: Some(SchemaReference::Inline(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" }
                        }
                    }))),
                    is_required: true,
                }
            ],
            outputs: vec![
                OutputDefinition {
                    name: "output1".to_string(),
                    description: None,
                    schema_ref: Some(SchemaReference::Uri("schema:object".to_string())),
                    is_error_path: false,
                }
            ],
            state_schema: None,
            shared_state_scope: None,
            resilience: None,
            metadata: HashMap::new(),
        };
        
        let errors = validator.validate_schema_references(&component, "test.path");
        assert!(errors.is_empty(), "Should not find errors with valid schema references");
    }
    
    #[test]
    fn test_validate_invalid_schema_references() {
        let validator = SchemaValidator::new();
        
        // Create a component with an invalid schema reference
        let component = ComponentDefinition {
            name: "test-component".to_string(),
            component_type: "test-type".to_string(),
            description: None,
            config: HashMap::new(),
            inputs: vec![
                InputDefinition {
                    name: "input1".to_string(),
                    description: None,
                    schema_ref: Some(SchemaReference::Uri("schema:unknown".to_string())),
                    is_required: true,
                }
            ],
            outputs: vec![],
            state_schema: None,
            shared_state_scope: None,
            resilience: None,
            metadata: HashMap::new(),
        };
        
        let errors = validator.validate_schema_references(&component, "test.path");
        assert!(!errors.is_empty(), "Should find errors with invalid schema references");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, error_codes::INVALID_SCHEMA);
        assert!(errors[0].message.contains("Unknown schema reference"));
        
        // Test with multiple invalid references
        let component_multi_invalid = ComponentDefinition {
            name: "test-component".to_string(),
            component_type: "test-type".to_string(),
            description: None,
            config: HashMap::new(),
            inputs: vec![
                InputDefinition {
                    name: "input1".to_string(),
                    description: None,
                    schema_ref: Some(SchemaReference::Uri("schema:unknown1".to_string())),
                    is_required: true,
                },
                InputDefinition {
                    name: "input2".to_string(),
                    description: None,
                    schema_ref: Some(SchemaReference::Uri("schema:unknown2".to_string())),
                    is_required: true,
                },
            ],
            outputs: vec![
                OutputDefinition {
                    name: "output1".to_string(),
                    description: None,
                    schema_ref: Some(SchemaReference::Uri("schema:unknown3".to_string())),
                    is_error_path: false,
                }
            ],
            state_schema: Some(SchemaReference::Uri("schema:unknown4".to_string())),
            shared_state_scope: None,
            resilience: None,
            metadata: HashMap::new(),
        };
        
        let errors = validator.validate_schema_references(&component_multi_invalid, "test.path");
        assert_eq!(errors.len(), 4, "Should find all invalid schema references");
    }
    
    #[test]
    fn test_validate_invalid_json_schema() {
        let validator = SchemaValidator::new();
        
        // Create a component with an invalid JSON Schema
        let component = ComponentDefinition {
            name: "test-component".to_string(),
            component_type: "test-type".to_string(),
            description: None,
            config: HashMap::new(),
            inputs: vec![
                InputDefinition {
                    name: "input1".to_string(),
                    description: None,
                    schema_ref: Some(SchemaReference::Inline(serde_json::json!({
                        "type": ["not-a-valid-type"]
                    }))),
                    is_required: true,
                }
            ],
            outputs: vec![],
            state_schema: None,
            shared_state_scope: None,
            resilience: None,
            metadata: HashMap::new(),
        };
        
        let errors = validator.validate_schema_references(&component, "test.path");
        assert!(!errors.is_empty(), "Should find errors with invalid JSON Schema");
        assert_eq!(errors[0].code, error_codes::INVALID_SCHEMA);
        assert!(errors[0].message.contains("Invalid JSON Schema"));
        
        // Test with a completely broken schema that's not even valid JSON
        let component_invalid_schema = ComponentDefinition {
            name: "test-component".to_string(),
            component_type: "test-type".to_string(),
            description: None,
            config: HashMap::new(),
            inputs: vec![
                InputDefinition {
                    name: "broken_schema".to_string(),
                    description: None,
                    schema_ref: Some(SchemaReference::Inline(serde_json::json!({
                        "type": {},
                        "required": 123,
                        "properties": "not-an-object"
                    }))),
                    is_required: true,
                }
            ],
            outputs: vec![],
            state_schema: None,
            shared_state_scope: None,
            resilience: None,
            metadata: HashMap::new(),
        };
        
        let errors = validator.validate_schema_references(&component_invalid_schema, "test.path");
        assert!(!errors.is_empty(), "Should find errors with broken JSON Schema");
    }
    
    #[test]
    fn test_validate_empty_component() {
        let validator = SchemaValidator::new();
        
        // Create a component with no schema references
        let component = ComponentDefinition {
            name: "empty-component".to_string(),
            component_type: "test-type".to_string(),
            description: None,
            config: HashMap::new(),
            inputs: vec![
                InputDefinition {
                    name: "input_no_schema".to_string(),
                    description: None,
                    schema_ref: None, // No schema reference
                    is_required: true,
                }
            ],
            outputs: vec![
                OutputDefinition {
                    name: "output_no_schema".to_string(),
                    description: None,
                    schema_ref: None, // No schema reference
                    is_error_path: false,
                }
            ],
            state_schema: None,
            shared_state_scope: None,
            resilience: None,
            metadata: HashMap::new(),
        };
        
        let errors = validator.validate_schema_references(&component, "test.path");
        assert!(errors.is_empty(), "Component with no schema references should be valid");
    }
    
    #[test]
    fn test_json_schema_validation() {
        let validator = SchemaValidator::new();
        
        // Valid schema
        let valid_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer", "minimum": 0 }
            },
            "required": ["name"]
        });
        
        assert!(validator.validate_json_schema(&valid_schema).is_ok(),
            "Valid JSON Schema should pass validation");
        
        // Invalid schema - wrong type format
        let invalid_schema1 = serde_json::json!({
            "type": "not-a-valid-type"
        });
        
        assert!(validator.validate_json_schema(&invalid_schema1).is_err(),
            "Invalid type should fail validation");
        
        // Invalid schema - malformed required array
        let invalid_schema2 = serde_json::json!({
            "type": "object",
            "required": "not-an-array"
        });
        
        assert!(validator.validate_json_schema(&invalid_schema2).is_err(),
            "Invalid required format should fail validation");
    }
} 
// This is an empty file

use cascade_dsl::flow::ComponentDefinition;
use cascade_dsl::validation::ValidationError;
use cascade_dsl::DslError;
use std::collections::HashMap;

// Create a simplified interface for working with schema validation
fn validate_document(document: &cascade_dsl::flow::ParsedDocument) -> Vec<ValidationError> {
    // Use the public API to validate the document and extract validation errors
    match cascade_dsl::validation::validate_document(document) {
        Ok(_) => Vec::new(), // No errors
        Err(err) => match err {
            DslError::ValidationError(e) => vec![e],
            DslError::MultipleValidationErrors(errors) => errors,
            _ => Vec::new(), // Other error types
        }
    }
}

#[test]
fn test_validate_shared_state_scope() {
    // Create a test component with valid shared state scope
    let component = create_test_component_with_scope("valid-scope");
    
    // Parse into a document
    let document = create_test_document(vec![component]);
    
    // Validate
    let errors = validate_document(&document);
    assert!(errors.is_empty(), "Valid scope should have no errors");
    
    // Test with invalid shared state scope (with whitespace)
    let component = create_test_component_with_scope("invalid scope");
    let document = create_test_document(vec![component]);
    let errors = validate_document(&document);
    assert!(!errors.is_empty(), "Invalid scope should have errors");
    assert!(errors.iter().any(|e| e.message.contains("whitespace")));
    
    // Test with empty shared state scope
    let component = create_test_component_with_scope("");
    let document = create_test_document(vec![component]);
    let errors = validate_document(&document);
    assert!(!errors.is_empty(), "Empty scope should have errors");
    assert!(errors.iter().any(|e| e.message.contains("cannot be empty")));
}

// Helper function to create a test component with shared state scope
fn create_test_component_with_scope(scope: &str) -> ComponentDefinition {
    ComponentDefinition {
        name: "test-component".to_string(),
        component_type: "test-type".to_string(),
        description: None,
        config: HashMap::new(),
        inputs: vec![],
        outputs: vec![],
        state_schema: None,
        shared_state_scope: Some(scope.to_string()),
        resilience: None,
        metadata: HashMap::new(),
    }
}

// Helper function to create a test document with components
fn create_test_document(components: Vec<ComponentDefinition>) -> cascade_dsl::flow::ParsedDocument {
    cascade_dsl::flow::ParsedDocument {
        dsl_version: "1.0".to_string(),
        definitions: cascade_dsl::flow::Definitions {
            components,
            flows: vec![],
        },
    }
}

use std::fmt;
use std::error::Error;
use crate::flow::ParsedDocument;
use crate::error::DslError;

mod flow_validator;
mod reference;
mod schema;

/// Represents a validation error that occurred during DSL processing
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Error code (should be a constant identifier)
    pub code: &'static str,
    
    /// Human-readable error message
    pub message: String,
    
    /// Optional path to the location of the error (e.g., "flows[0].steps[2]")
    pub path: Option<String>,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.path {
            write!(f, "{}: {} (at {})", self.code, self.message, path)
        } else {
            write!(f, "{}: {}", self.code, self.message)
        }
    }
}

impl Error for ValidationError {}

/// Validation error codes
pub mod error_codes {
    /// Invalid reference (e.g., component, step, etc.)
    pub const INVALID_REFERENCE: &str = "ERR_DSL_VALIDATION_INVALID_REFERENCE";
    
    /// Duplicate ID found
    pub const DUPLICATE_ID: &str = "ERR_DSL_VALIDATION_DUPLICATE_ID";
    
    /// Circular dependency detected
    pub const CIRCULAR_DEPENDENCY: &str = "ERR_DSL_VALIDATION_CIRCULAR_DEPENDENCY";
    
    /// Invalid schema
    pub const INVALID_SCHEMA: &str = "ERR_DSL_VALIDATION_INVALID_SCHEMA";
    
    /// Invalid DSL version
    pub const INVALID_VERSION: &str = "ERR_DSL_VALIDATION_INVALID_VERSION";
    
    /// Missing required field
    pub const MISSING_REQUIRED_FIELD: &str = "ERR_DSL_VALIDATION_MISSING_REQUIRED_FIELD";
    
    /// Invalid input mapping
    pub const INVALID_INPUT_MAPPING: &str = "ERR_DSL_VALIDATION_INVALID_INPUT_MAPPING";
    
    /// Invalid configuration value
    pub const INVALID_CONFIG: &str = "ERR_DSL_VALIDATION_INVALID_CONFIG";
}

/// A trait for validators that check specific aspects of the DSL document
pub trait Validator {
    /// Validate the document and return a list of validation errors (if any)
    fn validate(&self, document: &ParsedDocument) -> Vec<ValidationError>;
}

/// Validate a parsed DSL document
pub fn validate_document(document: &ParsedDocument) -> Result<(), DslError> {
    // Get a list of all validators
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(flow_validator::FlowValidator::new()),
        Box::new(reference::ReferenceValidator::new()),
        Box::new(schema::SchemaValidator::new()),
    ];
    
    // Run all validators and collect errors
    let mut errors = Vec::new();
    
    for validator in validators {
        let validator_errors = validator.validate(document);
        errors.extend(validator_errors);
    }
    
    // If there are errors, return them
    if !errors.is_empty() {
        return Err(DslError::from_validation_errors(errors));
    }
    
    Ok(())
} 
use thiserror::Error;
use crate::validation::ValidationError;
use std::fmt;

/// All possible errors that can occur in the DSL processing
#[derive(Error, Debug)]
pub enum DslError {
    /// Errors that occur during YAML parsing
    #[error("YAML parsing error: {0}")]
    YamlError(#[from] serde_yaml::Error),
    
    /// Errors that occur during JSON processing
    #[error("JSON processing error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    /// A single validation error
    #[error("Validation error: {0}")]
    ValidationError(#[from] ValidationError),
    
    /// Multiple validation errors
    #[error("{}", MultipleErrorsFormat(.0))]
    MultipleValidationErrors(Vec<ValidationError>),
    
    /// Unsupported DSL version
    #[error("Unsupported DSL version: {0}")]
    UnsupportedVersion(String),
    
    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingRequiredField(String),
    
    /// Internal error
    #[error("Internal error: {0}")]
    InternalError(String),
}

// Helper struct to format multiple errors
struct MultipleErrorsFormat<'a>(&'a [ValidationError]);

impl fmt::Display for MultipleErrorsFormat<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Multiple validation errors ({} issues):", self.0.len())?;
        for (i, err) in self.0.iter().enumerate() {
            write!(f, "\n  {}. {}", i + 1, err)?;
        }
        Ok(())
    }
}

impl DslError {
    /// Create a DslError from a validation error or a vector of validation errors
    pub fn from_validation_errors(errors: Vec<ValidationError>) -> Self {
        match errors.len() {
            0 => DslError::InternalError("Called from_validation_errors with empty vector".to_string()),
            1 => DslError::ValidationError(errors.into_iter().next().unwrap()),
            _ => DslError::MultipleValidationErrors(errors),
        }
    }
    
    /// Get the error code for this error
    pub fn error_code(&self) -> &'static str {
        match self {
            DslError::YamlError(_) => "ERR_DSL_YAML_PARSE",
            DslError::JsonError(_) => "ERR_DSL_JSON_PARSE",
            DslError::ValidationError(err) => err.code,
            DslError::MultipleValidationErrors(_) => "ERR_DSL_VALIDATION_MULTIPLE",
            DslError::UnsupportedVersion(_) => "ERR_DSL_UNSUPPORTED_VERSION",
            DslError::MissingRequiredField(_) => "ERR_DSL_MISSING_FIELD",
            DslError::InternalError(_) => "ERR_DSL_INTERNAL",
        }
    }
} 
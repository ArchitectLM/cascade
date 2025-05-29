use thiserror::Error;

/// Core error type for the Cascade runtime
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// Flow instance not found
    #[error("Flow instance not found: {0}")]
    FlowInstanceNotFound(String),

    /// Flow definition not found
    #[error("Flow definition not found: {0}")]
    FlowDefinitionNotFound(String),

    /// Component not found
    #[error("Component not found: {0}")]
    ComponentNotFoundError(String),

    /// Step execution error
    #[error("Step execution error: {0}")]
    StepExecutionError(String),

    /// Validation error
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// State store error
    #[error("State store error: {0}")]
    StateStoreError(String),

    /// Timer error
    #[error("Timer error: {0}")]
    TimerError(String),

    /// Condition evaluation error
    #[error("Condition evaluation error: {0}")]
    ConditionEvaluationError(String),

    /// Flow trigger error
    #[error("Flow trigger error: {0}")]
    FlowTriggerError(String),

    /// Input/output error
    #[error("Input/output error: {0}")]
    IOError(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Expression evaluation error
    #[error("Expression evaluation error: {0}")]
    ExpressionError(String),

    /// External dependency error
    #[error("External dependency error: {0}")]
    ExternalDependencyError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Component error
    #[error("Component error: {0}")]
    ComponentError(String),

    /// Reference error
    #[error("Reference error: {0}")]
    ReferenceError(String),

    /// Timer service error
    #[error("Timer service error: {0}")]
    TimerServiceError(String),

    /// Flow execution error
    #[error("Flow execution error: {0}")]
    FlowExecutionError(String),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<serde_json::Error> for CoreError {
    fn from(err: serde_json::Error) -> Self {
        CoreError::SerializationError(err.to_string())
    }
}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        CoreError::IOError(err.to_string())
    }
}

impl From<String> for CoreError {
    fn from(err: String) -> Self {
        CoreError::Other(err)
    }
}

impl From<&str> for CoreError {
    fn from(err: &str) -> Self {
        CoreError::Other(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error as IoError, ErrorKind};

    #[test]
    fn test_error_display() {
        let errors = vec![
            (CoreError::FlowInstanceNotFound("instance1".to_string()), "Flow instance not found: instance1"),
            (CoreError::FlowDefinitionNotFound("flow1".to_string()), "Flow definition not found: flow1"),
            (CoreError::ComponentNotFoundError("comp1".to_string()), "Component not found: comp1"),
            (CoreError::StepExecutionError("step_err".to_string()), "Step execution error: step_err"),
            (CoreError::ValidationError("invalid".to_string()), "Validation error: invalid"),
            (CoreError::StateStoreError("db_err".to_string()), "State store error: db_err"),
            (CoreError::TimerError("timeout".to_string()), "Timer error: timeout"),
            (CoreError::ConditionEvaluationError("syntax".to_string()), "Condition evaluation error: syntax"),
            (CoreError::FlowTriggerError("trigger".to_string()), "Flow trigger error: trigger"),
            (CoreError::IOError("io_err".to_string()), "Input/output error: io_err"),
            (CoreError::SerializationError("ser_err".to_string()), "Serialization error: ser_err"),
            (CoreError::ExpressionError("expr_err".to_string()), "Expression evaluation error: expr_err"),
            (CoreError::ExternalDependencyError("ext_err".to_string()), "External dependency error: ext_err"),
            (CoreError::ConfigurationError("config_err".to_string()), "Configuration error: config_err"),
            (CoreError::ComponentError("comp_err".to_string()), "Component error: comp_err"),
            (CoreError::ReferenceError("ref_err".to_string()), "Reference error: ref_err"),
            (CoreError::TimerServiceError("timer_svc_err".to_string()), "Timer service error: timer_svc_err"),
            (CoreError::FlowExecutionError("flow_exec_err".to_string()), "Flow execution error: flow_exec_err"),
            (CoreError::Other("other_err".to_string()), "other_err"),
        ];

        for (error, expected_msg) in errors {
            assert_eq!(error.to_string(), expected_msg);
        }
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let error: CoreError = json_error.into();
        
        match error {
            CoreError::SerializationError(msg) => {
                assert!(msg.contains("expected value"));
            }
            _ => panic!("Expected SerializationError variant"),
        }
    }

    #[test]
    fn test_from_io_error() {
        let io_error = IoError::new(ErrorKind::NotFound, "file not found");
        let error: CoreError = io_error.into();
        
        match error {
            CoreError::IOError(msg) => {
                assert!(msg.contains("file not found"));
            }
            _ => panic!("Expected IOError variant"),
        }
    }

    #[test]
    fn test_from_string() {
        let string_error = "test error message".to_string();
        let error: CoreError = string_error.into();
        
        match error {
            CoreError::Other(msg) => {
                assert_eq!(msg, "test error message");
            }
            _ => panic!("Expected Other variant"),
        }
    }

    #[test]
    fn test_from_str() {
        let str_error = "test error message";
        let error: CoreError = str_error.into();
        
        match error {
            CoreError::Other(msg) => {
                assert_eq!(msg, "test error message");
            }
            _ => panic!("Expected Other variant"),
        }
    }

    #[test]
    fn test_error_clone_and_eq() {
        let original = CoreError::ValidationError("test".to_string());
        let cloned = original.clone();
        
        assert_eq!(original, cloned);
        assert_eq!(format!("{:?}", original), format!("{:?}", cloned));
    }
}

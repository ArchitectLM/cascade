//! Assertion utilities for validating flow instance states.

use crate::mocks::core_runtime::{FlowInstance, FlowState};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

/// Error type for flow state validation failures
#[derive(Debug, Error)]
pub enum FlowStateValidationError {
    #[error("Invalid flow state: expected {expected}, got {actual}")]
    InvalidState { expected: String, actual: String },
    
    #[error("Missing data key: {0}")]
    MissingDataKey(String),
    
    #[error("Invalid data value: expected {expected}, got {actual}")]
    InvalidDataValue { expected: String, actual: String },
    
    #[error("Flow instance ID mismatch: expected {expected}, got {actual}")]
    InstanceIdMismatch { expected: String, actual: String },
    
    #[error("Flow ID mismatch: expected {expected}, got {actual}")]
    FlowIdMismatch { expected: String, actual: String },
    
    #[error("Flow validation error: {0}")]
    Other(String),
}

/// Asserts that a flow instance has the expected state.
///
/// # Arguments
///
/// * `instance` - The flow instance to validate
/// * `expected_state` - The expected flow state
///
/// # Returns
///
/// * `Ok(())` - If the flow instance has the expected state
/// * `Err(FlowStateValidationError)` - If the flow instance does not have the expected state
pub fn assert_flow_state(
    instance: &FlowInstance,
    expected_state: &FlowState,
) -> Result<(), FlowStateValidationError> {
    // Simply verify the instance has the expected state using string representation
    let expected_str = format!("{:?}", expected_state);
    let actual_str = format!("{:?}", &instance.state);
    
    if expected_str != actual_str {
        return Err(FlowStateValidationError::InvalidState {
            expected: expected_str,
            actual: actual_str,
        });
    }
    
    Ok(())
}

/// Asserts that a flow instance contains the expected data key with the expected value.
///
/// # Arguments
///
/// * `instance` - The flow instance to validate
/// * `key` - The expected data key
/// * `expected_value` - The expected value (as a JSON value)
///
/// # Returns
///
/// * `Ok(())` - If the flow instance contains the expected data key with the expected value
/// * `Err(FlowStateValidationError)` - If the flow instance does not contain the expected data
pub fn assert_flow_data_contains(
    instance: &FlowInstance,
    key: &str,
    expected_value: Value,
) -> Result<(), FlowStateValidationError> {
    let value = instance
        .data
        .get(key)
        .ok_or_else(|| FlowStateValidationError::MissingDataKey(key.to_string()))?;
    
    if value != &expected_value {
        return Err(FlowStateValidationError::InvalidDataValue {
            expected: expected_value.to_string(),
            actual: value.to_string(),
        });
    }
    
    Ok(())
}

/// Asserts that a flow instance contains all the expected data key-value pairs.
///
/// # Arguments
///
/// * `instance` - The flow instance to validate
/// * `expected_data` - The expected data key-value pairs
///
/// # Returns
///
/// * `Ok(())` - If the flow instance contains all the expected data
/// * `Err(FlowStateValidationError)` - If the flow instance does not contain the expected data
pub fn assert_flow_data_matches(
    instance: &FlowInstance,
    expected_data: HashMap<String, Value>,
) -> Result<(), FlowStateValidationError> {
    for (key, expected_value) in expected_data {
        assert_flow_data_contains(instance, &key, expected_value)?;
    }
    
    Ok(())
}

/// Asserts that a flow instance has the expected flow ID.
///
/// # Arguments
///
/// * `instance` - The flow instance to validate
/// * `expected_flow_id` - The expected flow ID
///
/// # Returns
///
/// * `Ok(())` - If the flow instance has the expected flow ID
/// * `Err(FlowStateValidationError)` - If the flow instance does not have the expected flow ID
pub fn assert_flow_id(
    instance: &FlowInstance,
    expected_flow_id: &str,
) -> Result<(), FlowStateValidationError> {
    if instance.flow_id != expected_flow_id {
        return Err(FlowStateValidationError::FlowIdMismatch {
            expected: expected_flow_id.to_string(),
            actual: instance.flow_id.clone(),
        });
    }
    
    Ok(())
}

/// Asserts that a flow instance has the expected instance ID.
///
/// # Arguments
///
/// * `instance` - The flow instance to validate
/// * `expected_instance_id` - The expected instance ID
///
/// # Returns
///
/// * `Ok(())` - If the flow instance has the expected instance ID
/// * `Err(FlowStateValidationError)` - If the flow instance does not have the expected instance ID
pub fn assert_instance_id(
    instance: &FlowInstance,
    expected_instance_id: &str,
) -> Result<(), FlowStateValidationError> {
    if instance.instance_id != expected_instance_id {
        return Err(FlowStateValidationError::InstanceIdMismatch {
            expected: expected_instance_id.to_string(),
            actual: instance.instance_id.clone(),
        });
    }
    
    Ok(())
}

/// Asserts that a flow instance matches the expected flow ID, state, and data.
///
/// # Arguments
///
/// * `instance` - The flow instance to validate
/// * `expected_flow_id` - The expected flow ID
/// * `expected_state` - The expected flow state
/// * `expected_data` - The expected flow data (if None, no data validation is performed)
///
/// # Returns
///
/// * `Ok(())` - If the flow instance matches the expected flow ID, state, and data
/// * `Err(FlowStateValidationError)` - If the flow instance does not match the expected values
pub fn assert_flow_instance_matches(
    instance: &FlowInstance,
    expected_flow_id: &str,
    expected_state: &FlowState,
    expected_data: Option<HashMap<String, Value>>,
) -> Result<(), FlowStateValidationError> {
    // Check flow ID
    if instance.flow_id != expected_flow_id {
        return Err(FlowStateValidationError::FlowIdMismatch {
            expected: expected_flow_id.to_string(),
            actual: instance.flow_id.clone(),
        });
    }
    
    // Check state using string representation since FlowState doesn't implement PartialEq
    let expected_state_str = format!("{:?}", expected_state);
    let actual_state_str = format!("{:?}", &instance.state);
    
    if expected_state_str != actual_state_str {
        return Err(FlowStateValidationError::InvalidState {
            expected: expected_state_str,
            actual: actual_state_str,
        });
    }
    
    // Check data if provided
    if let Some(expected_data) = expected_data {
        for (key, expected_value) in expected_data {
            match instance.data.get(&key) {
                Some(actual_value) => {
                    if actual_value != &expected_value {
                        return Err(FlowStateValidationError::InvalidDataValue {
                            expected: expected_value.to_string(),
                            actual: actual_value.to_string(),
                        });
                    }
                }
                None => {
                    return Err(FlowStateValidationError::MissingDataKey(key));
                }
            }
        }
    }
    
    Ok(())
}

/// Asserts that a flow instance is in a completed state with the expected output data.
///
/// # Arguments
///
/// * `instance` - The flow instance to validate
/// * `expected_output` - The expected output data
///
/// # Returns
///
/// * `Ok(())` - If the flow instance is completed with the expected output
/// * `Err(FlowStateValidationError)` - If the flow instance is not completed or does not have the expected output
pub fn assert_flow_completed_with_output(
    instance: &FlowInstance,
    expected_output: Value,
) -> Result<(), FlowStateValidationError> {
    assert_flow_state(instance, &FlowState::Completed)?;
    assert_flow_data_contains(instance, "output", expected_output)?;
    
    Ok(())
}

/// Asserts that a flow instance is in a failed state with the expected error message.
///
/// # Arguments
///
/// * `instance` - The flow instance to validate
/// * `expected_error_contains` - A substring that should be present in the error message
///
/// # Returns
///
/// * `Ok(())` - If the flow instance is failed with the expected error
/// * `Err(FlowStateValidationError)` - If the flow instance is not failed or does not have the expected error
pub fn assert_flow_failed_with_error(
    instance: &FlowInstance,
    expected_error_contains: &str,
) -> Result<(), FlowStateValidationError> {
    match &instance.state {
        FlowState::Failed(error) => {
            if !error.contains(expected_error_contains) {
                return Err(FlowStateValidationError::InvalidDataValue {
                    expected: expected_error_contains.to_string(),
                    actual: error.clone(),
                });
            }
        }
        _ => {
            return Err(FlowStateValidationError::InvalidState {
                expected: "Failed".to_string(),
                actual: format!("{:?}", instance.state),
            });
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mocks::core_runtime::{FlowInstance, FlowState};
    use std::collections::HashMap;
    use serde_json::json;
    
    fn create_test_instance() -> FlowInstance {
        let mut data = HashMap::new();
        data.insert("test_key".to_string(), json!("test_value"));
        data.insert("output".to_string(), json!({"result": "success"}));
        
        FlowInstance {
            instance_id: "test-instance".to_string(),
            flow_id: "test-flow".to_string(),
            state: FlowState::Completed,
            data,
        }
    }
    
    #[test]
    fn test_assert_flow_state() {
        let instance = create_test_instance();
        
        assert!(assert_flow_state(&instance, &FlowState::Completed).is_ok());
        
        // Use a different instance to test the error case
        let mut running_instance = create_test_instance();
        running_instance.state = FlowState::Running;
        assert!(assert_flow_state(&running_instance, &FlowState::Failed("error".to_string())).is_err());
    }
    
    #[test]
    fn test_assert_flow_data_contains() {
        let instance = create_test_instance();
        
        assert!(assert_flow_data_contains(&instance, "test_key", json!("test_value")).is_ok());
        assert!(assert_flow_data_contains(&instance, "missing_key", json!("value")).is_err());
        assert!(assert_flow_data_contains(&instance, "test_key", json!("wrong_value")).is_err());
    }
    
    #[test]
    fn test_assert_flow_data_matches() {
        let instance = create_test_instance();
        
        let mut expected_data = HashMap::new();
        expected_data.insert("test_key".to_string(), json!("test_value"));
        
        assert!(assert_flow_data_matches(&instance, expected_data).is_ok());
        
        let mut wrong_data = HashMap::new();
        wrong_data.insert("test_key".to_string(), json!("wrong_value"));
        
        assert!(assert_flow_data_matches(&instance, wrong_data).is_err());
    }
    
    #[test]
    fn test_assert_flow_id() {
        let instance = create_test_instance();
        
        assert!(assert_flow_id(&instance, "test-flow").is_ok());
        assert!(assert_flow_id(&instance, "wrong-flow").is_err());
    }
    
    #[test]
    fn test_assert_instance_id() {
        let instance = create_test_instance();
        
        assert!(assert_instance_id(&instance, "test-instance").is_ok());
        assert!(assert_instance_id(&instance, "wrong-instance").is_err());
    }
    
    #[test]
    fn test_assert_flow_instance_matches() {
        let instance = create_test_instance();
        
        let mut expected_data = HashMap::new();
        expected_data.insert("test_key".to_string(), json!("test_value"));
        
        assert!(assert_flow_instance_matches(
            &instance,
            "test-flow",
            &FlowState::Completed,
            Some(expected_data)
        ).is_ok());
        
        assert!(assert_flow_instance_matches(
            &instance,
            "wrong-flow",
            &FlowState::Completed,
            None
        ).is_err());
        
        // Test with a different instance state
        let mut running_instance = create_test_instance();
        running_instance.state = FlowState::Running;
        
        assert!(assert_flow_instance_matches(
            &running_instance,
            "test-flow",
            &FlowState::Failed("error".to_string()),
            None
        ).is_err());
    }
    
    #[test]
    fn test_assert_flow_completed_with_output() {
        let instance = create_test_instance();
        
        assert!(assert_flow_completed_with_output(&instance, json!({"result": "success"})).is_ok());
        assert!(assert_flow_completed_with_output(&instance, json!({"result": "failure"})).is_err());
        
        // Clear the output to test error case
        let mut failed_instance = create_test_instance();
        failed_instance.state = FlowState::Failed("Test error".to_string());
        failed_instance.data.remove("output");
        
        assert!(assert_flow_completed_with_output(&failed_instance, json!({"result": "success"})).is_err());
    }
    
    #[test]
    fn test_assert_flow_failed_with_error() {
        let mut instance = create_test_instance();
        instance.state = FlowState::Failed("Test error message".to_string());
        
        assert!(assert_flow_failed_with_error(&instance, "Test error").is_ok());
        assert!(assert_flow_failed_with_error(&instance, "wrong error").is_err());
        
        let completed_instance = create_test_instance();
        assert!(assert_flow_failed_with_error(&completed_instance, "Test error").is_err());
    }
} 
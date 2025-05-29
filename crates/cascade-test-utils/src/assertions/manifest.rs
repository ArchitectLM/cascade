//! Assertion utilities for validating Cascade manifest structures.

use serde_json::Value;

/// Error type for manifest validation failures
#[derive(Debug, thiserror::Error)]
pub enum ManifestValidationError {
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Invalid field value: {0} (expected: {1})")]
    InvalidValue(String, String),
    #[error("Component not found: {0}")]
    ComponentNotFound(String),
    #[error("Step not found: {0}")]
    StepNotFound(String),
    #[error("Invalid manifest structure: {0}")]
    InvalidStructure(String),
}

/// Asserts that a manifest has the expected flow ID.
///
/// # Arguments
///
/// * `manifest` - The manifest to validate
/// * `expected_flow_id` - The expected flow ID
///
/// # Returns
///
/// * `Ok(())` - If the manifest has the expected flow ID
/// * `Err(ManifestValidationError)` - If the manifest does not have the expected flow ID
pub fn assert_manifest_has_flow_id(
    manifest: &Value,
    expected_flow_id: &str,
) -> Result<(), ManifestValidationError> {
    let flow_id = manifest
        .get("flowId")
        .ok_or_else(|| ManifestValidationError::MissingField("flowId".to_string()))?
        .as_str()
        .ok_or_else(|| {
            ManifestValidationError::InvalidValue(
                "flowId".to_string(),
                "string".to_string(),
            )
        })?;

    if flow_id != expected_flow_id {
        return Err(ManifestValidationError::InvalidValue(
            "flowId".to_string(),
            expected_flow_id.to_string(),
        ));
    }

    Ok(())
}

/// Asserts that a manifest has the expected component type.
///
/// # Arguments
///
/// * `manifest` - The manifest to validate
/// * `component_type` - The expected component type
///
/// # Returns
///
/// * `Ok(())` - If the manifest has the expected component type
/// * `Err(ManifestValidationError)` - If the manifest does not have the expected component type
pub fn assert_manifest_has_component_type(
    manifest: &Value,
    component_type: &str,
) -> Result<(), ManifestValidationError> {
    let components = manifest
        .get("components")
        .ok_or_else(|| ManifestValidationError::MissingField("components".to_string()))?
        .as_array()
        .ok_or_else(|| {
            ManifestValidationError::InvalidValue(
                "components".to_string(),
                "array".to_string(),
            )
        })?;

    let component_exists = components.iter().any(|component| {
        component
            .get("type")
            .and_then(|t| t.as_str())
            .map(|t| t == component_type)
            .unwrap_or(false)
    });

    if !component_exists {
        return Err(ManifestValidationError::ComponentNotFound(
            component_type.to_string(),
        ));
    }

    Ok(())
}

/// Asserts that a manifest has a component with the specified ID.
///
/// # Arguments
///
/// * `manifest` - The manifest to validate
/// * `component_id` - The expected component ID
///
/// # Returns
///
/// * `Ok(())` - If the manifest has a component with the specified ID
/// * `Err(ManifestValidationError)` - If the manifest does not have a component with the specified ID
pub fn assert_manifest_has_component_id(
    manifest: &Value,
    component_id: &str,
) -> Result<(), ManifestValidationError> {
    let components = manifest
        .get("components")
        .ok_or_else(|| ManifestValidationError::MissingField("components".to_string()))?
        .as_array()
        .ok_or_else(|| {
            ManifestValidationError::InvalidValue(
                "components".to_string(),
                "array".to_string(),
            )
        })?;

    let component_exists = components.iter().any(|component| {
        component
            .get("id")
            .and_then(|id| id.as_str())
            .map(|id| id == component_id)
            .unwrap_or(false)
    });

    if !component_exists {
        return Err(ManifestValidationError::ComponentNotFound(
            component_id.to_string(),
        ));
    }

    Ok(())
}

/// Asserts that a manifest has a step with the specified ID.
///
/// # Arguments
///
/// * `manifest` - The manifest to validate
/// * `step_id` - The expected step ID
///
/// # Returns
///
/// * `Ok(())` - If the manifest has a step with the specified ID
/// * `Err(ManifestValidationError)` - If the manifest does not have a step with the specified ID
pub fn assert_manifest_has_step_id(
    manifest: &Value,
    step_id: &str,
) -> Result<(), ManifestValidationError> {
    let steps = manifest
        .get("steps")
        .ok_or_else(|| ManifestValidationError::MissingField("steps".to_string()))?
        .as_array()
        .ok_or_else(|| {
            ManifestValidationError::InvalidValue("steps".to_string(), "array".to_string())
        })?;

    let step_exists = steps.iter().any(|step| {
        step.get("id")
            .and_then(|id| id.as_str())
            .map(|id| id == step_id)
            .unwrap_or(false)
    });

    if !step_exists {
        return Err(ManifestValidationError::StepNotFound(step_id.to_string()));
    }

    Ok(())
}

/// Asserts that a manifest has a step with the specified component type.
///
/// # Arguments
///
/// * `manifest` - The manifest to validate
/// * `step_id` - The step ID to check
/// * `component_type` - The expected component type
///
/// # Returns
///
/// * `Ok(())` - If the manifest has a step with the specified component type
/// * `Err(ManifestValidationError)` - If the manifest does not have a step with the specified component type
pub fn assert_step_has_component_type(
    manifest: &Value,
    step_id: &str,
    component_type: &str,
) -> Result<(), ManifestValidationError> {
    let steps = manifest
        .get("steps")
        .ok_or_else(|| ManifestValidationError::MissingField("steps".to_string()))?
        .as_array()
        .ok_or_else(|| {
            ManifestValidationError::InvalidValue("steps".to_string(), "array".to_string())
        })?;

    let step = steps
        .iter()
        .find(|step| {
            step.get("id")
                .and_then(|id| id.as_str())
                .map(|id| id == step_id)
                .unwrap_or(false)
        })
        .ok_or_else(|| ManifestValidationError::StepNotFound(step_id.to_string()))?;

    let component_id = step
        .get("component")
        .ok_or_else(|| ManifestValidationError::MissingField("component".to_string()))?
        .as_str()
        .ok_or_else(|| {
            ManifestValidationError::InvalidValue(
                "component".to_string(),
                "string".to_string(),
            )
        })?;

    let components = manifest
        .get("components")
        .ok_or_else(|| ManifestValidationError::MissingField("components".to_string()))?
        .as_array()
        .ok_or_else(|| {
            ManifestValidationError::InvalidValue(
                "components".to_string(),
                "array".to_string(),
            )
        })?;

    let component = components
        .iter()
        .find(|c| {
            c.get("id")
                .and_then(|id| id.as_str())
                .map(|id| id == component_id)
                .unwrap_or(false)
        })
        .ok_or_else(|| ManifestValidationError::ComponentNotFound(component_id.to_string()))?;

    let actual_type = component
        .get("type")
        .ok_or_else(|| ManifestValidationError::MissingField("type".to_string()))?
        .as_str()
        .ok_or_else(|| {
            ManifestValidationError::InvalidValue("type".to_string(), "string".to_string())
        })?;

    if actual_type != component_type {
        return Err(ManifestValidationError::InvalidValue(
            "component type".to_string(),
            component_type.to_string(),
        ));
    }

    Ok(())
}

/// Asserts that a manifest has the expected deployment location for a step.
///
/// # Arguments
///
/// * `manifest` - The manifest to validate
/// * `step_id` - The step ID to check
/// * `expected_location` - The expected deployment location
///
/// # Returns
///
/// * `Ok(())` - If the manifest has the expected deployment location for the step
/// * `Err(ManifestValidationError)` - If the manifest does not have the expected deployment location
pub fn assert_step_has_deployment_location(
    manifest: &Value,
    step_id: &str,
    expected_location: &str,
) -> Result<(), ManifestValidationError> {
    let steps = manifest
        .get("steps")
        .ok_or_else(|| ManifestValidationError::MissingField("steps".to_string()))?
        .as_array()
        .ok_or_else(|| {
            ManifestValidationError::InvalidValue("steps".to_string(), "array".to_string())
        })?;

    let step = steps
        .iter()
        .find(|step| {
            step.get("id")
                .and_then(|id| id.as_str())
                .map(|id| id == step_id)
                .unwrap_or(false)
        })
        .ok_or_else(|| ManifestValidationError::StepNotFound(step_id.to_string()))?;

    let location = step
        .get("deployment_location")
        .ok_or_else(|| ManifestValidationError::MissingField("deployment_location".to_string()))?
        .as_str()
        .ok_or_else(|| {
            ManifestValidationError::InvalidValue(
                "deployment_location".to_string(),
                "string".to_string(),
            )
        })?;

    if location != expected_location {
        return Err(ManifestValidationError::InvalidValue(
            "deployment_location".to_string(),
            expected_location.to_string(),
        ));
    }

    Ok(())
}

/// Validates a manifest against a set of assertions.
///
/// # Arguments
///
/// * `manifest` - The manifest to validate
/// * `assertions` - A vector of assertion functions
///
/// # Returns
///
/// * `Ok(())` - If all assertions pass
/// * `Err(ManifestValidationError)` - If any assertion fails
pub fn validate_manifest<F>(
    manifest: &Value,
    assertions: Vec<F>,
) -> Result<(), ManifestValidationError>
where
    F: FnOnce(&Value) -> Result<(), ManifestValidationError>,
{
    for assertion in assertions {
        assertion(manifest)?;
    }
    Ok(())
}

/// Assertion utility for more readable tests
#[macro_export]
macro_rules! assert_manifest_valid {
    ($manifest:expr, $($assertion:expr),+ $(,)?) => {
        {
            let manifest = $manifest;
            let assertions: Vec<Box<dyn FnOnce(&serde_json::Value) -> Result<(), $crate::assertions::manifest::ManifestValidationError>>> = vec![
                $(Box::new(|m| $assertion(m))),+
            ];
            for assertion in assertions {
                assertion(&manifest)?;
            }
            Ok::<(), $crate::assertions::manifest::ManifestValidationError>(())
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_assert_manifest_has_flow_id() {
        let manifest = json!({
            "flowId": "test-flow",
            "name": "Test Flow",
            "steps": {}
        });
        
        assert_manifest_has_flow_id(&manifest, "test-flow").unwrap();
        
        let result = assert_manifest_has_flow_id(&manifest, "wrong-id");
        assert!(result.is_err());
    }

    #[test]
    fn test_assert_manifest_has_component_type() {
        let manifest = json!({
            "flow_id": "test-flow",
            "components": [
                {
                    "id": "comp1",
                    "type": "HttpCall"
                }
            ],
            "steps": []
        });

        assert!(assert_manifest_has_component_type(&manifest, "HttpCall").is_ok());
        assert!(assert_manifest_has_component_type(&manifest, "MapData").is_err());
    }

    #[test]
    fn test_assert_manifest_has_component_id() {
        let manifest = json!({
            "flow_id": "test-flow",
            "components": [
                {
                    "id": "comp1",
                    "type": "HttpCall"
                }
            ],
            "steps": []
        });

        assert!(assert_manifest_has_component_id(&manifest, "comp1").is_ok());
        assert!(assert_manifest_has_component_id(&manifest, "comp2").is_err());
    }

    #[test]
    fn test_assert_manifest_has_step_id() {
        let manifest = json!({
            "flow_id": "test-flow",
            "components": [],
            "steps": [
                {
                    "id": "step1",
                    "component": "comp1"
                }
            ]
        });

        assert!(assert_manifest_has_step_id(&manifest, "step1").is_ok());
        assert!(assert_manifest_has_step_id(&manifest, "step2").is_err());
    }

    #[test]
    fn test_assert_step_has_component_type() {
        let manifest = json!({
            "flow_id": "test-flow",
            "components": [
                {
                    "id": "comp1",
                    "type": "HttpCall"
                }
            ],
            "steps": [
                {
                    "id": "step1",
                    "component": "comp1"
                }
            ]
        });

        assert!(assert_step_has_component_type(&manifest, "step1", "HttpCall").is_ok());
        assert!(assert_step_has_component_type(&manifest, "step1", "MapData").is_err());
        assert!(assert_step_has_component_type(&manifest, "step2", "HttpCall").is_err());
    }

    #[test]
    fn test_assert_step_has_deployment_location() {
        let manifest = json!({
            "flow_id": "test-flow",
            "components": [],
            "steps": [
                {
                    "id": "step1",
                    "component": "comp1",
                    "deployment_location": "edge"
                }
            ]
        });

        assert!(assert_step_has_deployment_location(&manifest, "step1", "edge").is_ok());
        assert!(assert_step_has_deployment_location(&manifest, "step1", "server").is_err());
    }

    #[test]
    fn test_validate_manifest() {
        let manifest = json!({
            "flowId": "test-flow",
            "components": [
                {
                    "id": "comp1",
                    "type": "HttpCall"
                }
            ],
            "steps": [
                {
                    "id": "step1",
                    "component": "comp1",
                    "deployment_location": "edge"
                }
            ]
        });

        let assertions = vec![
            |m: &Value| assert_manifest_has_flow_id(m, "test-flow"),
            |m: &Value| assert_manifest_has_component_type(m, "HttpCall"),
            |m: &Value| assert_manifest_has_step_id(m, "step1"),
            |m: &Value| assert_step_has_deployment_location(m, "step1", "edge"),
        ];

        assert!(validate_manifest(&manifest, assertions).is_ok());
    }

    #[test]
    fn test_assert_manifest_valid_macro() -> Result<(), Box<dyn std::error::Error>> {
        let manifest = json!({
            "flowId": "test-flow",
            "name": "Test Flow",
            "components": [
                {
                    "id": "comp1",
                    "type": "HttpCall"
                }
            ],
            "steps": [
                {
                    "id": "step1",
                    "component": "comp1",
                    "deployment_location": "edge"
                }
            ]
        });
        
        let result = assert_manifest_valid!(
            manifest,
            |m| assert_manifest_has_flow_id(m, "test-flow"),
            |m| assert_manifest_has_component_type(m, "HttpCall"),
            |m| assert_manifest_has_step_id(m, "step1"),
            |m| assert_step_has_deployment_location(m, "step1", "edge"),
        );
        
        assert!(result.is_ok());
        Ok(())
    }
} 
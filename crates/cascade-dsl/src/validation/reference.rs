use std::collections::HashSet;
use crate::flow::{ParsedDocument, FlowDefinition};
use crate::validation::{ValidationError, error_codes, Validator};
use crate::utils::reference::{is_valid_data_reference_format, extract_step_id};

/// Validates references in the DSL document:
/// - Component references in steps
/// - Step dependencies (run_after)
/// - Data references in inputs_map
pub struct ReferenceValidator;

impl ReferenceValidator {
    /// Create a new reference validator
    pub fn new() -> Self {
        ReferenceValidator
    }
    
    /// Validate component references in steps
    ///
    /// Checks if each step's component_ref refers to a component that exists.
    fn validate_component_references(
        &self,
        flow: &FlowDefinition,
        component_names: &HashSet<&str>,
        path: &str
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        
        for (step_idx, step) in flow.steps.iter().enumerate() {
            if !component_names.contains(step.component_ref.as_str()) {
                errors.push(ValidationError {
                    code: error_codes::INVALID_REFERENCE,
                    message: format!(
                        "Component reference '{}' not found in step '{}'. Available components: {}", 
                        step.component_ref, 
                        step.step_id,
                        component_names.iter().map(|c| format!("'{}'", c)).collect::<Vec<_>>().join(", ")
                    ),
                    path: Some(format!("{}.steps[{}].component_ref", path, step_idx)),
                });
            }
        }
        
        errors
    }
    
    /// Validate step references in run_after
    ///
    /// Checks if each step's run_after refers to steps that exist.
    fn validate_step_references(
        &self,
        flow: &FlowDefinition,
        step_ids: &HashSet<&str>,
        path: &str
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        
        for (step_idx, step) in flow.steps.iter().enumerate() {
            for dep_id in &step.run_after {
                if !step_ids.contains(dep_id.as_str()) {
                    errors.push(ValidationError {
                        code: error_codes::INVALID_REFERENCE,
                        message: format!(
                            "Step '{}' depends on non-existent step ID '{}'. Available steps: {}", 
                            step.step_id, 
                            dep_id,
                            step_ids.iter().map(|s| format!("'{}'", s)).collect::<Vec<_>>().join(", ")
                        ),
                        path: Some(format!("{}.steps[{}].run_after", path, step_idx)),
                    });
                }
            }
        }
        
        errors
    }
    
    /// Validate data references in inputs_map
    ///
    /// Checks both the format of data references and that referenced step IDs exist.
    fn validate_data_references(
        &self,
        flow: &FlowDefinition,
        step_ids: &HashSet<&str>,
        path: &str
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        
        for (step_idx, step) in flow.steps.iter().enumerate() {
            for (input_name, reference) in &step.inputs_map {
                // Check reference format
                if !is_valid_data_reference_format(reference) {
                    errors.push(ValidationError {
                        code: error_codes::INVALID_REFERENCE,
                        message: format!(
                            "Invalid data reference format: '{}'. Must be either 'trigger.<field>' or 'steps.<step_id>.outputs.<output>'", 
                            reference
                        ),
                        path: Some(format!("{}.steps[{}].inputs_map.{}", path, step_idx, input_name)),
                    });
                    continue;
                }
                
                // If it's a step reference, check that the step exists
                if let Some(referenced_step_id) = extract_step_id(reference) {
                    if !step_ids.contains(referenced_step_id) {
                        errors.push(ValidationError {
                            code: error_codes::INVALID_REFERENCE,
                            message: format!(
                                "Data reference '{}' in step '{}' refers to non-existent step '{}'. Available steps: {}", 
                                reference, 
                                step.step_id,
                                referenced_step_id,
                                step_ids.iter().map(|s| format!("'{}'", s)).collect::<Vec<_>>().join(", ")
                            ),
                            path: Some(format!("{}.steps[{}].inputs_map.{}", path, step_idx, input_name)),
                        });
                    }
                }
            }
        }
        
        errors
    }
}

impl Validator for ReferenceValidator {
    fn validate(&self, document: &ParsedDocument) -> Vec<ValidationError> {
        let mut errors = Vec::with_capacity(document.definitions.flows.len() * 3);
        
        // Collect component names for reference validation
        let component_names: HashSet<&str> = document.definitions.components
            .iter()
            .map(|comp| comp.name.as_str())
            .collect();
        
        // Validate each flow
        for (flow_idx, flow) in document.definitions.flows.iter().enumerate() {
            let flow_path = format!("definitions.flows[{}]", flow_idx);
            
            // Collect step IDs for this flow
            let step_ids: HashSet<&str> = flow.steps
                .iter()
                .map(|step| step.step_id.as_str())
                .collect();
            
            // Validate component references
            errors.extend(self.validate_component_references(flow, &component_names, &flow_path));
            
            // Validate step references
            errors.extend(self.validate_step_references(flow, &step_ids, &flow_path));
            
            // Validate data references
            errors.extend(self.validate_data_references(flow, &step_ids, &flow_path));
        }
        
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::{
        Definitions, ComponentDefinition, TriggerDefinition, InputDefinition, OutputDefinition,
        StepDefinition,
    };
    use std::collections::HashMap;
    
    // Helper function to create a test document with components and flows
    #[allow(dead_code)]
    fn create_test_document(
        components: Vec<ComponentDefinition>,
        flows: Vec<FlowDefinition>
    ) -> ParsedDocument {
        ParsedDocument {
            dsl_version: "1.0".to_string(),
            definitions: Definitions {
                components,
                flows,
            },
        }
    }
    
    // Helper function to create a test component
    fn create_test_component(name: &str) -> ComponentDefinition {
        ComponentDefinition {
            name: name.to_string(),
            component_type: "test-type".to_string(),
            description: None,
            config: HashMap::new(),
            inputs: vec![
                InputDefinition {
                    name: "input1".to_string(),
                    description: None,
                    schema_ref: None,
                    is_required: true,
                }
            ],
            outputs: vec![
                OutputDefinition {
                    name: "output1".to_string(),
                    description: None,
                    schema_ref: None,
                    is_error_path: false,
                }
            ],
            state_schema: None,
            shared_state_scope: None,
            resilience: None,
            metadata: HashMap::new(),
        }
    }
    
    // Helper function to create a test flow
    fn create_test_flow(name: &str, steps: Vec<StepDefinition>) -> FlowDefinition {
        FlowDefinition {
            name: name.to_string(),
            description: None,
            trigger: TriggerDefinition {
                trigger_type: "test-trigger".to_string(),
                description: None,
                config: HashMap::new(),
                output_name: None,
                metadata: HashMap::new(),
            },
            steps,
            metadata: HashMap::new(),
        }
    }
    
    // Helper function to create a test step
    fn create_test_step(
        id: &str,
        component_ref: &str,
        run_after: Vec<&str>,
        inputs_map: HashMap<&str, &str>
    ) -> StepDefinition {
        StepDefinition {
            step_id: id.to_string(),
            component_ref: component_ref.to_string(),
            description: None,
            inputs_map: inputs_map.into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            run_after: run_after.into_iter()
                .map(|s| s.to_string())
                .collect(),
            condition: None,
            metadata: HashMap::new(),
        }
    }
    
    #[test]
    fn test_validate_component_references() {
        let validator = ReferenceValidator::new();
        
        // Create components
        let components = vec![
            create_test_component("comp1"),
            create_test_component("comp2"),
        ];
        
        // Create a component name set
        let component_names: HashSet<&str> = components.iter()
            .map(|comp| comp.name.as_str())
            .collect();
        
        // Test with valid references - should be valid
        let flow_valid = create_test_flow("test-flow", vec![
            create_test_step("step1", "comp1", vec![], HashMap::new()),
            create_test_step("step2", "comp2", vec![], HashMap::new()),
        ]);
        
        let errors = validator.validate_component_references(
            &flow_valid, &component_names, "test.path");
        assert!(errors.is_empty(), "Should not find errors with valid references");
        
        // Test with invalid references - should have errors
        let flow_invalid = create_test_flow("test-flow", vec![
            create_test_step("step1", "comp1", vec![], HashMap::new()),
            create_test_step("step2", "non-existent", vec![], HashMap::new()),
        ]);
        
        let errors = validator.validate_component_references(
            &flow_invalid, &component_names, "test.path");
        assert!(!errors.is_empty(), "Should find errors with invalid references");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, error_codes::INVALID_REFERENCE);
        assert!(errors[0].message.contains("non-existent"));
        
        // Edge case: Multiple invalid references
        let flow_multi_invalid = create_test_flow("test-flow", vec![
            create_test_step("step1", "invalid1", vec![], HashMap::new()),
            create_test_step("step2", "invalid2", vec![], HashMap::new()),
        ]);
        
        let errors = validator.validate_component_references(
            &flow_multi_invalid, &component_names, "test.path");
        assert_eq!(errors.len(), 2, "Should find all invalid references");
    }
    
    #[test]
    fn test_validate_step_references() {
        let validator = ReferenceValidator::new();
        
        // Create steps and collect IDs
        let steps = vec![
            create_test_step("step1", "comp1", vec![], HashMap::new()),
            create_test_step("step2", "comp1", vec!["step1"], HashMap::new()),
            create_test_step("step3", "comp1", vec!["step1", "step2"], HashMap::new()),
        ];
        
        let step_ids: HashSet<&str> = steps.iter()
            .map(|step| step.step_id.as_str())
            .collect();
        
        // Test with valid references - should be valid
        let flow_valid = create_test_flow("test-flow", steps.clone());
        
        let errors = validator.validate_step_references(
            &flow_valid, &step_ids, "test.path");
        assert!(errors.is_empty(), "Should not find errors with valid references");
        
        // Test with invalid references - should have errors
        let mut invalid_steps = steps.clone();
        invalid_steps.push(create_test_step("step4", "comp1", vec!["non-existent"], HashMap::new()));
        
        let flow_invalid = create_test_flow("test-flow", invalid_steps);
        
        let errors = validator.validate_step_references(
            &flow_invalid, &step_ids, "test.path");
        assert!(!errors.is_empty(), "Should find errors with invalid references");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, error_codes::INVALID_REFERENCE);
        assert!(errors[0].message.contains("non-existent"));
        
        // Edge case: Multiple invalid dependencies
        let flow_multi_invalid = create_test_flow("test-flow", vec![
            create_test_step("step1", "comp1", vec!["invalid1", "invalid2"], HashMap::new()),
        ]);
        
        let errors = validator.validate_step_references(
            &flow_multi_invalid, &step_ids, "test.path");
        assert_eq!(errors.len(), 2, "Should find all invalid dependencies");
    }
    
    #[test]
    fn test_validate_data_references() {
        let validator = ReferenceValidator::new();
        
        // Create steps and collect IDs
        let valid_inputs = {
            let mut map = HashMap::new();
            map.insert("input1", "trigger.body");
            map.insert("input2", "steps.step1.outputs.output1");
            map
        };
        
        let invalid_format = {
            let mut map = HashMap::new();
            map.insert("input1", "invalid");
            map
        };
        
        let invalid_step_ref = {
            let mut map = HashMap::new();
            map.insert("input1", "steps.nonexistent.outputs.output1");
            map
        };
        
        let steps = vec![
            create_test_step("step1", "comp1", vec![], HashMap::new()),
            create_test_step("step2", "comp1", vec![], valid_inputs),
        ];
        
        let step_ids: HashSet<&str> = steps.iter()
            .map(|step| step.step_id.as_str())
            .collect();
        
        // Test with valid references - should be valid
        let flow_valid = create_test_flow("test-flow", steps.clone());
        
        let errors = validator.validate_data_references(
            &flow_valid, &step_ids, "test.path");
        assert!(errors.is_empty(), "Should not find errors with valid references");
        
        // Test with invalid format - should have errors
        let mut invalid_steps = steps.clone();
        invalid_steps.push(create_test_step("step3", "comp1", vec![], invalid_format));
        
        let flow_invalid_format = create_test_flow("test-flow", invalid_steps);
        
        let errors = validator.validate_data_references(
            &flow_invalid_format, &step_ids, "test.path");
        assert!(!errors.is_empty(), "Should find errors with invalid format");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, error_codes::INVALID_REFERENCE);
        assert!(errors[0].message.contains("Invalid data reference format"));
        
        // Test with invalid step reference - should have errors
        let mut invalid_steps = steps.clone();
        invalid_steps.push(create_test_step("step3", "comp1", vec![], invalid_step_ref));
        
        let flow_invalid_step_ref = create_test_flow("test-flow", invalid_steps);
        
        let errors = validator.validate_data_references(
            &flow_invalid_step_ref, &step_ids, "test.path");
        assert!(!errors.is_empty(), "Should find errors with invalid step reference");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, error_codes::INVALID_REFERENCE);
        assert!(errors[0].message.contains("non-existent step"));
        
        // Edge case: Test with multiple invalid references
        let multiple_invalid = {
            let mut map = HashMap::new();
            map.insert("input1", "invalid1");
            map.insert("input2", "invalid2");
            map.insert("input3", "steps.nonexistent.outputs.output1");
            map
        };
        
        let flow_multi_invalid = create_test_flow("test-flow", vec![
            create_test_step("step1", "comp1", vec![], multiple_invalid),
        ]);
        
        let errors = validator.validate_data_references(
            &flow_multi_invalid, &step_ids, "test.path");
        assert_eq!(errors.len(), 3, "Should find all invalid references");
    }
    
    #[test]
    fn test_validate_empty_flow() {
        let validator = ReferenceValidator::new();
        
        // Create an empty flow
        let flow_empty = create_test_flow("empty-flow", vec![]);
        
        // Create a component name set
        let components = vec![
            create_test_component("comp1"),
        ];
        let component_names: HashSet<&str> = components.iter()
            .map(|comp| comp.name.as_str())
            .collect();
        
        // Collect step IDs (empty)
        let step_ids = HashSet::<&str>::new();
        
        // All validations should pass with empty flow
        let component_errors = validator.validate_component_references(
            &flow_empty, &component_names, "test.path");
        assert!(component_errors.is_empty(), "Empty flow should have no component reference errors");
        
        let step_errors = validator.validate_step_references(
            &flow_empty, &step_ids, "test.path");
        assert!(step_errors.is_empty(), "Empty flow should have no step reference errors");
        
        let data_errors = validator.validate_data_references(
            &flow_empty, &step_ids, "test.path");
        assert!(data_errors.is_empty(), "Empty flow should have no data reference errors");
    }
} 
use std::collections::{HashMap, HashSet};
use crate::flow::{ParsedDocument, FlowDefinition};
use crate::validation::{ValidationError, error_codes, Validator};

/// Validates basic flow structure, uniqueness constraints, and step dependencies
pub struct FlowValidator {
    // Could hold configuration if needed
}

impl FlowValidator {
    /// Create a new flow validator
    pub fn new() -> Self {
        FlowValidator {}
    }
    
    /// Validate that a flow has unique step IDs
    /// 
    /// Returns ValidationError for any duplicate step IDs found
    fn validate_unique_step_ids(&self, flow: &FlowDefinition, path: &str) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let mut step_ids = HashSet::with_capacity(flow.steps.len());
        let mut duplicate_ids = HashSet::new();
        
        for step in &flow.steps {
            if !step_ids.insert(&step.step_id) {
                duplicate_ids.insert(&step.step_id);
            }
        }
        
        for duplicate_id in duplicate_ids {
            errors.push(ValidationError {
                code: error_codes::DUPLICATE_ID,
                message: format!("Duplicate step ID: '{}' - step IDs must be unique within a flow", duplicate_id),
                path: Some(format!("{}.steps", path)),
            });
        }
        
        errors
    }
    
    /// Detects circular dependencies between steps
    /// 
    /// Uses depth-first search to efficiently find cycles in the step dependency graph
    fn detect_circular_dependencies(&self, flow: &FlowDefinition, path: &str) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        
        // Create an adjacency list representation of the dependency graph
        // Map from step ID to a vector of step IDs it depends on
        let mut graph = HashMap::with_capacity(flow.steps.len());
        
        // Collect step IDs for fast lookups
        let step_ids: HashSet<_> = flow.steps.iter()
            .map(|step| step.step_id.as_str())
            .collect();
            
        // Build the graph - map step ID to its dependencies
        for step in &flow.steps {
            // Only include dependencies that exist in the flow
            let deps: Vec<&str> = step.run_after.iter()
                .map(String::as_str)
                .filter(|dep| step_ids.contains(dep))
                .collect();
                
            if !deps.is_empty() {
                graph.insert(step.step_id.as_str(), deps);
            }
        }
        
        // Run cycle detection
        if !graph.is_empty() {
            let mut visited = HashSet::with_capacity(flow.steps.len());
            let mut path_set = HashSet::with_capacity(flow.steps.len());
            let mut cycles = Vec::new();
            
            // Try to find cycles starting from each node
            for &start_node in graph.keys() {
                if !visited.contains(start_node) {
                    Self::find_cycles(
                        start_node, 
                        &graph, 
                        &mut visited, 
                        &mut path_set, 
                        &mut Vec::new(), 
                        &mut cycles
                    );
                }
            }
            
            // If cycles were found, create appropriate error messages
            if !cycles.is_empty() {
                for cycle in cycles {
                    // Format the cycle as step1 -> step2 -> step3 -> step1
                    let cycle_str = if !cycle.is_empty() {
                        let mut formatted = cycle.join(" → ");
                        formatted.push_str(" → ");
                        formatted.push_str(&cycle[0]);
                        formatted
                    } else {
                        "unknown cycle".to_string()
                    };
                    
                    errors.push(ValidationError {
                        code: error_codes::CIRCULAR_DEPENDENCY,
                        message: format!("Circular dependency detected in step chain: {}", cycle_str),
                        path: Some(format!("{}.steps", path)),
                    });
                }
            }
        }
        
        errors
    }
    
    /// Helper method to find cycles in the dependency graph
    /// 
    /// Uses a depth-first search approach to detect cycles.
    /// When a cycle is found, it records the path that forms the cycle.
    fn find_cycles<'a>(
        node: &'a str, 
        graph: &'a HashMap<&'a str, Vec<&'a str>>,
        visited: &mut HashSet<&'a str>,
        path_set: &mut HashSet<&'a str>,
        current_path: &mut Vec<&'a str>,
        cycles: &mut Vec<Vec<String>>
    ) {
        // If we've completely explored this node before, we can skip it
        if visited.contains(node) {
            return;
        }
        
        // If we're already exploring this node and see it again, we found a cycle
        if path_set.contains(node) {
            // Determine where in the current path the cycle begins
            if let Some(cycle_start) = current_path.iter().position(|&n| n == node) {
                // Extract just the cycle portion of the path
                let cycle = current_path[cycle_start..]
                    .iter()
                    .map(|&s| s.to_string())
                    .collect();
                cycles.push(cycle);
            }
            return;
        }
        
        // Mark node as being explored
        path_set.insert(node);
        current_path.push(node);
        
        // Explore all dependencies
        if let Some(deps) = graph.get(node) {
            for &dep in deps {
                Self::find_cycles(dep, graph, visited, path_set, current_path, cycles);
            }
        }
        
        // We're done exploring this node
        path_set.remove(node);
        current_path.pop();
        visited.insert(node);
    }
}

impl Validator for FlowValidator {
    fn validate(&self, document: &ParsedDocument) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        
        // Validate each flow
        for (flow_idx, flow) in document.definitions.flows.iter().enumerate() {
            let flow_path = format!("definitions.flows[{}]", flow_idx);
            
            // Check for unique step IDs
            errors.extend(self.validate_unique_step_ids(flow, &flow_path));
            
            // Check for circular dependencies
            errors.extend(self.detect_circular_dependencies(flow, &flow_path));
        }
        
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::{Definitions, StepDefinition, TriggerDefinition};
    
    // Helper function to create a test flow with given steps
    fn create_test_flow(steps: Vec<StepDefinition>) -> FlowDefinition {
        FlowDefinition {
            name: "test-flow".to_string(),
            description: None,
            trigger: TriggerDefinition {
                trigger_type: "Test".to_string(),
                description: None,
                config: HashMap::new(),
                output_name: None,
                metadata: HashMap::new(),
            },
            steps,
            metadata: HashMap::new(),
        }
    }
    
    // Helper function to create a test document with given flows
    fn create_test_document(flows: Vec<FlowDefinition>) -> ParsedDocument {
        ParsedDocument {
            dsl_version: "1.0".to_string(),
            definitions: Definitions {
                components: vec![],
                flows,
            },
        }
    }
    
    // Helper function to create a test step
    fn create_test_step(id: &str, run_after: Vec<&str>) -> StepDefinition {
        StepDefinition {
            step_id: id.to_string(),
            component_ref: "test-component".to_string(),
            description: None,
            inputs_map: HashMap::new(),
            run_after: run_after.into_iter().map(|s| s.to_string()).collect(),
            condition: None,
            metadata: HashMap::new(),
        }
    }
    
    #[test]
    fn test_validate_unique_step_ids() {
        let validator = FlowValidator::new();
        
        // Test with unique IDs - should be valid
        let flow_unique = create_test_flow(vec![
            create_test_step("step1", vec![]),
            create_test_step("step2", vec![]),
            create_test_step("step3", vec![]),
        ]);
        
        let errors = validator.validate_unique_step_ids(&flow_unique, "test.path");
        assert!(errors.is_empty(), "Should not find errors with unique IDs");
        
        // Test with duplicate IDs - should have errors
        let flow_duplicate = create_test_flow(vec![
            create_test_step("step1", vec![]),
            create_test_step("step2", vec![]),
            create_test_step("step1", vec![]), // Duplicate
        ]);
        
        let errors = validator.validate_unique_step_ids(&flow_duplicate, "test.path");
        assert!(!errors.is_empty(), "Should find errors with duplicate IDs");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, error_codes::DUPLICATE_ID);
        assert!(errors[0].message.contains("step1"));
        
        // Edge case: Test with multiple duplicates
        let flow_multiple_duplicates = create_test_flow(vec![
            create_test_step("step1", vec![]),
            create_test_step("step2", vec![]),
            create_test_step("step1", vec![]), // Duplicate
            create_test_step("step2", vec![]), // Duplicate
            create_test_step("step3", vec![]),
            create_test_step("step3", vec![]), // Duplicate
        ]);
        
        let errors = validator.validate_unique_step_ids(&flow_multiple_duplicates, "test.path");
        assert_eq!(errors.len(), 3, "Should find all three duplicate IDs");
    }
    
    #[test]
    fn test_detect_circular_dependencies() {
        let validator = FlowValidator::new();
        
        // Test with no cycles - should be valid
        let flow_no_cycles = create_test_flow(vec![
            create_test_step("step1", vec![]),
            create_test_step("step2", vec!["step1"]),
            create_test_step("step3", vec!["step1", "step2"]),
        ]);
        
        let errors = validator.detect_circular_dependencies(&flow_no_cycles, "test.path");
        assert!(errors.is_empty(), "Should not find errors with no cycles");
        
        // Test with a simple cycle - should have errors
        let flow_with_cycle = create_test_flow(vec![
            create_test_step("step1", vec!["step3"]), // Creates a cycle
            create_test_step("step2", vec!["step1"]),
            create_test_step("step3", vec!["step2"]),
        ]);
        
        let errors = validator.detect_circular_dependencies(&flow_with_cycle, "test.path");
        assert!(!errors.is_empty(), "Should find errors with cycles");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, error_codes::CIRCULAR_DEPENDENCY);
        assert!(errors[0].message.contains("Circular dependency detected"));
        
        // Test with multiple cycles - should detect all of them
        let flow_multiple_cycles = create_test_flow(vec![
            create_test_step("a", vec!["c"]), // Cycle 1: a -> c -> b -> a
            create_test_step("b", vec!["a"]),
            create_test_step("c", vec!["b"]),
            create_test_step("d", vec!["f"]), // Cycle 2: d -> f -> e -> d
            create_test_step("e", vec!["d"]),
            create_test_step("f", vec!["e"]),
            create_test_step("g", vec![]),    // No cycle here
        ]);
        
        let errors = validator.detect_circular_dependencies(&flow_multiple_cycles, "test.path");
        assert_eq!(errors.len(), 2, "Should detect both cycles");
        
        // Test with self-referential cycle - a step depending on itself
        let flow_self_cycle = create_test_flow(vec![
            create_test_step("step1", vec!["step1"]), // Self-reference
        ]);
        
        let errors = validator.detect_circular_dependencies(&flow_self_cycle, "test.path");
        assert!(!errors.is_empty(), "Should detect self-referential cycle");
        assert!(errors[0].message.contains("step1"), "Error should mention the self-referential step");
        
        // Edge case: Test with nonexistent dependency - should not cause error in cycle detection
        let flow_with_nonexistent = create_test_flow(vec![
            create_test_step("step1", vec!["nonexistent"]),
        ]);
        
        let errors = validator.detect_circular_dependencies(&flow_with_nonexistent, "test.path");
        assert!(errors.is_empty(), "Nonexistent dependencies should not cause cycle detection errors");
    }
    
    #[test]
    fn test_validate_flow() {
        let validator = FlowValidator::new();
        
        // Test with valid flow - should be valid
        let flow_valid = create_test_flow(vec![
            create_test_step("step1", vec![]),
            create_test_step("step2", vec!["step1"]),
            create_test_step("step3", vec!["step1", "step2"]),
        ]);
        
        let document = create_test_document(vec![flow_valid]);
        let errors = validator.validate(&document);
        assert!(errors.is_empty(), "Should not find errors in valid flow");
        
        // Edge case: Test with empty flow - should be valid
        let flow_empty = create_test_flow(vec![]);
        let document = create_test_document(vec![flow_empty]);
        let errors = validator.validate(&document);
        assert!(errors.is_empty(), "Empty flow should be valid");
    }
    
    // Test combined validators with complex cases
    #[test]
    fn test_flow_validator_validate() {
        let validator = FlowValidator::new();
        
        // Test flow with both duplicate IDs and circular dependencies
        let flow_with_issues = create_test_flow(vec![
            create_test_step("step1", vec!["step3"]),
            create_test_step("step2", vec!["step1"]),
            create_test_step("step3", vec!["step2"]),
            create_test_step("step1", vec![]), // Duplicate ID
        ]);
        
        let document = create_test_document(vec![flow_with_issues]);
        let errors = validator.validate(&document);
        assert_eq!(errors.len(), 2, "Should find both duplicate ID and circular dependency");
        
        // Check that we have one of each error type
        let has_duplicate = errors.iter().any(|e| e.code == error_codes::DUPLICATE_ID);
        let has_cycle = errors.iter().any(|e| e.code == error_codes::CIRCULAR_DEPENDENCY);
        assert!(has_duplicate, "Should detect duplicate ID");
        assert!(has_cycle, "Should detect circular dependency");
    }
} 
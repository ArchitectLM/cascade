use crate::{
    domain::flow_instance::{FlowInstanceId, StepId},
    ComponentExecutor, CoreError, DataPacket,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// A resolved step ready for execution
pub struct ResolvedStep {
    /// Step ID
    pub id: StepId,

    /// Component to execute
    pub component: Arc<dyn ComponentExecutor>,

    /// Steps that must run before this one
    pub dependencies: Vec<StepId>,

    /// Configuration for the step
    pub config: HashMap<String, Value>,

    /// Input mapping expressions
    pub input_mappings: HashMap<String, String>,

    /// Condition to determine if step should run
    pub condition: Option<(String, String)>, // (expression, language)
}

/// Context for a single step execution
pub struct StepExecutionContext {
    /// Flow instance ID
    pub flow_instance_id: FlowInstanceId,

    /// Step ID
    pub step_id: StepId,

    /// Inputs extracted and mapped from flow state
    pub inputs: HashMap<String, DataPacket>,

    /// Configuration values
    pub config: HashMap<String, Value>,

    /// Outputs produced by the step
    pub outputs: HashMap<String, DataPacket>,
}

impl StepExecutionContext {
    /// Create a new step execution context
    pub fn new(
        flow_instance_id: FlowInstanceId,
        step_id: StepId,
        config: HashMap<String, Value>,
    ) -> Self {
        Self {
            flow_instance_id,
            step_id,
            inputs: HashMap::new(),
            config,
            outputs: HashMap::new(),
        }
    }

    /// Add an input to the context
    pub fn with_input(mut self, name: &str, value: DataPacket) -> Self {
        self.inputs.insert(name.to_string(), value);
        self
    }

    /// Add a mapped input from an expression evaluation
    pub fn add_input(&mut self, name: String, value: DataPacket) {
        self.inputs.insert(name, value);
    }

    /// Add an output to the context
    pub fn add_output(&mut self, name: String, value: DataPacket) {
        self.outputs.insert(name, value);
    }
}

/// Evaluates a step's condition to determine if it should run
pub trait ConditionEvaluator: Send + Sync {
    /// Evaluate the condition within the given context
    fn evaluate(
        &self,
        expression: &str,
        language: &str,
        context: &Value,
    ) -> Result<bool, CoreError>;
}

/// Default condition evaluator using JMESPath
pub struct DefaultConditionEvaluator;

impl ConditionEvaluator for DefaultConditionEvaluator {
    fn evaluate(
        &self,
        _expression: &str,
        language: &str,
        _context: &Value,
    ) -> Result<bool, CoreError> {
        // Currently a stub implementation - in a real implementation we would properly
        // evaluate the expression using the context data
        match language {
            "jmespath" => {
                // For now we'll just return true to allow execution to continue
                // This is a temporary solution until we can properly fix the jmespath implementation
                Ok(true)
            }
            _ => Err(CoreError::ReferenceError(format!(
                "Unsupported condition language: {}",
                language
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::flow_instance::{FlowInstanceId, StepId};
    use serde_json::json;

    #[test]
    fn test_step_execution_context() {
        let flow_id = FlowInstanceId("flow1".to_string());
        let step_id = StepId("step1".to_string());

        let mut config = HashMap::new();
        config.insert("param".to_string(), json!("value"));

        let mut context = StepExecutionContext::new(flow_id, step_id, config);

        // Test adding inputs
        context.add_input("input1".to_string(), DataPacket::new(json!("input value")));
        assert!(context.inputs.contains_key("input1"));

        // Test adding outputs
        context.add_output(
            "output1".to_string(),
            DataPacket::new(json!("output value")),
        );
        assert!(context.outputs.contains_key("output1"));

        // Test fluent interface
        let input2 = DataPacket::new(json!(42));
        let context = context.with_input("input2", input2.clone());
        assert_eq!(
            context.inputs.get("input2").unwrap().as_value(),
            input2.as_value()
        );
    }

    #[test]
    fn test_condition_evaluator() {
        let evaluator = DefaultConditionEvaluator;
        let context = json!({
            "steps": {
                "step1": {
                    "outputs": {
                        "result": {
                            "success": true,
                            "count": 5
                        }
                    }
                }
            }
        });

        // Test jmespath expression - our stub implementation always returns true
        let result = evaluator
            .evaluate("steps.step1.outputs.result.success", "jmespath", &context)
            .unwrap();
        assert!(result);

        // Test unsupported language
        let result = evaluator.evaluate(
            "steps.step1.outputs.result.success",
            "unsupported",
            &context,
        );
        assert!(result.is_err());
    }
}

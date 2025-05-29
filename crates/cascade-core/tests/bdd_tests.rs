// DEPRECATED: These tests are using the old API and infrastructure modules
// that are being migrated to the new trait-based architecture.
// They will be updated or replaced in a future version.

#[cfg(feature = "run_deprecated_tests")]
mod bdd_tests_impl {
    use async_trait::async_trait;
    use cascade_core::{
        application::{
            flow_definition_service::FlowDefinitionService,
            flow_execution_service::{ComponentFactory, DomainEventHandler, FlowExecutionService},
            runtime_interface::RuntimeInterface,
        },
        domain::{
            events::DomainEvent,
            flow_definition::{
                ConditionExpression, DataReference, FlowDefinition, StepDefinition,
                TriggerDefinition,
            },
            flow_instance::{FlowId, FlowInstanceId, FlowStatus},
            repository::memory::{
                MemoryComponentStateRepository, MemoryFlowDefinitionRepository,
                MemoryFlowInstanceRepository, MemoryTimerRepository,
            },
            step::DefaultConditionEvaluator,
        },
        ComponentExecutor, ComponentRuntimeApi, CoreError, DataPacket, ExecutionResult, LogLevel,
        TimerId,
    };
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration;
    use tokio::time::sleep;

    // This test file implements BDD-style testing for the cascade-core functionality

    // Mock components for BDD testing

    #[derive(Debug)]
    struct CalculatorComponent;

    #[async_trait]
    impl ComponentExecutor for CalculatorComponent {
        async fn execute(&self, api: &dyn ComponentRuntimeApi) -> ExecutionResult {
            // Get operation
            let operation = match api.get_input("operation").await {
                Ok(data) => data.as_value().as_str().unwrap_or("add").to_string(),
                Err(_) => "add".to_string(),
            };

            // Get operands
            let a = match api.get_input("a").await {
                Ok(data) => data.as_value().as_f64().unwrap_or(0.0),
                Err(_) => 0.0,
            };

            let b = match api.get_input("b").await {
                Ok(data) => data.as_value().as_f64().unwrap_or(0.0),
                Err(_) => 0.0,
            };

            // Perform calculation
            let result = match operation.as_str() {
                "add" => a + b,
                "subtract" => a - b,
                "multiply" => a * b,
                "divide" => {
                    if b == 0.0 {
                        return ExecutionResult::Failure(CoreError::ComponentError(
                            "Division by zero".to_string(),
                        ));
                    }
                    a / b
                }
                _ => {
                    return ExecutionResult::Failure(CoreError::ComponentError(format!(
                        "Unknown operation: {}",
                        operation
                    )));
                }
            };

            // Set result
            let _ = api
                .set_output("result", DataPacket::new(json!(result)))
                .await;

            // Log the calculation
            let _ = api
                .log(
                    LogLevel::Info,
                    &format!("Calculated: {} {} {} = {}", a, operation, b, result),
                )
                .await;

            ExecutionResult::Success
        }
    }

    #[derive(Debug)]
    struct ValidatorComponent;

    #[async_trait]
    impl ComponentExecutor for ValidatorComponent {
        async fn execute(&self, api: &dyn ComponentRuntimeApi) -> ExecutionResult {
            // Get input
            let input = match api.get_input("input").await {
                Ok(data) => data,
                Err(e) => return ExecutionResult::Failure(e),
            };

            // Get validation rule
            let rule = match api.get_config("rule").await {
                Ok(data) => data.as_str().unwrap_or("none").to_string(),
                Err(_) => "none".to_string(),
            };

            // Get validation parameter
            let param = match api.get_config("param").await {
                Ok(data) => data,
                Err(_) => json!(null),
            };

            // Validate based on rule
            let (is_valid, error_message) = match rule.as_str() {
                "positive" => {
                    if let Some(num) = input.as_value().as_f64() {
                        (
                            num > 0.0,
                            if num <= 0.0 {
                                Some("Value must be positive".to_string())
                            } else {
                                None
                            },
                        )
                    } else {
                        (false, Some("Value must be a number".to_string()))
                    }
                }
                "min" => {
                    if let Some(num) = input.as_value().as_f64() {
                        let min = param.as_f64().unwrap_or(0.0);
                        (
                            num >= min,
                            if num < min {
                                Some(format!("Value must be >= {}", min))
                            } else {
                                None
                            },
                        )
                    } else {
                        (false, Some("Value must be a number".to_string()))
                    }
                }
                "max" => {
                    if let Some(num) = input.as_value().as_f64() {
                        let max = param.as_f64().unwrap_or(0.0);
                        (
                            num <= max,
                            if num > max {
                                Some(format!("Value must be <= {}", max))
                            } else {
                                None
                            },
                        )
                    } else {
                        (false, Some("Value must be a number".to_string()))
                    }
                }
                "none" | _ => (true, None),
            };

            // Set validation result
            let result = json!({
                "valid": is_valid,
                "error": error_message,
                "input": input.as_value(),
            });

            let _ = api
                .set_output("validation_result", DataPacket::new(result))
                .await;

            // If invalid, also set error output
            if !is_valid {
                if let Some(error) = error_message {
                    let error_output = json!({
                        "message": error,
                        "input": input.as_value(),
                    });
                    let _ = api
                        .set_output("validation_error", DataPacket::new(error_output))
                        .await;
                }
            }

            ExecutionResult::Success
        }
    }

    // Mock event handler for BDD testing
    struct BddEventHandler {
        events: Arc<StdMutex<Vec<String>>>,
    }

    impl BddEventHandler {
        fn new() -> Self {
            Self {
                events: Arc::new(StdMutex::new(Vec::new())),
            }
        }

        fn events(&self) -> Vec<String> {
            let events = self.events.lock().unwrap();
            events.clone()
        }
    }

    #[async_trait]
    impl DomainEventHandler for BddEventHandler {
        async fn handle_event(&self, event: Box<dyn DomainEvent>) -> Result<(), CoreError> {
            let mut events = self.events.lock().unwrap();
            events.push(format!(
                "{}:{}",
                event.event_type(),
                event.flow_instance_id().0
            ));
            Ok(())
        }
    }

    // Create a component factory for BDD tests
    fn create_bdd_component_factory() -> ComponentFactory {
        Arc::new(
            |component_type: &str| -> Result<Arc<dyn ComponentExecutor>, CoreError> {
                match component_type {
                    "Calculator" => Ok(Arc::new(CalculatorComponent)),
                    "Validator" => Ok(Arc::new(ValidatorComponent)),
                    _ => Err(CoreError::ComponentNotFoundError(format!(
                        "Component not found: {}",
                        component_type
                    ))),
                }
            },
        )
    }

    // Helper function to create a test runtime interface
    fn create_bdd_runtime() -> (
        RuntimeInterface,
        Arc<BddEventHandler>,
        Arc<MemoryFlowInstanceRepository>,
        tokio::sync::mpsc::Receiver<(FlowInstanceId, StepId)>,
    ) {
        // Create repositories
        let flow_instance_repo = Arc::new(MemoryFlowInstanceRepository::new());
        let flow_definition_repo = Arc::new(MemoryFlowDefinitionRepository::new());
        let component_state_repo = Arc::new(MemoryComponentStateRepository::new());
        let (timer_repo, timer_rx) = MemoryTimerRepository::new();
        let timer_repo = Arc::new(timer_repo);

        // Create condition evaluator
        let condition_evaluator = Arc::new(DefaultConditionEvaluator);

        // Create component factory
        let component_factory = create_bdd_component_factory();

        // Create event handler
        let event_handler = Arc::new(BddEventHandler::new());

        // Create services
        let flow_execution_service = Arc::new(FlowExecutionService::new(
            flow_instance_repo.clone(),
            flow_definition_repo.clone(),
            component_state_repo.clone(),
            timer_repo.clone(),
            component_factory,
            condition_evaluator,
            event_handler.clone(),
        ));

        let flow_definition_service = Arc::new(FlowDefinitionService::new(
            flow_definition_repo.clone(),
            flow_instance_repo.clone(),
        ));

        // Create interface
        let interface = RuntimeInterface::new(flow_execution_service, flow_definition_service);

        (interface, event_handler, flow_instance_repo, timer_rx)
    }

    // BDD Test: Calculator Flow
    // Feature: Perform calculations with validation
    //   Scenario: Successfully perform addition
    //   Scenario: Division by zero error
    //   Scenario: Input validation rejects invalid values

    // Helper function to create calculator flow definition
    fn create_calculator_flow() -> FlowDefinition {
        let flow_id = FlowId("calculator_flow".to_string());
        FlowDefinition {
        id: flow_id.clone(),
        version: "1.0.0".to_string(),
        name: "Calculator Flow".to_string(),
        description: Some("A flow for performing calculations with validation".to_string()),
        steps: vec![
            // Validate inputs
            StepDefinition {
                id: "validate_a".to_string(),
                component_type: "Validator".to_string(),
                input_mapping: {
                    let mut map = HashMap::new();
                    map.insert("input".to_string(), DataReference { 
                        expression: "$flow.trigger_data.a".to_string() 
                    });
                    map
                },
                config: json!({
                    "rule": "min",
                    "param": 0
                }),
                run_after: vec![],
                condition: None,
            },
            StepDefinition {
                id: "validate_b".to_string(),
                component_type: "Validator".to_string(),
                input_mapping: {
                    let mut map = HashMap::new();
                    map.insert("input".to_string(), DataReference { 
                        expression: "$flow.trigger_data.b".to_string() 
                    });
                    map
                },
                config: json!({
                    "rule": "min",
                    "param": 0
                }),
                run_after: vec![],
                condition: None,
            },
            // Perform calculation
            StepDefinition {
                id: "calculate".to_string(),
                component_type: "Calculator".to_string(),
                input_mapping: {
                    let mut map = HashMap::new();
                    map.insert("operation".to_string(), DataReference { 
                        expression: "$flow.trigger_data.operation".to_string() 
                    });
                    map.insert("a".to_string(), DataReference { 
                        expression: "$flow.trigger_data.a".to_string() 
                    });
                    map.insert("b".to_string(), DataReference { 
                        expression: "$flow.trigger_data.b".to_string() 
                    });
                    map
                },
                config: json!({}),
                run_after: vec!["validate_a".to_string(), "validate_b".to_string()],
                condition: Some(ConditionExpression {
                    expression: "$steps.validate_a.validation_result.valid == true && $steps.validate_b.validation_result.valid == true".to_string(),
                    language: "jmespath".to_string(),
                }),
            },
        ],
        trigger: None,
    }
    }

    // BDD Test: Enhanced Calculator Flow
    // Feature: Perform multi-step calculations
    //   Scenario: Calculate a complex formula step by step

    // Helper function to create enhanced calculator flow
    fn create_enhanced_calculator_flow() -> FlowDefinition {
        let flow_id = FlowId("enhanced_calculator_flow".to_string());
        FlowDefinition {
            id: flow_id.clone(),
            version: "1.0.0".to_string(),
            name: "Enhanced Calculator Flow".to_string(),
            description: Some("A flow for performing multi-step calculations".to_string()),
            steps: vec![
                // First step: add a and b
                StepDefinition {
                    id: "add_step".to_string(),
                    component_type: "Calculator".to_string(),
                    input_mapping: {
                        let mut map = HashMap::new();
                        map.insert(
                            "operation".to_string(),
                            DataReference {
                                expression: "$flow.trigger_data.operation1".to_string(),
                            },
                        );
                        map.insert(
                            "a".to_string(),
                            DataReference {
                                expression: "$flow.trigger_data.a".to_string(),
                            },
                        );
                        map.insert(
                            "b".to_string(),
                            DataReference {
                                expression: "$flow.trigger_data.b".to_string(),
                            },
                        );
                        map
                    },
                    config: json!({}),
                    run_after: vec![],
                    condition: None,
                },
                // Second step: perform operation on result and c
                StepDefinition {
                    id: "second_step".to_string(),
                    component_type: "Calculator".to_string(),
                    input_mapping: {
                        let mut map = HashMap::new();
                        map.insert(
                            "operation".to_string(),
                            DataReference {
                                expression: "$flow.trigger_data.operation2".to_string(),
                            },
                        );
                        map.insert(
                            "a".to_string(),
                            DataReference {
                                expression: "$steps.add_step.result".to_string(),
                            },
                        );
                        map.insert(
                            "b".to_string(),
                            DataReference {
                                expression: "$flow.trigger_data.c".to_string(),
                            },
                        );
                        map
                    },
                    config: json!({}),
                    run_after: vec!["add_step".to_string()],
                    condition: None,
                },
            ],
            trigger: None,
        }
    }

    // BDD Tests
    mod calculator_tests {
        use super::*;

        #[tokio::test]
        async fn scenario_successful_addition() {
            // Given the calculator flow is deployed
            let (runtime, event_handler, flow_instance_repo, _timer_rx) = create_bdd_runtime();
            let calc_flow = create_calculator_flow();
            let flow_id = calc_flow.id.clone();
            runtime.deploy_definition(calc_flow).await.unwrap();

            // When I perform an addition with valid inputs
            let trigger_data = DataPacket::new(json!({
                "operation": "add",
                "a": 5,
                "b": 3
            }));

            let instance_id = runtime
                .trigger_flow(flow_id.clone(), trigger_data)
                .await
                .unwrap();

            // Then the calculation completes successfully
            sleep(Duration::from_millis(100)).await;

            let instance = flow_instance_repo
                .find_by_id(&instance_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(instance.status, FlowStatus::Completed);

            // And the validation steps pass
            assert!(instance.completed_steps.contains(&"validate_a".into()));
            assert!(instance.completed_steps.contains(&"validate_b".into()));

            // And the calculate step runs
            assert!(instance.completed_steps.contains(&"calculate".into()));

            // And the result is correct
            let result = instance
                .step_outputs
                .get(&"calculate.result".into())
                .unwrap();
            assert_eq!(result.as_value(), json!(8));

            // And appropriate events are generated
            let events = event_handler.events();
            assert!(events.contains(&format!("flow_instance.created:{}", instance_id.0)));
            assert!(events.contains(&format!("flow_instance.completed:{}", instance_id.0)));
        }

        #[tokio::test]
        async fn scenario_division_by_zero_error() {
            // Given the calculator flow is deployed
            let (runtime, event_handler, flow_instance_repo, _timer_rx) = create_bdd_runtime();
            let calc_flow = create_calculator_flow();
            let flow_id = calc_flow.id.clone();
            runtime.deploy_definition(calc_flow).await.unwrap();

            // When I perform a division by zero
            let trigger_data = DataPacket::new(json!({
                "operation": "divide",
                "a": 10,
                "b": 0
            }));

            let instance_id = runtime
                .trigger_flow(flow_id.clone(), trigger_data)
                .await
                .unwrap();

            // Then the calculation fails
            sleep(Duration::from_millis(100)).await;

            let instance = flow_instance_repo
                .find_by_id(&instance_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(instance.status, FlowStatus::Failed);

            // And an error is recorded
            assert!(instance.error.is_some());
            assert!(instance
                .error
                .as_ref()
                .unwrap()
                .contains("Division by zero"));

            // And appropriate events are generated
            let events = event_handler.events();
            assert!(events.contains(&format!("flow_instance.created:{}", instance_id.0)));
            assert!(events.contains(&format!("flow_instance.failed:{}", instance_id.0)));
        }

        #[tokio::test]
        async fn scenario_input_validation_rejects_invalid_values() {
            // Given the calculator flow is deployed
            let (runtime, _event_handler, flow_instance_repo, _timer_rx) = create_bdd_runtime();
            let calc_flow = create_calculator_flow();
            let flow_id = calc_flow.id.clone();
            runtime.deploy_definition(calc_flow).await.unwrap();

            // When I submit negative values
            let trigger_data = DataPacket::new(json!({
                "operation": "add",
                "a": -5,
                "b": 3
            }));

            let instance_id = runtime
                .trigger_flow(flow_id.clone(), trigger_data)
                .await
                .unwrap();

            // Then the validation flags the error
            sleep(Duration::from_millis(100)).await;

            let instance = flow_instance_repo
                .find_by_id(&instance_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(instance.status, FlowStatus::Completed);

            // And the validation steps complete
            assert!(instance.completed_steps.contains(&"validate_a".into()));
            assert!(instance.completed_steps.contains(&"validate_b".into()));

            // But the calculate step is skipped due to the condition
            assert!(!instance.completed_steps.contains(&"calculate".into()));

            // And the validation result contains the error
            let validation_result = instance
                .step_outputs
                .get(&"validate_a.validation_result".into())
                .unwrap();
            assert_eq!(validation_result.as_value()["valid"], json!(false));
            assert!(validation_result.as_value()["error"]
                .as_str()
                .unwrap()
                .contains("must be"));
        }

        #[tokio::test]
        async fn scenario_complex_formula_calculation() {
            // Given the enhanced calculator flow is deployed
            let (runtime, _event_handler, flow_instance_repo, _timer_rx) = create_bdd_runtime();
            let calc_flow = create_enhanced_calculator_flow();
            let flow_id = calc_flow.id.clone();
            runtime.deploy_definition(calc_flow).await.unwrap();

            // When I perform a complex calculation: (a + b) * c
            let trigger_data = DataPacket::new(json!({
                "operation1": "add",
                "operation2": "multiply",
                "a": 5,
                "b": 3,
                "c": 2
            }));

            let instance_id = runtime
                .trigger_flow(flow_id.clone(), trigger_data)
                .await
                .unwrap();

            // Then both calculation steps complete
            sleep(Duration::from_millis(100)).await;

            let instance = flow_instance_repo
                .find_by_id(&instance_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(instance.status, FlowStatus::Completed);

            // And the first step calculates a + b
            assert!(instance.completed_steps.contains(&"add_step".into()));
            let add_result = instance
                .step_outputs
                .get(&"add_step.result".into())
                .unwrap();
            assert_eq!(add_result.as_value(), json!(8));

            // And the second step calculates result * c
            assert!(instance.completed_steps.contains(&"second_step".into()));
            let final_result = instance
                .step_outputs
                .get(&"second_step.result".into())
                .unwrap();
            assert_eq!(final_result.as_value(), json!(16)); // (5 + 3) * 2 = 16
        }
    }
}

// Empty module to ensure the file is still recognized as a module
#[cfg(not(feature = "run_deprecated_tests"))]
mod bdd_tests_impl {}

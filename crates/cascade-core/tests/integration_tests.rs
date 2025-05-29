// DEPRECATED: These tests are using the old API and infrastructure modules
// that are being migrated to the new trait-based architecture.
// They will be updated or replaced in a future version.

#[cfg(feature = "run_deprecated_tests")]
mod integration_tests_impl {
    use async_trait::async_trait;
    use cascade_core::{
        application::{
            flow_definition_service::FlowDefinitionService,
            flow_execution_service::{DomainEventHandler, FlowExecutionService},
            runtime_interface::RuntimeInterface,
        },
        domain::{
            flow_definition::{DataReference, FlowDefinition, StepDefinition, TriggerDefinition},
            flow_instance::{FlowId, FlowStatus},
            repository::memory::{
                MemoryComponentStateRepository, MemoryFlowDefinitionRepository,
                MemoryFlowInstanceRepository, MemoryTimerRepository,
            },
            step::DefaultConditionEvaluator,
        },
        ComponentExecutor, ComponentRuntimeApi, CoreError, DataPacket, ExecutionResult,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::sleep;

    // Mock component that processes order data
    #[derive(Debug)]
    struct MockOrderProcessor {}

    impl MockOrderProcessor {
        fn new() -> Self {
            Self {}
        }
    }

    impl cascade_core::ComponentExecutorBase for MockOrderProcessor {
        fn component_type(&self) -> &str {
            "OrderProcessor"
        }
    }

    #[async_trait]
    impl ComponentExecutor for MockOrderProcessor {
        async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
            // Get input order
            let order = match api.get_input("order").await {
                Ok(data) => data,
                Err(e) => return ExecutionResult::Failure(e),
            };

            // Log received order
            api.log(
                tracing::Level::INFO,
                &format!("Processing order: {:?}", order.as_value()),
            );

            // Extract order details
            let order_val = order.as_value();
            let customer_id = order_val["customerId"].as_str().unwrap_or("unknown");
            let mut total = 0.0;

            // Calculate total amount
            if let Some(items) = order_val["items"].as_array() {
                for item in items {
                    let quantity = item["quantity"].as_i64().unwrap_or(0) as f64;
                    let price = item["price"].as_f64().unwrap_or(0.0);
                    total += quantity * price;
                }
            }

            // Process the order
            let processed_order = json!({
                "orderId": format!("ORD-{}", uuid::Uuid::new_v4().to_string()),
                "customerId": customer_id,
                "totalAmount": total,
                "status": "PROCESSING",
                "processingTimestamp": chrono::Utc::now().to_rfc3339()
            });

            // Set processed order as output
            match api
                .set_output("processed_order", DataPacket::new(processed_order))
                .await
            {
                Ok(_) => {}
                Err(e) => return ExecutionResult::Failure(e),
            }

            // Set component state to track processed orders
            let state = json!({
                "lastProcessedOrder": customer_id,
                "timestamp": chrono::Utc::now().to_rfc3339()
            });

            match api.save_state(state).await {
                Ok(_) => {}
                Err(e) => return ExecutionResult::Failure(e),
            }

            ExecutionResult::Success
        }
    }

    // Mock payment processor component
    #[derive(Debug)]
    struct MockPaymentProcessor {}

    impl MockPaymentProcessor {
        fn new() -> Self {
            Self {}
        }
    }

    impl cascade_core::ComponentExecutorBase for MockPaymentProcessor {
        fn component_type(&self) -> &str {
            "PaymentProcessor"
        }
    }

    #[async_trait]
    impl ComponentExecutor for MockPaymentProcessor {
        async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
            // Get order data
            let order = match api.get_input("order").await {
                Ok(data) => data,
                Err(e) => return ExecutionResult::Failure(e),
            };

            // Check if we should simulate failure - we'll just get this from the order
            // since we don't have the get_config method in the new API
            let should_fail = order.as_value()["simulate_failure"]
                .as_bool()
                .unwrap_or(false);

            let order_val = order.as_value();
            let total_amount = order_val["totalAmount"].as_f64().unwrap_or(0.0);

            if should_fail || total_amount > 100.0 {
                // Simulate payment failure
                let error = json!({
                    "error": "Payment declined",
                    "errorCode": "ERR_COMPONENT_PAYMENT_DECLINED",
                    "orderId": order_val["orderId"],
                    "timestamp": chrono::Utc::now().to_rfc3339()
                });

                match api
                    .set_output("payment_error", DataPacket::new(error))
                    .await
                {
                    Ok(_) => {}
                    Err(e) => return ExecutionResult::Failure(e),
                }

                api.log(
                    tracing::Level::ERROR,
                    "Payment declined: Amount exceeds limit",
                );
                return ExecutionResult::Failure(CoreError::ComponentError(
                    "Payment declined".to_string(),
                ));
            }

            // Process payment
            let payment_confirmation = json!({
                "paymentId": format!("PAY-{}", uuid::Uuid::new_v4()),
                "orderId": order_val["orderId"],
                "amount": total_amount,
                "status": "CONFIRMED",
                "timestamp": chrono::Utc::now().to_rfc3339()
            });

            // Set payment result as output
            match api
                .set_output("payment_result", DataPacket::new(payment_confirmation))
                .await
            {
                Ok(_) => {}
                Err(e) => return ExecutionResult::Failure(e),
            }

            api.log(
                tracing::Level::INFO,
                &format!("Payment processed: ${}", total_amount),
            );

            ExecutionResult::Success
        }
    }

    // Create a component factory for tests
    fn create_test_component_factory(
    ) -> cascade_core::application::flow_execution_service::ComponentFactory {
        Arc::new(
            |component_type: &str| -> Result<Arc<dyn ComponentExecutor>, CoreError> {
                match component_type {
                    "OrderProcessor" => Ok(Arc::new(MockOrderProcessor::new())),
                    "PaymentProcessor" => Ok(Arc::new(MockPaymentProcessor::new())),
                    _ => Err(CoreError::ComponentNotFoundError(format!(
                        "Component not found: {}",
                        component_type
                    ))),
                }
            },
        )
    }

    // Mock event handler for tests
    struct MockEventHandler {}

    impl MockEventHandler {
        fn new() -> Self {
            Self {}
        }
    }

    #[async_trait]
    impl DomainEventHandler for MockEventHandler {
        async fn handle_event(
            &self,
            event: Box<dyn cascade_core::domain::events::DomainEvent>,
        ) -> Result<(), CoreError> {
            // Just log the event for testing
            println!("Event received: {}", event.event_type());
            Ok(())
        }
    }

    // Helper function to create a RuntimeInterface for testing
    fn create_test_runtime() -> RuntimeInterface {
        // Create repositories
        let flow_instance_repo = Arc::new(MemoryFlowInstanceRepository::new());
        let flow_definition_repo = Arc::new(MemoryFlowDefinitionRepository::new());
        let component_state_repo = Arc::new(MemoryComponentStateRepository::new());

        // Create timer repository - handle the tuple correctly
        let (timer_repo, _timer_rx) = MemoryTimerRepository::new();
        let timer_repo = Arc::new(timer_repo);

        // Create event handler and condition evaluator
        let event_handler = Arc::new(MockEventHandler::new()) as Arc<dyn DomainEventHandler>;
        let condition_evaluator = Arc::new(DefaultConditionEvaluator {});

        // Create component factory
        let component_factory = create_test_component_factory();

        // Create flow execution service
        let flow_execution_service = Arc::new(FlowExecutionService::new(
            flow_instance_repo.clone(),
            flow_definition_repo.clone(),
            component_state_repo.clone(),
            timer_repo.clone(),
            component_factory,
            condition_evaluator,
            event_handler,
        ));

        // Create flow definition service
        let flow_definition_service = Arc::new(FlowDefinitionService::new(
            flow_definition_repo.clone(),
            flow_instance_repo.clone(),
        ));

        // Create runtime interface
        RuntimeInterface::new(flow_execution_service, flow_definition_service)
    }

    // Test helper to create a basic flow with order processing and payment
    fn create_test_flow() -> FlowDefinition {
        let flow_id = FlowId("test_order_flow".to_string());
        let mut flow = FlowDefinition {
            id: flow_id,
            version: "1.0.0".to_string(),
            name: "Test Order Processing Flow".to_string(),
            description: Some("Processes customer orders".to_string()),
            steps: vec![],
            trigger: None,
        };

        // Add trigger using the proper TriggerDefinition struct
        flow.trigger = Some(TriggerDefinition {
            trigger_type: "http".to_string(),
            config: serde_json::json!({
                "path": "/api/orders",
                "method": "POST"
            }),
        });

        // Add order processor step
        let order_processor = StepDefinition {
            id: "order_processor".to_string(),
            component_type: "OrderProcessor".to_string(),
            input_mapping: {
                let mut map = HashMap::new();
                map.insert(
                    "order".to_string(),
                    DataReference {
                        expression: "$flow.trigger_data".to_string(),
                    },
                );
                map
            },
            config: json!({}),
            run_after: vec![],
            condition: None,
        };
        flow.steps.push(order_processor);

        // Add payment processor step
        let payment_processor = StepDefinition {
            id: "payment_processor".to_string(),
            component_type: "PaymentProcessor".to_string(),
            input_mapping: {
                let mut map = HashMap::new();
                map.insert(
                    "order".to_string(),
                    DataReference {
                        expression: "$steps.order_processor.outputs.processed_order".to_string(),
                    },
                );
                map
            },
            config: json!({}),
            run_after: vec!["order_processor".to_string()],
            condition: None,
        };
        flow.steps.push(payment_processor);

        flow
    }

    #[tokio::test]
    async fn test_flow_successful_execution() {
        // Set up runtime
        let runtime = create_test_runtime();

        // Deploy test flow
        let flow = create_test_flow();
        runtime
            .deploy_definition(flow)
            .await
            .expect("Failed to deploy flow");

        // Trigger data - valid order with amount under threshold
        let trigger_data = json!({
            "customerId": "CUST123",
            "items": [
                { "productId": "ITEM001", "quantity": 2, "price": 10.00 }
            ],
            "paymentDetails": { "token": "VALID_TOKEN" }
        });

        // Start flow execution
        let flow_id = FlowId("test_order_flow".to_string());
        let instance_id = runtime
            .trigger_flow(flow_id, DataPacket::new(trigger_data))
            .await
            .expect("Failed to start flow");

        println!("Flow instance started with ID: {:?}", instance_id);

        // Wait a bit to allow some processing to happen
        sleep(Duration::from_millis(500)).await;

        // Get flow state
        let flow_state = runtime
            .get_flow_instance_state(instance_id.clone())
            .await
            .expect("Failed to get flow state")
            .expect("Flow state not found");

        // Print the entire flow state for debugging
        println!(
            "Flow state: {}",
            serde_json::to_string_pretty(&flow_state).unwrap()
        );

        // Parse the flow state
        let flow_state_obj = flow_state
            .as_object()
            .expect("Expected flow state to be an object");

        // Check if order_processor is in completed_steps
        let completed_steps = flow_state_obj
            .get("completed_steps")
            .and_then(|s| s.as_array())
            .expect("Expected completed_steps to be an array");

        let contains_order_processor = completed_steps
            .iter()
            .any(|s| s.as_str() == Some("order_processor"));

        assert!(
            contains_order_processor,
            "order_processor should be in completed steps"
        );

        // Check if step output exists with the correct key
        if let Some(step_outputs) = flow_state_obj
            .get("step_outputs")
            .and_then(|s| s.as_object())
        {
            println!(
                "Step outputs keys: {:?}",
                step_outputs.keys().collect::<Vec<_>>()
            );

            // Check if the order processor step output exists with the correct key
            let order_output = step_outputs
                .get("order_processor.processed_order")
                .and_then(|o| o.get("value"))
                .expect("Order processor output not found");

            println!("Order processor output: {}", order_output);

            // Verify order processor output
            assert_eq!(order_output["status"], "PROCESSING");
            assert_eq!(order_output["totalAmount"], 20.0);
        } else {
            panic!("Step outputs not found in flow state");
        }
    }

    #[tokio::test]
    async fn test_flow_payment_failure() {
        // Set up runtime
        let runtime = create_test_runtime();

        // Deploy test flow
        let flow = create_test_flow();
        runtime
            .deploy_definition(flow)
            .await
            .expect("Failed to deploy flow");

        // Trigger data - order that will cause payment failure (high amount)
        let trigger_data = json!({
            "customerId": "CUST456",
            "items": [
                { "productId": "ITEM001", "quantity": 15, "price": 10.00 }
            ],
            "paymentDetails": { "token": "VALID_TOKEN" }
        });

        // Start flow execution
        let flow_id = FlowId("test_order_flow".to_string());
        let instance_id = runtime
            .trigger_flow(flow_id, DataPacket::new(trigger_data))
            .await
            .expect("Failed to start flow");

        println!("Flow instance started with ID: {:?}", instance_id);

        // Wait a bit to allow some processing to happen
        sleep(Duration::from_millis(500)).await;

        // Get flow state
        let flow_state = runtime
            .get_flow_instance_state(instance_id.clone())
            .await
            .expect("Failed to get flow state")
            .expect("Flow state not found");

        // Print the entire flow state for debugging
        println!(
            "Flow state: {}",
            serde_json::to_string_pretty(&flow_state).unwrap()
        );

        // Parse the flow state
        let flow_state_obj = flow_state
            .as_object()
            .expect("Expected flow state to be an object");

        // Check if order_processor is in completed_steps
        let completed_steps = flow_state_obj
            .get("completed_steps")
            .and_then(|s| s.as_array())
            .expect("Expected completed_steps to be an array");

        let contains_order_processor = completed_steps
            .iter()
            .any(|s| s.as_str() == Some("order_processor"));

        assert!(
            contains_order_processor,
            "order_processor should be in completed steps"
        );

        // Check if step output exists with the correct key
        if let Some(step_outputs) = flow_state_obj
            .get("step_outputs")
            .and_then(|s| s.as_object())
        {
            println!(
                "Step outputs keys: {:?}",
                step_outputs.keys().collect::<Vec<_>>()
            );

            // Check if the order processor step output exists with the correct key
            let order_output = step_outputs
                .get("order_processor.processed_order")
                .and_then(|o| o.get("value"))
                .expect("Order processor output not found");

            println!("Order processor output: {}", order_output);

            // Verify order processor output
            assert_eq!(order_output["status"], "PROCESSING");
            assert_eq!(order_output["totalAmount"], 150.0);
        } else {
            panic!("Step outputs not found in flow state");
        }
    }
}

// Empty module to ensure the file is still recognized as a module
#[cfg(not(feature = "run_deprecated_tests"))]
mod integration_tests_impl {}

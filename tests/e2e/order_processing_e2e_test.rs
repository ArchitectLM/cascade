//! End-to-end test for order processing flow using cascade-test-utils
//!
//! This test demonstrates a complete E2E test for an order processing flow that:
//! 1. Validates order input
//! 2. Processes payment
//! 3. Creates order record
//! 4. Sends confirmation

use cascade_test_utils::{
    builders::TestServerBuilder,
    data_generators::create_sequential_flow_dsl,
    mocks::core_runtime::{FlowInstance, FlowState, create_mock_core_runtime_api},
};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::Duration;
use serde_json::{json, Value};

/// Mock Database for tracking order data
#[derive(Default)]
struct MockOrderDatabase {
    orders: Arc<Mutex<HashMap<String, Value>>>,
}

impl MockOrderDatabase {
    fn new() -> Self {
        Self {
            orders: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn insert_order(&self, order_id: &str, order_data: Value) {
        self.orders.lock().unwrap().insert(order_id.to_string(), order_data);
    }

    fn get_order(&self, order_id: &str) -> Option<Value> {
        self.orders.lock().unwrap().get(order_id).cloned()
    }
}

#[tokio::test]
async fn test_order_processing_flow_e2e() {
    // === SETUP: Create a test environment ===
    
    // Create a mock database for tracking orders
    let db = MockOrderDatabase::new();
    let db_clone = db.clone();
    
    // Generate a sequential flow DSL with custom components
    let flow_id = "order-processing-flow";
    let flow_dsl = create_sequential_flow_dsl(flow_id);
    
    println!("Created order processing flow definition");
    
    // Create and configure a mock core runtime
    let mut mock_core_runtime = create_mock_core_runtime_api();
    
    // Configure the mock core runtime to simulate successful deployment
    mock_core_runtime
        .expect_deploy_dsl()
        .withf(move |id, _| id == flow_id)
        .returning(|_, _| Ok(()));
    
    // Setup flow trigger response
    mock_core_runtime
        .expect_trigger_flow()
        .withf(move |id, _| id == flow_id)
        .returning(|_, _| Ok("test-instance-123".to_string()));
    
    // Setup flow state response with the proper FlowInstance type
    mock_core_runtime
        .expect_get_flow_state()
        .returning(move |instance_id| {
            // Create a completed flow state with order data
            let order_id = "ord-123456";
            
            // Store the order in our mock database
            db_clone.insert_order(order_id, json!({
                "order_id": order_id,
                "customer_id": "cust-123",
                "items": [
                    {
                        "product_id": "prod-456",
                        "name": "Wireless Headphones",
                        "quantity": 1,
                        "price": 79.99
                    },
                    {
                        "product_id": "prod-789",
                        "name": "USB-C Cable",
                        "quantity": 2,
                        "price": 24.99
                    }
                ],
                "payment_id": "pay-12345",
                "status": "created",
                "created_at": "2023-06-15T10:30:00Z"
            }));
            
            // Create data map with step outputs
            let mut data = HashMap::new();
            data.insert("step1.response".to_string(), json!({
                "statusCode": 200,
                "body": {
                    "status": "valid",
                    "message": "Order validation successful"
                }
            }));
            
            data.insert("step2.response".to_string(), json!({
                "statusCode": 200,
                "body": {
                    "status": "success",
                    "payment_id": "pay-12345",
                    "amount": 129.97,
                    "timestamp": "2023-06-15T10:30:00Z"
                }
            }));
            
            data.insert("step3.response".to_string(), json!({
                "statusCode": 201,
                "body": {
                    "order_id": order_id,
                    "status": "created"
                }
            }));
            
            // Return a flow instance with all steps completed
            Ok(Some(FlowInstance {
                instance_id: instance_id.to_string(),
                flow_id: flow_id.to_string(),
                state: FlowState::Completed,
                data,
            }))
        });
    
    // Start a test server using our mock core runtime
    let server = TestServerBuilder::new()
        .with_live_log(true)
        .with_core_runtime(mock_core_runtime)
        .build()
        .await
        .unwrap();
    
    println!("Test server started on: {}", server.base_url);
    
    // === DEPLOY: Deploy the flow ===
    
    // Deploy the flow
    let deploy_result = server.core_runtime
        .deploy_dsl(flow_id, &flow_dsl)
        .await;
    
    assert!(deploy_result.is_ok(), "Failed to deploy flow: {:?}", deploy_result);
    println!("Flow deployed successfully");
    
    // === TEST: Execute the order processing flow end-to-end ===
    
    // Prepare test order data
    let order_data = json!({
        "customer_id": "cust-123",
        "items": [
            {
                "product_id": "prod-456",
                "name": "Wireless Headphones",
                "quantity": 1,
                "price": 79.99
            },
            {
                "product_id": "prod-789",
                "name": "USB-C Cable",
                "quantity": 2,
                "price": 24.99
            }
        ],
        "payment": {
            "method": "credit_card",
            "card_token": "tok_visa_testcard",
            "amount": 129.97
        },
        "shipping_address": {
            "street": "123 Main St",
            "city": "Anytown",
            "state": "CA",
            "zip": "94043",
            "country": "US"
        }
    });
    
    // Make the API request to trigger the flow
    let response = server.client
        .post(&format!("{}/api/trigger/{}", server.base_url, flow_id))
        .json(&order_data)
        .send()
        .await
        .expect("Failed to send request");
    
    // Verify the response status
    assert_eq!(response.status().as_u16(), 200, "Expected 200 OK response");
    
    // Extract the flow instance ID
    let response_json = response.json::<serde_json::Value>()
        .await
        .expect("Failed to parse response JSON");
    
    let instance_id = response_json["instanceId"]
        .as_str()
        .expect("Response missing instanceId")
        .to_string();
    
    println!("Flow instance started with ID: {}", instance_id);
    
    // === VERIFY: Poll for flow completion and verify results ===
    
    // Poll until the flow completes or timeout (with a maximum wait time)
    let max_wait_ms = 5000;
    let mut elapsed_ms = 0;
    let poll_interval_ms = 100;
    
    let mut flow_instance = None;
    
    while elapsed_ms < max_wait_ms {
        // Get the current flow state
        let state_result = server.core_runtime
            .get_flow_state(&instance_id)
            .await;
            
        if let Ok(Some(instance)) = state_result {
            // Check the flow state
            match instance.state {
                FlowState::Completed => {
                    flow_instance = Some(instance);
                    break;
                },
                FlowState::Failed(ref error) => {
                    panic!("Flow failed with error: {}", error);
                },
                _ => {
                    println!("Flow state: {:?}, waiting...", instance.state);
                }
            }
        }
        
        // Wait before polling again
        tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
        elapsed_ms += poll_interval_ms;
    }
    
    let flow_instance = flow_instance.expect("Flow did not complete within timeout");
    
    // Verify flow completed successfully
    assert_eq!(flow_instance.state, FlowState::Completed, "Flow did not complete successfully");
    
    // Verify step outputs from the data map
    let step_outputs = &flow_instance.data;
    
    // Check payment response
    assert!(step_outputs.contains_key("step2.response"), 
        "Missing payment step output");
    
    let payment_response = &step_outputs["step2.response"];
    assert_eq!(payment_response["statusCode"], 200, "Payment HTTP status code mismatch");
    assert_eq!(
        payment_response["body"]["payment_id"], 
        "pay-12345", 
        "Payment ID mismatch"
    );
    
    // Check order creation result
    assert!(step_outputs.contains_key("step3.response"), 
        "Missing order creation step output");
    
    let order_result = &step_outputs["step3.response"]["body"];
    let order_id = order_result["order_id"].as_str().expect("Missing order ID");
    
    // === VERIFY: Check side effects (database records) ===
    
    // Check our mock database to verify the order was stored
    let stored_order = db.get_order(order_id)
        .expect("Order not found in database");
    
    // Verify the stored order contains the correct data
    assert_eq!(stored_order["customer_id"], "cust-123", "Customer ID mismatch");
    assert_eq!(stored_order["payment_id"], "pay-12345", "Payment ID mismatch");
    assert_eq!(stored_order["status"], "created", "Order status mismatch");
    
    // Verify the items were stored correctly
    let stored_items = stored_order["items"].as_array().expect("Items not stored as array");
    assert_eq!(stored_items.len(), 2, "Expected 2 items");
    
    // === CLEANUP: Any test-specific cleanup ===
    // In this case, the TestServer will automatically clean up when dropped
    
    println!("E2E test completed successfully");
}

// Helper implementation for cloning the MockOrderDatabase
impl Clone for MockOrderDatabase {
    fn clone(&self) -> Self {
        Self {
            orders: self.orders.clone(),
        }
    }
} 
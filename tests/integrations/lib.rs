// Cascade Integration Tests
//
// This crate contains integration tests for the Cascade platform

/// Utility functions and shared test infrastructure for integration tests
pub mod utils {
    use cascade_test_utils::{
        builders::TestServerBuilder,
        builders::TestServerHandles,
        data_generators::create_http_flow_dsl,
    };
    use serde_json::json;
    
    /// Creates a test server with default settings
    pub async fn create_test_server() -> TestServerHandles {
        TestServerBuilder::new()
            .with_live_log(false) // Disable live logging for cleaner test output
            .build()
            .await
            .unwrap()
    }
    
    /// Deploys a basic HTTP flow for testing
    /// This is a placeholder implementation that simulates a successful deployment
    pub async fn deploy_basic_flow(_server: &TestServerHandles, flow_id: &str) -> Result<(), String> {
        // Generate the flow DSL - in a real test this would be deployed to the runtime
        let _flow_dsl = create_http_flow_dsl(
            flow_id,
            "https://api.example.com/data", 
            "GET"
        );
        
        // For the test structure fixes, we'll simulate a successful deployment
        println!("Simulated successful deployment of flow: {}", flow_id);
        
        // Return success
        Ok(())
    }
    
    /// Helper to create a standard test order for testing
    pub fn create_test_order(customer_id: &str) -> serde_json::Value {
        json!({
            "order_id": format!("order-{}", rand::random::<u32>()),
            "customer_id": customer_id,
            "items": [
                {
                    "product_id": "prod-001",
                    "quantity": 2,
                    "price": 10.99
                },
                {
                    "product_id": "prod-002",
                    "quantity": 1,
                    "price": 24.99
                }
            ],
            "total": 46.97,
            "created_at": "2023-05-01T12:00:00Z"
        })
    }
} 
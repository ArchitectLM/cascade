//! Standard step definitions for Cascade BDD tests

use crate::bdd::world::{CascadeWorld, MockOrderDatabase};
use crate::builders::TestServerBuilder;
use crate::mocks::component_behaviors::{HttpResponseData, simulate_http_response};
use cucumber::{given, then, when};
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::info;

use super::world::CascadeWorld;

// Maximum time to wait for flow execution to complete
const FLOW_EXECUTION_TIMEOUT_MS: u64 = 5000;

/// Server setup steps

/// Setup a Cascade server with default mock components
#[given(expr = "a Cascade server")]
pub async fn setup_cascade_server(world: &mut CascadeWorld) {
    let mut server_builder = TestServerBuilder::default();

    // Configure the server based on real component flags
    if world.use_real_core {
        #[cfg(feature = "real_core")]
        {
            server_builder = server_builder.with_real_core(true);
        }
        #[cfg(not(feature = "real_core"))]
        {
            panic!("Cannot use real core runtime: feature 'real_core' is not enabled");
        }
    }

    if world.use_real_edge {
        #[cfg(feature = "real_edge")]
        {
            server_builder = server_builder.with_real_edge(true);
        }
        #[cfg(not(feature = "real_edge"))]
        {
            panic!("Cannot use real edge platform: feature 'real_edge' is not enabled");
        }
    }

    if world.use_real_content_store {
        #[cfg(feature = "real_content_store")]
        {
            server_builder = server_builder.with_real_content_store(true);
        }
        #[cfg(not(feature = "real_content_store"))]
        {
            panic!("Cannot use real content store: feature 'real_content_store' is not enabled");
        }
    }
    
    // Set up the order database for the mock components (still needed for tests)
    // The TestServerBuilder doesn't yet have a method to set the order database
    // This would need to be added if/when needed
    // server_builder = server_builder.with_order_database(world.db.clone());
    
    // Configure payment service behaviors
    // The TestServerBuilder doesn't yet have a method to configure payment service
    // This would need to be added if/when needed
    // server_builder = server_builder
    //     .payment_service_config(|config| {
    //         config.decline_next_payment = world.payment_config.decline_next;
    //         config.timeout_next_payment = world.payment_config.timeout_next;
    //     });

    // Build and start the server
    let server = server_builder.build().await.expect("Failed to build test server");
    world.server_handle = Some(server);
}

/// Setup a Cascade server with real core runtime
#[given(expr = "a Cascade server with real core runtime")]
pub async fn setup_cascade_server_with_real_core(world: &mut CascadeWorld) {
    world.use_real_core = true;
    setup_cascade_server(world).await;
}

/// Setup a Cascade server with real edge platform
#[given(expr = "a Cascade server with real edge platform")]
pub async fn setup_cascade_server_with_real_edge(world: &mut CascadeWorld) {
    world.use_real_edge = true;
    setup_cascade_server(world).await;
}

/// Setup a Cascade server with real content store
#[given(expr = "a Cascade server with real content store")]
pub async fn setup_cascade_server_with_real_content_store(world: &mut CascadeWorld) {
    world.use_real_content_store = true;
    setup_cascade_server(world).await;
}

/// Setup a Cascade server with all real components
#[given(expr = "a Cascade server with all real components")]
pub async fn setup_cascade_server_with_all_real_components(world: &mut CascadeWorld) {
    world.use_real_core = true;
    world.use_real_edge = true;
    world.use_real_content_store = true;
    setup_cascade_server(world).await;
}

/// Core assertion steps

#[then(expr = "the HTTP response status code should be {int}")]
pub fn check_http_status_code(world: &mut CascadeWorld, status_code: i32) {
    let status = world.response_status.expect("No response status available");
    assert_eq!(status.as_u16() as i32, status_code, 
               "Expected HTTP status code {}, got {}", status_code, status);
}

#[then(expr = r#"the HTTP response body field "{word}" should be "{word}""#)]
pub fn check_response_body_field_value(world: &mut CascadeWorld, field: String, expected_value: String) {
    let body = world.response_body.as_ref().expect("No response body available");
    
    let field_value = body.get(&field).expect(&format!("Field '{}' not found in response", field));
    let field_str = field_value.as_str().expect(&format!("Field '{}' is not a string", field));
    
    assert_eq!(field_str, expected_value, 
               "Expected field '{}' to be '{}', got '{}'", field, expected_value, field_str);
}

#[then(expr = r#"the HTTP response body should contain "{word}""#)]
pub fn check_response_body_contains(world: &mut CascadeWorld, expected_text: String) {
    let body = world.response_body.as_ref().expect("No response body available");
    let body_str = serde_json::to_string(body).expect("Failed to serialize response body");
    
    assert!(body_str.contains(&expected_text), 
            "Expected response body to contain '{}', but it doesn't: {}", expected_text, body_str);
}

/// Helper function to wait for flow completion
pub async fn wait_for_flow_completion(world: &mut CascadeWorld) -> Result<(), String> {
    let server = world.server_handle.as_ref().unwrap();
    let instance_id = world.instance_id.as_ref().unwrap();
    
    // Poll until the flow completes or timeout
    timeout(Duration::from_millis(FLOW_EXECUTION_TIMEOUT_MS), async {
        let mut delay_ms = 50;
        let max_delay_ms = 500;
        
        loop {
            // Get the current flow state
            let state_result = server.core_runtime
                .get_flow_state(instance_id)
                .await;
                
            match state_result {
                Ok(Some(state)) => {
                    // Check if flow has reached a terminal state
                    let state_obj = state.as_object().unwrap();
                    let status = state_obj.get("state").and_then(|s| s.as_str()).unwrap_or("");
                    if status == "COMPLETED" || status == "FAILED" || status == "ERROR" {
                        return Ok(());
                    }
                    
                    // If still running, wait and retry
                    println!("Flow state: {}, waiting...", status);
                },
                Ok(None) => {
                    return Err(format!("Flow instance {} not found", instance_id));
                },
                Err(e) => {
                    return Err(format!("Error getting flow state: {:?}", e));
                }
            }
            
            // Implement exponential backoff
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            
            // Increase delay for next iteration (with a maximum limit)
            delay_ms = (delay_ms * 2).min(max_delay_ms);
        }
    }).await.map_err(|_| "Flow execution timed out".to_string())?
} 
# Cascade Testing Utilities Guide

This guide provides an overview of the testing utilities available in the Cascade platform for both integration and BDD (Behavior-Driven Development) testing.

## Overview

The `cascade-test-utils` crate provides utilities for testing the Cascade platform components and workflows. These utilities are designed to be used in both integration tests and BDD tests.

## Available Utilities

### Test Server

The `TestServer` provides a fully-functional Cascade server for testing:

```rust
use cascade_test_utils::server::TestServer;
use cascade_test_utils::builders::TestServerBuilder;

// Create a test server with default configuration
let server = TestServerBuilder::new()
    .with_live_log(true)
    .build()
    .await
    .unwrap();

// Use the server to deploy a flow
server.core_runtime
    .deploy_dsl("test-flow", flow_dsl)
    .await
    .unwrap();
```

### MockComponentExecutor

The `MockComponentExecutor` allows mocking component behavior:

```rust
use cascade_test_utils::mocks::component_executor::MockComponentExecutor;
use cascade_test_utils::mocks::component_behaviors::HttpResponseData;

// Create a mock component executor
let mut executor = MockComponentExecutor::new();

// Configure behavior
executor.expect_execute()
    .returning(|runtime_api| {
        Box::pin(async move {
            // Simulate a successful HTTP response
            let response = HttpResponseData::new(200, json!({"success": true}));
            simulate_http_response(runtime_api, response).await
        })
    });
```

### Data Generators

Generate test data for flows:

```rust
use cascade_test_utils::data_generators::{
    create_minimal_flow_dsl,
    create_http_flow_dsl,
};

// Create a minimal flow for testing
let minimal_flow = create_minimal_flow_dsl();

// Create an HTTP flow with specific parameters
let http_flow = create_http_flow_dsl(
    "http-test-flow",
    "https://example.com/api",
    "GET"
);
```

### Resilience Patterns

Test resilience patterns like circuit breakers:

```rust
use cascade_test_utils::resilience::CircuitBreaker;
use cascade_test_utils::resilience::circuit_breaker::CircuitBreakerConfig;

// Create a circuit breaker
let config = CircuitBreakerConfig {
    failure_threshold: 3,
    reset_timeout_ms: 500,
    half_open_max_calls: 1,
};
let circuit_breaker = CircuitBreaker::new(config);

// Execute with circuit breaker protection
let result = circuit_breaker.execute(|| async {
    // Call potentially failing operation
    make_api_call().await
}).await;
```

## BDD Testing Integration

### Step Definitions

Use test utilities in BDD step definitions:

```rust
use cucumber::{given, when, then};
use cascade_bdd_tests::CascadeWorld;
use cascade_test_utils::builders::TestServerBuilder;

#[given(regex = r"the Cascade server is running")]
async fn setup_server(world: &mut CascadeWorld) {
    // Create a test server
    let server = TestServerBuilder::new()
        .with_live_log(true)
        .build()
        .await
        .unwrap();
    
    world.server = Some(server);
}

#[when(regex = r"I deploy a flow named \"(.*)\"")]
async fn deploy_flow(world: &mut CascadeWorld, flow_id: String) {
    let server = world.server.as_ref().unwrap();
    let flow_dsl = create_minimal_flow_dsl();
    
    server.core_runtime
        .deploy_dsl(&flow_id, &flow_dsl)
        .await
        .unwrap();
    
    world.flow_id = Some(flow_id);
}
```

### The World

The `CascadeWorld` struct holds shared state between step definitions:

```rust
use cucumber::World;
use cascade_test_utils::server::TestServer;

#[derive(Debug, World)]
pub struct CascadeWorld {
    pub server: Option<TestServer>,
    pub flow_id: Option<String>,
    pub instance_id: Option<String>,
    // Other state fields...
}
```

## End-to-End Testing

For end-to-end testing, combine the test server with actual HTTP calls:

```rust
use cascade_test_utils::builders::TestServerBuilder;
use reqwest::Client;

// Start a test server
let server = TestServerBuilder::new()
    .with_live_log(true)
    .build()
    .await
    .unwrap();

// Deploy a flow
server.core_runtime
    .deploy_dsl("e2e-test-flow", flow_dsl)
    .await
    .unwrap();

// Make an actual HTTP call to the server
let client = Client::new();
let response = client
    .post(&format!("{}/flows/e2e-test-flow/trigger", server.base_url))
    .json(&json!({"test": "data"}))
    .send()
    .await
    .unwrap();

// Verify the response
assert_eq!(response.status(), 200);
```

## Mock Clock

For time-dependent tests, use the SimpleTimerService:

```rust
use cascade_test_utils::implementations::simple_timer_service::SimpleTimerService;
use std::time::Duration;

// Create a timer service
let timer_service = SimpleTimerService::new();

// Schedule a timer
timer_service.schedule_timer(
    500,  // 500ms
    Box::new(|| Box::pin(async { 
        println!("Timer fired");
        Ok(())
    })),
).await.unwrap();

// Advance time to trigger the timer
timer_service.advance_time(Duration::from_millis(600)).await.unwrap();
```

## Best Practices

1. **Isolated Tests**: Each test should create its own test server to avoid state interference between tests.
2. **Cleanup**: Use `tokio::time::timeout` to ensure tests don't hang due to waiting for async operations.
3. **Mock External Services**: Use MockComponentExecutor to mock external service behavior.
4. **Descriptive Assertions**: Use descriptive assertion messages to clarify test failures.
5. **Reusable Steps**: Write step definitions that can be reused across multiple feature files.

## Conclusion

The `cascade-test-utils` crate provides a comprehensive set of utilities for testing the Cascade platform. These utilities make it easier to write integration tests, BDD tests, and end-to-end tests. 
# BDD Testing with cascade-test-utils

This document describes how to use the BDD (Behavior-Driven Development) testing capabilities provided by the `cascade-test-utils` crate.

## Prerequisites

To use the BDD testing capabilities, you need to enable the `bdd` feature in your `Cargo.toml`:

```toml
cascade-test-utils = { path = "../cascade-test-utils", features = ["bdd"] }
```

## Core Components

### The CascadeWorld

The `CascadeWorld` struct is the central component for BDD testing. It provides:

- A test server for sending requests to Cascade flows
- Storage for HTTP request and response data
- A mock database for orders and inventory
- Storage for flow execution state
- Configuration for simulating different scenarios (e.g., payment failures)

```rust
use cascade_test_utils::bdd::CascadeWorld;

let world = CascadeWorld::default();
```

### Running Tests

The BDD module provides functions for running tests:

```rust
use cascade_test_utils::bdd::{CascadeWorld, run_scenario, run_features};
use std::path::PathBuf;

// Run a specific scenario
let world = CascadeWorld::default();
let feature_file = PathBuf::from("features/order_processing.feature");
let scenario_name = "Successfully process a valid order";
run_scenario(world, &feature_file, Some(scenario_name)).await;

// Or run all features in a directory
let features_dir = PathBuf::from("features");
run_features(world, &features_dir).await;
```

### Step Definitions

The BDD module provides common step definitions:

```gherkin
Given the Cascade server is running
Then the HTTP response status code should be 200
Then the HTTP response body field "status" should be "success"
Then the HTTP response body should contain "order_id"
```

## Creating Custom Steps

You can create custom step definitions using the Cucumber macros:

```rust
use cascade_test_utils::bdd::CascadeWorld;
use cucumber::{given, when, then};

#[given(expr = r#"I have a customer with ID "{word}""#)]
fn set_customer_id(world: &mut CascadeWorld, customer_id: String) {
    world.customer_id = Some(customer_id);
}

#[when(expr = r#"I submit an order for product "{word}" with quantity {int}"#)]
async fn submit_order(world: &mut CascadeWorld, product_id: String, quantity: i32) {
    // Your step implementation here
}

#[then("the order should be confirmed")]
fn check_order_confirmed(world: &mut CascadeWorld) {
    // Your assertion implementation here
}
```

## Testing Patterns

### 1. Setting Up Test Data

```gherkin
Given I have a customer with ID "cust-123"
And product "prod-001" is in stock with quantity "50" and price "10.99"
```

### 2. Performing Actions

```gherkin
When I create an order with the following details:
  | customerId | productId | quantity | price  |
  | cust-123   | prod-001  | 2        | 10.99  |
```

### 3. Checking Results

```gherkin
Then I should receive a successful order response
And the order should be saved in the database
And the inventory for product "prod-001" should be reduced by "2"
```

### 4. Testing Error Scenarios

```gherkin
Given the payment system will decline the next transaction
When I create an order with the following details:
  | customerId | productId | quantity | price  |
  | cust-123   | prod-001  | 2        | 10.99  |
Then I should receive an error response with status 402
And the error code should be "ERR_COMPONENT_PAYMENT_DECLINED"
```

## Advanced Features

### Custom Component Behaviors

The test utils provide helpers for simulating component behaviors:

```rust
use cascade_test_utils::mocks::component_behaviors::{HttpResponseData, simulate_http_response};
use cascade_test_utils::mocks::component::{ComponentRuntimeAPI, DataPacket};
use serde_json::json;

// In your component executor mock:
let mock_runtime = Arc::new(create_mock_component_runtime_api("http-component"));
let response = HttpResponseData::new(200, json!({"order_id": "ord-123"}))
    .with_headers(json!({"content-type": "application/json"}));

// Simulate an HTTP response
simulate_http_response(mock_runtime, response).await?;

// Or with latency
simulate_http_response_with_latency(mock_runtime, response, 100).await?;
```

### Flow State Tracking

The BDD utilities can track flow state and wait for completion:

```rust
// After triggering a flow:
match wait_for_flow_completion(world).await {
    Ok(_) => println!("Flow completed successfully"),
    Err(e) => println!("Flow execution error or timeout: {}", e),
}
```

## Best Practices

1. **Keep scenarios focused**: Each scenario should test one specific behavior
2. **Reuse step definitions**: Create reusable steps to avoid duplication
3. **Use backgrounds for common setup**: Move common setup steps to a Background section
4. **Tag scenarios for organization**: Use tags like `@order_processing`, `@happy_path`, `@error_case`
5. **Keep step definitions simple**: Focus on a single action or assertion per step
6. **Use clear, business-focused language**: Write scenarios in terms that make sense to non-technical stakeholders

## Example

```gherkin
Feature: Order Processing

  Background:
    Given the Cascade server is running
    And I have a customer with ID "cust-123"
    
  Scenario: Process a valid order
    Given product "prod-001" is in stock with quantity "50" and price "10.99"
    When I create an order with the following details:
      | customerId | productId | quantity | price  |
      | cust-123   | prod-001  | 2        | 10.99  |
    Then I should receive a successful order response
    And the order should be saved in the database
    And the inventory for product "prod-001" should be reduced by "2"
``` 
# BDD Testing Guide for Cascade

This guide explains how to write and run Behavior-Driven Development (BDD) tests for the Cascade platform using the Cucumber Rust framework.

## Current Status

The BDD test suite has been fixed to compile and run with Cucumber 0.19.1. 

✅ Working:
- **Minimal Test**: (`minimal_test.rs`) - A simple counter example
- **Simple Cucumber**: (`simple_cucumber.rs`) - Basic example with temporary feature file
- **Circuit Breaker Test**: (`circuit-breaker-test.rs`) - Circuit breaker pattern example

⚠️ Partially Working:
- **Circuit Breaker Feature**: Feature compiles but has some assertion failures
- **Simple Order Feature**: Flow deployment and some API calls need mocking

## Core Problems Fixed

1. **Table Issues**: Steps that use tables need special handling:
   - Avoid using `cucumber::step::Context`
   - Instead, use explicit parameters in the step definitions
   - Use normal function parameters like `String`, `i32`, etc.

2. **World Implementation**: 
   - The World struct must have a proper Default implementation
   - Avoid derive attributes for Default and provide an explicit implementation

3. **Execution Path**:
   - Use a simplified runner pattern: `MyWorld::cucumber().run(features_dir).await`
   - Make sure to correctly set the features directory path

4. **Network Calls**:
   - Avoid making actual network calls in tests
   - Use mock implementations for API clients
   - Don't try to store Response objects directly (they don't implement Clone)
   - Save response data before consuming the response object with methods like `json()`

## Working with Cucumber 0.19.1

The Cascade project uses cucumber 0.19.1, which has specific requirements:

1. The World struct must implement Default trait correctly
2. Step functions should use explicit parameter types (no Context)
3. Avoid tables where possible - use parameters instead
4. Use multiline string parameters for longer text

## Writing Feature Files

Feature files should be placed in the `features/` directory. Example:

```gherkin
Feature: Simple Order Processing
  Background:
    Given the Cascade server is running
    And the order processing system is initialized
    And I have a customer with ID "cust-001"
    And a flow "simple-order" is deployed with DSL:
    """
    flow "simple-order" {
      start {
        validate_order()
        process_order()
        confirm_order()
      }
    }
    """

  Scenario: Successful order submission
    When I create an order with product prod-001 and quantity 2
    Then I should receive a successful order response
```

## Writing Step Definitions

Step functions should match the steps in feature files:

```rust
#[given(expr = r#"I have a customer with ID "{word}""#)]
fn set_customer_id(world: &mut CascadeWorld, customer_id: String) {
    world.customer_id = Some(customer_id);
}

#[when(expr = "I create an order with product {word} and quantity {int}")]
async fn create_order_with_product_quantity(world: &mut CascadeWorld, product_id: String, quantity: i32) {
    // Implementation...
    
    // Use mock responses for testing
    world.response_status = Some(reqwest::StatusCode::OK);
    world.order_id = Some("test-order-id".to_string());
}
```

## Important: Handling Response Objects

When working with HTTP responses, follow these guidelines:

1. **Never store Response objects**: They don't implement Clone and get consumed by methods like `json()`
2. **Extract data early**: Get status codes and headers before calling consuming methods
3. **Store extracted data**: Save values in the World object, not the Response itself
4. **Example pattern**:
   ```rust
   // Correct way to handle responses
   match client.send_request().await {
       Ok(response) => {
           // Extract and store status first
           let status = response.status();
           world.response_status = Some(status);
           
           // Then consume the response to get the body
           if let Ok(body) = response.json::<Value>().await {
               world.response_body = Some(body);
           }
       },
       Err(e) => { /* Handle error */ }
   }
   ```

## Testing

### Running Tests

Run the individual test binaries:

```bash
# Minimal test
cargo run --bin minimal_test

# Simple cucumber test
cargo run --bin simple_cucumber

# Circuit breaker test
cargo run --bin circuit-breaker-test
```

## Troubleshooting

### Common Issues

1. **Failed to parse feature**: Check path to feature files
2. **Step skipped/failed/undefined**: Check step definition and parameters
3. **No such function**: Update step function declarations to match Cucumber 0.19.1
4. **Compilation errors with Response.clone()**: Don't try to clone Response objects, just store their data
5. **"borrow of moved value"**: Response methods like `json()` consume the response - extract all needed data before calling these methods

### Tables in Step Definitions

The table functionality in Cucumber 0.19.1 is tricky. When possible, avoid tables and use:

1. Simple parameters: `{word}`, `{int}`, `{float}`
2. Multiline strings with triple quotes
3. JSON-formatted strings

## Reference Examples

- **Standalone example**: See `minimal_test.rs` for a simple example
- **Working steps**: See the fixed `circuit_breaker_steps.rs` functions
- **API client**: See the API client pattern in `api_client.rs`

## Next Steps

1. Fix flow deployment in server_steps.rs
2. Update order_steps.rs to use the new parameter approach
3. Add more comprehensive mocks for network calls
4. Make the main test runner support feature filtering 
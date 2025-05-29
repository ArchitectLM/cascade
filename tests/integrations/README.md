# Cascade Test Utils Examples

This directory contains example tests demonstrating how to use the `cascade-test-utils` crate with various Cascade crates. These examples show how to effectively use the testing utilities in different contexts.

## Examples Overview

### Core Functionality Examples (`core_example.rs`)

Shows how to use the test utilities with `cascade-core`:
- Testing flow execution with HTTP components
- Testing components in isolation
- Testing content storage operations

### DSL Examples (`dsl_example.rs`)

Demonstrates using the test utilities with `cascade-dsl`:
- Testing validation of different flow DSL patterns
- Using assertion helpers to validate parsed manifests
- Testing error detection in flow DSL

### Server Examples (`server_example.rs`)

Shows how to test `cascade-server` endpoints and functionality:
- Testing flow deployment endpoints
- Testing flow trigger endpoints
- Testing health and content storage endpoints
- Testing error handling

### Edge Resilience Examples (`edge_resilience_example.rs`)

Demonstrates testing resilience patterns with `cascade-edge`:
- Testing circuit breaker patterns
- Testing bulkhead patterns
- Using SimpleTimerService for time-dependent tests

### Monitoring Examples (`monitoring_example.rs`)

Shows how to use the test utilities for testing monitoring functionality:
- Testing metric collection for flow execution
- Testing metric collection for multiple flows
- Testing error scenario metrics

## Running the Examples

To run these examples, you need to have Rust and Cargo installed. Then you can use the following command to run a specific example:

```bash
cargo test --test using-test-utils/core_example
cargo test --test using-test-utils/dsl_example
cargo test --test using-test-utils/server_example
cargo test --test using-test-utils/edge_resilience_example
cargo test --test using-test-utils/monitoring_example
```

Or run all examples with:

```bash
cargo test --test using-test-utils
```

## Key Testing Techniques Demonstrated

1. **Mock Usage**
   - Creating and configuring mock components
   - Setting up expectations and behaviors

2. **Test Server Setup**
   - Creating a TestServerBuilder with custom configuration
   - Using the TestServer for integration testing

3. **Data Generators**
   - Generating test flow DSL
   - Creating component configurations

4. **Assertion Helpers**
   - Validating flow state
   - Validating manifest structure

5. **Time Control**
   - Using SimpleTimerService to control time in tests
   - Testing time-dependent behaviors

## Integration with Cargo

These examples are designed to work as part of your Cargo test suite. To add them to your project's test suite, ensure your `Cargo.toml` is configured to include test examples.

## Further Resources

- See the `cascade-test-utils` README.md for more detailed documentation
- Refer to the MIGRATION_GUIDE.md for guidelines on migrating existing tests
- Check the API documentation for all available utilities

# Cascade Integration Tests

This directory contains integration tests for the Cascade project. These tests demonstrate how to use various components of the Cascade system together.

## Test Files

- **dsl_example.rs**: Tests for the domain-specific language (DSL) parser and validator
- **edge_resilience_example.rs**: Tests for edge resilience patterns like circuit breakers and bulkheads
- **monitoring_example.rs**: Tests for flow monitoring and metrics collection
- **server_example.rs**: Tests for the HTTP server APIs and endpoints
- **core_example.rs**: Tests for core functionality of the Cascade system

## Key Fixes Implemented

### DSL Example Fixes
- Added proper `serde` and `serde_yaml` dependencies to handle serialization/deserialization
- Implemented `Serialize` and `Deserialize` traits for mock types
- Fixed YAML parsing logic that was incorrectly using JSON parsing
- Created a proper `to_json()` method to handle flow_id generation
- Fixed circular dependency detection in flow validation

### Edge Resilience Example Fixes
- Fixed Rust borrow checker issues with `Mutex` by using `Arc<Mutex<>>` consistently
- Improved `SimpleTimerService` implementation to correctly handle timer callbacks
- Used `std::mem::take()` to safely move data out of a mutex
- Implemented proper `Clone` trait for mock types
- Fixed async/sync mismatches in the mock expectation handlers

### Monitoring Example Fixes
- Implemented `Clone` trait for `HttpResponseData`
- Fixed closure ownership issues with response data by using primitives
- Used primitive values instead of complex types in closures to avoid ownership issues
- Fixed variable mutability issues

### Server Example Fixes
- Created custom HTTP status code and response types
- Fixed mock HTTP client to correctly handle different URL paths
- Improved the response handling in `MockRequestBuilder`
- Updated predefined mock responses to match test expectations

## Common Patterns Used

1. **Mutex Handling**: Using `std::sync::Mutex` with `Arc` for safe concurrent access
2. **Closure Safety**: Capturing only primitive or cloneable values in closures
3. **Trait Implementation**: Implementing traits like `Clone`, `Serialize`, and `Deserialize` for custom types
4. **Mock Patterns**: Creating effective mocks that simulate real component behavior
5. **Async Pattern**: Proper handling of async/await in test code

## Running Tests

To run all tests:
```
cargo test
```

To run a specific test file:
```
cargo test --test <test_file_name>
```

For example:
```
cargo test --test edge_resilience_example
```

To see test output:
```
cargo test -- --nocapture
``` 
// Main entry point for the cascade-test-utils examples
//
// This file serves as the main entry point for the test examples,
// making it easier to run all tests using `cargo test --test using-test-utils`.

mod core_example;
mod dsl_example;
mod server_example;
mod edge_resilience_example;
mod monitoring_example;
mod retry_component_test;
mod timer_service_test;

// Empty test to ensure cargo recognizes this as a test file
#[test]
fn test_examples_loaded() {
    // This test doesn't do anything, it just ensures cargo recognizes this file
    // as a test file and loads all the modules.
    assert!(true);
} 
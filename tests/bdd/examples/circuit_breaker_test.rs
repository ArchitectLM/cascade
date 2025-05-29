//! Example showing how to run the circuit breaker BDD test
//!
//! This example runs the circuit breaker BDD tests using the cascade-test-utils framework

use cascade_bdd_tests::CascadeWorld;
use cucumber::{writer, World};

#[tokio::main]
async fn main() {
    // Initialize logger for better debug output
    pretty_env_logger::init();
    
    // Path to the specific feature file
    let feature_path = "tests/bdd/features/circuit_breaker.feature";
    
    // Run the test with basic console output
    CascadeWorld::cucumber()
        .run(feature_path)
        .await;
} 
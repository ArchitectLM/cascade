//! Example showing how to run a specific BDD test
//!
//! This runs a single BDD scenario using the cascade-test-utils BDD framework
//! Note: Requires the `bdd` feature to be enabled: cascade-test-utils = { path = "../crates/cascade-test-utils", features = ["bdd"] }

#[cfg(feature = "bdd")]
use cascade_test_utils::bdd::{CascadeWorld, run_scenario};

#[cfg(feature = "bdd")]
use std::path::PathBuf;

#[cfg(feature = "bdd")]
#[tokio::main]
async fn main() {
    // Initialize logger
    pretty_env_logger::init();
    
    // Create a new test world
    let world = CascadeWorld::default();
    
    // Path to the specific feature file
    let feature_file = PathBuf::from("tests/bdd/features/standard_order_processing.feature");
    
    // Optional: specific scenario name (if not provided, all scenarios in the file run)
    let scenario_name = "Successfully process a valid order";
    
    // Run the specific scenario
    run_scenario(world, &feature_file, Some(scenario_name)).await;
}

#[cfg(not(feature = "bdd"))]
fn main() {
    eprintln!("This example requires the 'bdd' feature to be enabled. Please recompile with: --features bdd");
} 
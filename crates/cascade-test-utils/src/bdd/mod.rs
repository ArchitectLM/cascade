//! BDD testing utilities for the Cascade Platform
//!
//! This module provides utilities for BDD-style testing using cucumber-rs.

mod world;
pub use world::*;

mod steps;
pub use steps::*;

use cucumber::{writer, World, AfterHook, ScenarioContext};
use std::path::Path;

/// Run a specific BDD scenario or all scenarios in a feature file
pub async fn run_scenario<W: World + Default + std::fmt::Debug + cucumber::codegen::WorldInventory>(
    _world: W,
    feature_file: &Path,
    scenario_name: Option<&str>,
) {
    let config = W::cucumber()
        .with_writer(writer::Basic::stdout())
        .after(cleanup_after_scenario);
    
    // Run the scenario(s)
    if let Some(name) = scenario_name {
        config.run_and_exit(&[
            "--name", name,
            feature_file.to_str().unwrap(),
        ]).await;
    } else {
        config.run_and_exit(&[feature_file.to_str().unwrap()]).await;
    }
}

/// Run all feature files in a directory
pub async fn run_features<W: World + Default + std::fmt::Debug + cucumber::codegen::WorldInventory>(
    _world: W,
    features_dir: &Path,
) {
    W::cucumber()
        .with_writer(writer::Basic::stdout())
        .after(cleanup_after_scenario)
        .run_and_exit(&[features_dir.to_str().unwrap()])
        .await;
}

/// Cleanup hook that runs after each scenario
async fn cleanup_after_scenario(
    world: &mut CascadeWorld,
    _ctx: &ScenarioContext
) -> AfterHook {
    // Ensure the server is explicitly shut down
    if let Some(server) = &mut world.server_handle {
        println!("Explicitly shutting down test server");
        server.shutdown();
    }
    
    // Reset the world's server-related state
    world.server_handle = None;
    world.last_response = None;
    world.response_body = None;
    world.response_status = None;
    world.flow_id = None;
    world.instance_id = None;
    
    // Wait a short time to allow any background tasks to complete
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    
    AfterHook::Pass
} 
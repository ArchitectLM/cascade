// Main BDD test runner that runs all feature files directly

mod steps;
use steps::world::CascadeWorld;
use cucumber::World;
use std::path::{Path, PathBuf};
use std::fs;

#[tokio::main]
async fn main() {
    // Initialize logging
    std::env::set_var("RUST_LOG", "debug,cucumber=trace");
    env_logger::init();
    
    println!("Starting BDD tests...");
    
    // Get the workspace directory
    let workspace_dir = Path::new("/Users/maxsvargal/Documents/Projects/nocode/cascade");
    let features_dir = workspace_dir.join("tests/bdd/features");
    
    // Verify the features directory exists
    if !features_dir.exists() {
        panic!("Features directory not found: {:?}", features_dir);
    }
    
    println!("Scanning feature files in: {:?}", features_dir);
    
    // Get all .feature files in the directory
    let feature_files: Vec<PathBuf> = fs::read_dir(&features_dir)
        .expect("Failed to read features directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_file() && path.extension()? == "feature" {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    
    if feature_files.is_empty() {
        panic!("No feature files found in: {:?}", features_dir);
    }
    
    println!("Found {} feature files", feature_files.len());
    
    // Run each feature file
    for feature_file in feature_files {
        println!("Running feature file: {:?}", feature_file);
        
        // Verify the feature file exists
        assert!(feature_file.exists(), "Feature file not found: {:?}", feature_file);
        
        // Run the test with default configuration
        CascadeWorld::cucumber()
            .run(feature_file)
            .await;
    }
} 
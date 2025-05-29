//! # BDD Tests Status
//!
//! BDD tests are currently a work in progress and may not all pass. 
//! The tests are designed to validate behavior of the Cascade platform
//! according to the specifications in the feature files.
//! 
//! To run the tests:
//! 
//! ```bash
//! # Run all BDD tests
//! cargo test --test bdd
//! 
//! # Run a specific feature
//! cargo test --test bdd -- order_processing
//! ```
//! 
//! If you encounter failures, please check:
//! 
//! 1. The implementation of step definitions in steps/
//! 2. The feature files in features/
//! 3. The world.rs file for the test context configuration
//! 
//! For issues with errors like "can't run without a World", make sure
//! you've properly set up the cucumber dependency and added required imports.
//! See the documentation at https://cucumber-rs.github.io/ for more details. 

// Public API for the cascade BDD tests

// Re-export the World type for use in the test runners
pub use steps::world::CascadeWorld;

// Re-export our step modules
pub mod steps;

// No need to re-export cucumber since it's a public dependency
// Users can just import it directly from their tests

/// Re-export async_trait for easier use in step definitions
pub use async_trait::async_trait;

/// Utility functions for BDD tests
pub mod utils {
    use std::collections::HashMap;
    use std::time::Duration;
    use tokio::time::sleep;
    
    /// Helper for retrying API calls with backoff
    pub async fn retry_with_backoff<F, Fut, T, E>(
        operation: F,
        retries: usize,
        initial_delay_ms: u64,
    ) -> Result<T, E>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        let mut delay_ms = initial_delay_ms;
        let mut remaining_retries = retries;
        
        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    if remaining_retries == 0 {
                        return Err(err);
                    }
                    
                    // Add jitter to avoid thundering herd
                    let jitter = rand::random::<u64>() % 20;
                    sleep(Duration::from_millis(delay_ms + jitter)).await;
                    
                    // Exponential backoff
                    delay_ms *= 2;
                    remaining_retries -= 1;
                }
            }
        }
    }
    
    /// Helper to parse a table from Cucumber into a Vector of HashMaps
    pub fn parse_table(table: &cucumber::gherkin::Table) -> Vec<HashMap<String, String>> {
        let headers = table.rows.first().cloned().unwrap_or_default();
        
        table.rows.iter().skip(1).map(|row| {
            let mut map = HashMap::new();
            for (i, cell) in row.iter().enumerate() {
                if i < headers.len() {
                    map.insert(headers[i].clone(), cell.clone());
                }
            }
            map
        }).collect()
    }
} 

// BDD Test Library

// Re-export the cucumber crate
pub use cucumber;

/// Helper function to run the cucumber tests with the given features directory
pub async fn run_cucumber_tests(features_dir: &str) {
    use cucumber::World;
    use std::path::PathBuf;

    // Set up the features directory
    let features_path = PathBuf::from(features_dir);
    
    // Run the cucumber tests with a simple setup
    // Note: We rely on the Drop trait implementation to clean up resources
    // when the test completes
    CascadeWorld::cucumber()
        .run(features_path)
        .await;
} 

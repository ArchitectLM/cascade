//! Environment setup builders for testing the Cascade Platform.
//!
//! This module provides builder patterns and utilities for setting up
//! test environments, such as a test server with controlled dependencies.

mod test_server;

// Re-export all builders for easy access
pub use test_server::*; 
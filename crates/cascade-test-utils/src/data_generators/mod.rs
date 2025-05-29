//! Test data generators for the Cascade Platform.
//!
//! This module provides functions for generating test data specific to the
//! Cascade Platform, such as DSL snippets and component configurations.

mod dsl;
mod component_config;

// Re-export all data generators for easy access
pub use dsl::*;
 
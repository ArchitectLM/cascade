//! Assertion utilities for validating Cascade Platform data structures.
//!
//! This module provides helper functions for validating and asserting properties
//! of Cascade Platform data structures, making tests more concise and readable.

mod manifest;
mod flow_state;

// Re-export all assertion helpers for easy access
pub use manifest::*;
pub use flow_state::*; 
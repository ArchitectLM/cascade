//! Mock implementations of key Cascade Platform interfaces.
//!
//! This module provides mock implementations of the core interfaces used in the
//! Cascade Platform, allowing for isolated, controlled testing of components
//! that depend on these interfaces.

pub mod content_storage;
pub mod core_runtime;
pub mod edge_platform;
pub mod component;
pub mod component_behaviors;

// Re-export all mocks and their creator functions for easy access
pub use content_storage::*;
pub use core_runtime::*;
pub use edge_platform::*;
pub use component::*;
pub use component_behaviors::*; 
//! Test implementations (fakes) of key Cascade Platform interfaces.
//!
//! This module provides concrete test implementations of the core interfaces
//! used in the Cascade Platform. These implementations provide higher-fidelity
//! testing capabilities than mocks, but still operate in-memory for testability.

pub mod in_memory_content_store;
pub mod simple_timer_service;

// Re-export all implementations for easy access
pub use in_memory_content_store::*;
pub use simple_timer_service::*; 
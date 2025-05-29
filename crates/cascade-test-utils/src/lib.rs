//! Testing utilities for the Cascade Platform.
//!
//! This crate provides standardized testing utilities for the Cascade Platform,
//! including mocks, test implementations (fakes), environment setup helpers,
//! assertion utilities, and test data generators.

pub mod builders;
pub mod config;
pub mod error;
pub mod mocks;
pub mod implementations;
pub mod assertions;
pub mod data_generators;
pub mod client;
pub mod server;
pub mod util;
pub mod resilience;

/// BDD testing utilities
#[cfg(feature = "bdd")]
pub mod bdd;

/// Re-export commonly used types for convenience
pub use mockall;

pub use config::TestConfig;
pub use client::TestClient;
pub use server::TestServer;

// These imports cause errors - commenting them out until they are fixed
// use builders::TestServerBuilder;
// use data_generators::create_minimal_flow_dsl;
// use mocks::component_behaviors::HttpResponseData;
// use mocks::component_executor::MockComponentExecutor;
// use mocks::core_runtime::MockCoreRuntime; 
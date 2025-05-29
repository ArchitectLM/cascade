//! Integration tests for cascade-kb
//! 
//! This module organizes all integration tests in the integration directory.

// Export test utilities for use in all integration tests
pub mod test_utils;

// Integration test modules
pub mod app_state_ingestion;
pub mod ingestion_query_flow;
pub mod semantic_search_test;
pub mod version_management_test;
pub mod wiring_test; 
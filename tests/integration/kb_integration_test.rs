//! KB Integration Tests
//!
//! This module contains integration tests for the KB client.
// Only the real KB client ("direct" provider) is used in these tests.

use cdas_config::KnowledgeBaseConfig;
use cdas_core_agent::kb::create_kb_client;
use cascade_interfaces::kb::KnowledgeBaseClient;
use std::sync::Arc;

#[tokio::test]
async fn test_kb_client_integration() {
    // Test with direct provider
    let direct_config = KnowledgeBaseConfig {
        provider: "direct".to_string(),
        remote: None,
    };
    
    // Create a KB client (real KB)
    let kb_client = create_kb_client(&direct_config);
    
    // Test that we can call a method on the client
    let result = kb_client.validate_dsl("test-tenant", "sample dsl").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_kb_search_integration() {
    // Test with direct provider
    let direct_config = KnowledgeBaseConfig {
        provider: "direct".to_string(),
        remote: None,
    };
    let kb_client = create_kb_client(&direct_config);
    
    // Test search functionality
    let search_result = kb_client.search_related_artefacts(
        "test-tenant", 
        Some("test query"), 
        None, 
        None,
        5
    ).await;
    assert!(search_result.is_ok());
    // Optionally, assert on the actual results if you have seeded data
} 
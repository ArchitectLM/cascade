//! Integration test utilities
//!
//! This module provides utility functions for integration tests,
//! including functions to set up Neo4j test databases and ensure vector indexes.

use std::sync::Arc;
use std::time::Duration;
use dotenv::dotenv;
use tracing::{info, debug, warn};
use tokio::time::timeout;
use std::env;

use cascade_kb::{
    data::{
        errors::{CoreError, StateStoreError},
        identifiers::TenantId,
        trace_context::TraceContext,
        types::DataPacket,
    },
    adapters::neo4j_store::{Neo4jStateStore, Neo4jConfig},
    traits::{StateStore, EmbeddingGenerator},
    embedding::test_utils::PredictableEmbeddingGenerator,
};

/// Set up a Neo4j test database connection for integration tests
pub async fn setup_neo4j_test_db() -> Result<Arc<dyn StateStore>, StateStoreError> {
    dotenv().ok();
    
    // Get Neo4j connection details from environment
    let uri = std::env::var("NEO4J_URI")
        .unwrap_or_else(|_| "neo4j://localhost:7687".to_string());
    
    let username = std::env::var("NEO4J_USERNAME")
        .unwrap_or_else(|_| "neo4j".to_string());
    
    let password = std::env::var("NEO4J_PASSWORD")
        .unwrap_or_else(|_| "password".to_string());
    
    let database = std::env::var("NEO4J_DATABASE").ok();
    
    let config = Neo4jConfig {
        uri,
        username,
        password,
        database,
        pool_size: 5,
        connection_timeout: Duration::from_secs(5),
        connection_retry_count: 3,
        connection_retry_delay: Duration::from_secs(1),
        query_timeout: Duration::from_secs(10),
    };
    
    // Create and connect the Neo4j state store
    let store = Neo4jStateStore::new(config).await?;
    Ok(Arc::new(store))
}

/// Ensure vector indexes exist in the Neo4j database for testing
pub async fn ensure_vector_indexes(store: &Arc<dyn StateStore>) -> Result<(), CoreError> {
    let tenant_id = TenantId::new_v4();
    let trace_ctx = TraceContext::default();
    
    // Define queries to create vector indexes if they don't exist
    let queries = vec![
        // Check if vector index for components exists
        "CALL db.indexes() YIELD name, type WHERE name = 'component_vector_index' AND type = 'VECTOR' RETURN count(*) > 0 AS exists",
        
        // Create component vector index if it doesn't exist
        "CALL apoc.when(
            MATCH (i:DenseIndex {name: 'component_vector_index'}) RETURN count(i) = 0,
            'CREATE VECTOR INDEX component_vector_index IF NOT EXISTS FOR (n:ComponentDefinition) ON (n.embedding) OPTIONS {indexConfig: {\"vector.dimensions\": 384, \"vector.similarity_function\": \"cosine\"}}',
            '',
            {}
        )",
        
        // Check if vector index for flows exists
        "CALL db.indexes() YIELD name, type WHERE name = 'flow_vector_index' AND type = 'VECTOR' RETURN count(*) > 0 AS exists",
        
        // Create flow vector index if it doesn't exist
        "CALL apoc.when(
            MATCH (i:DenseIndex {name: 'flow_vector_index'}) RETURN count(i) = 0,
            'CREATE VECTOR INDEX flow_vector_index IF NOT EXISTS FOR (n:Flow) ON (n.embedding) OPTIONS {indexConfig: {\"vector.dimensions\": 384, \"vector.similarity_function\": \"cosine\"}}',
            '',
            {}
        )",
        
        // Record that we've created indexes
        "MERGE (i:DenseIndex {name: 'component_vector_index'})
         MERGE (i2:DenseIndex {name: 'flow_vector_index'})"
    ];
    
    // Execute each query with a timeout
    for query in queries {
        debug!("Executing index query: {}", query);
        match timeout(Duration::from_secs(10), store.execute_query(&tenant_id, &trace_ctx, query, None)).await {
            Ok(result) => match result {
                Ok(_) => debug!("Successfully executed index query"),
                Err(e) => {
                    warn!("Error executing index query: {}", e);
                    // Continue despite errors - Neo4j 4.x doesn't support vector indexes
                    // and we want tests to still run with fallback behavior
                }
            },
            Err(_) => {
                warn!("Timeout executing index query");
                // Continue despite timeout
            }
        }
    }
    
    Ok(())
}

/// Create a predictable embedding generator for tests
pub fn create_embedding_generator(dimension: usize) -> Arc<dyn EmbeddingGenerator> {
    // Check environment variables
    let use_mock_embedding = std::env::var("USE_MOCK_EMBEDDING")
        .map(|val| val.to_lowercase() == "true")
        .unwrap_or(false);
        
    let use_predictable = std::env::var("USE_PREDICTABLE_EMBEDDING")
        .map(|val| val.to_lowercase() == "true")
        .unwrap_or(true);
        
    info!("Using PredictableEmbeddingGenerator from main codebase for tests");
    Arc::new(PredictableEmbeddingGenerator::new(dimension))
} 
use tokio;
use dotenv::dotenv;
use std::{env, time::Duration, collections::HashMap, sync::Arc};
use serde_json::json;
use tracing::{info, error, debug};

use cascade_kb::{
    data::{
        errors::StateStoreError,
        identifiers::TenantId,
        trace_context::TraceContext,
        types::DataPacket,
    },
    adapters::neo4j_store::{Neo4jConfig, Neo4jStateStore},
    traits::state_store::StateStore,
};

#[tokio::test]
async fn test_neo4j_error_handling() {
    // Initialize tracing for better debugging
    let _ = tracing_subscriber::fmt()
        .with_env_filter("cascade_kb=debug,neo4j_error_handling_test=debug")
        .try_init();
    
    // Load environment variables
    dotenv().ok();
    
    // Get Neo4j connection details from environment variables
    let uri = env::var("NEO4J_URI")
        .unwrap_or_else(|_| "neo4j://localhost:7687".to_string());
    
    let username = env::var("NEO4J_USERNAME")
        .unwrap_or_else(|_| "neo4j".to_string());
    
    let password = match env::var("NEO4J_PASSWORD") {
        Ok(pass) => pass,
        Err(_) => {
            info!("NEO4J_PASSWORD not set, skipping test");
            return;
        }
    };
    
    let database = env::var("NEO4J_DATABASE").ok();
    
    info!("Connecting to Neo4j at: {}", uri);
    
    // Create Neo4j configuration
    let config = Neo4jConfig {
        uri,
        username,
        password,
        database,
        pool_size: 5,
        connection_timeout: Duration::from_secs(10),
        connection_retry_count: 3,
        connection_retry_delay: Duration::from_secs(2),
        query_timeout: Duration::from_secs(15),
    };
    
    // Create store instance
    let store = match Neo4jStateStore::new(config).await {
        Ok(store) => store,
        Err(e) => {
            info!("Failed to connect to Neo4j: {}, skipping test", e);
            return;
        }
    };
    
    let tenant_id = TenantId::new_v4();
    let trace_ctx = TraceContext::default();
    
    // Clean up any existing test data
    cleanup_test_data(&store, &tenant_id, &trace_ctx).await;
    
    // Test 1: Query syntax error handling
    {
        info!("Test 1: Query syntax error handling");
        
        // Invalid Cypher query
        let invalid_query = "MATCH (n) WHERE syntax error RETURN n";
        
        match store.execute_query(&tenant_id, &trace_ctx, invalid_query, None).await {
            Ok(_) => {
                error!("Query with syntax error succeeded unexpectedly");
                assert!(false, "Invalid query should fail");
            },
            Err(e) => {
                info!("Received expected error for invalid query: {}", e);
                // Verify it's a QueryError
                match e {
                    StateStoreError::QueryError(_) => {
                        info!("Correctly classified as QueryError");
                    },
                    _ => {
                        error!("Incorrect error type: {:?}", e);
                        assert!(false, "Error should be a QueryError");
                    }
                }
            }
        }
    }
    
    // Test 2: Parameter type mismatch error
    {
        info!("Test 2: Parameter type mismatch error");
        
        // Query expecting a string but providing a number
        let query = "MATCH (n) WHERE n.id = $id RETURN n";
        
        let mut params = HashMap::new();
        // ID should be a string, but we're providing a number
        params.insert("id".to_string(), DataPacket::Number(123.0));
        
        match store.execute_query(&tenant_id, &trace_ctx, query, Some(params)).await {
            // This might succeed or fail depending on Neo4j's implicit conversion rules
            // We just want to make sure it doesn't crash
            Ok(results) => {
                info!("Parameter type mismatch handled gracefully: {:?}", results);
            },
            Err(e) => {
                info!("Parameter type mismatch error (expected): {}", e);
            }
        }
    }
    
    // Test 3: Missing parameter error
    {
        info!("Test 3: Missing parameter error");
        
        // Query with parameter that isn't provided
        let query = "MATCH (n) WHERE n.id = $missing_param RETURN n";
        
        let params = HashMap::new(); // Empty params
        
        match store.execute_query(&tenant_id, &trace_ctx, query, Some(params)).await {
            Ok(_) => {
                error!("Query with missing parameter succeeded unexpectedly");
                assert!(false, "Missing parameter should cause error");
            },
            Err(e) => {
                info!("Received expected error for missing parameter: {}", e);
                // Verify it's a QueryError
                match e {
                    StateStoreError::QueryError(_) => {
                        info!("Correctly classified as QueryError");
                    },
                    _ => {
                        error!("Incorrect error type: {:?}", e);
                        assert!(false, "Error should be a QueryError");
                    }
                }
            }
        }
    }
    
    // Test 4: Tenant isolation - Query modification
    {
        info!("Test 4: Testing tenant isolation query modification");
        
        // Create a test node for this tenant
        let node_id = "tenant-isolation-test-node";
        let create_query = format!(
            "CREATE (n:TestNode {{id: '{}', name: 'Test Node', tenant_id: '{}'}}) RETURN n.id", 
            node_id, tenant_id
        );
        
        match store.execute_query(&tenant_id, &trace_ctx, &create_query, None).await {
            Ok(_) => info!("Created test node"),
            Err(e) => {
                error!("Failed to create test node: {}", e);
                assert!(false, "Node creation should succeed: {}", e);
            }
        }
        
        // Query without explicit tenant filter - should be added by Neo4jStateStore
        let simple_query = format!("MATCH (n:TestNode) WHERE n.id = '{}' RETURN n.name", node_id);
        
        match store.execute_query(&tenant_id, &trace_ctx, &simple_query, None).await {
            Ok(results) => {
                assert!(!results.is_empty(), "Should find node after tenant isolation is applied");
                if let Some(row) = results.first() {
                    if let Some(DataPacket::String(name)) = row.get("name") {
                        assert_eq!(name, "Test Node", "Node name should match");
                    }
                }
            },
            Err(e) => {
                error!("Tenant isolation query failed: {}", e);
                assert!(false, "Query should succeed: {}", e);
            }
        }
        
        // Different tenant should not see the node
        let other_tenant_id = TenantId::new_v4();
        
        match store.execute_query(&other_tenant_id, &trace_ctx, &simple_query, None).await {
            Ok(results) => {
                assert!(results.is_empty(), "Other tenant should not see node");
            },
            Err(e) => {
                error!("Other tenant query failed: {}", e);
                assert!(false, "Query should succeed but return empty results: {}", e);
            }
        }
    }
    
    // Test 5: Vector search error handling
    {
        info!("Test 5: Vector search error handling");
        
        // Test with unknown index name
        let unknown_index = "nonExistentIndex";
        let embedding = vec![0.1, 0.2, 0.3, 0.4, 0.5]; // Sample embedding
        
        match store.vector_search(&tenant_id, &trace_ctx, unknown_index, &embedding, 5, None).await {
            Ok(_) => {
                error!("Vector search with non-existent index succeeded unexpectedly");
                assert!(false, "Non-existent index should cause error");
            },
            Err(e) => {
                info!("Received expected error for non-existent index: {}", e);
                // Verify it's an appropriate error type
                match e {
                    StateStoreError::QueryError(_) => {
                        info!("Correctly classified as QueryError");
                    },
                    _ => {
                        // Other error types might be acceptable depending on implementation
                        info!("Error type: {:?}", e);
                    }
                }
            }
        }
    }
    
    // Test 6: Testing query with CREATE/MERGE/DELETE that doesn't need tenant isolation
    {
        info!("Test 6: Testing CREATE/MERGE/DELETE query handling");
        
        // CREATE query should not have tenant isolation WHERE clause added
        let create_query = format!(
            "CREATE (n:TestNode {{id: 'create-test', name: 'Create Test', tenant_id: '{}'}}) RETURN n.id", 
            tenant_id
        );
        
        match store.execute_query(&tenant_id, &trace_ctx, &create_query, None).await {
            Ok(_) => info!("CREATE query executed successfully"),
            Err(e) => {
                error!("CREATE query failed: {}", e);
                assert!(false, "CREATE query should succeed: {}", e);
            }
        }
        
        // MERGE query should not have tenant isolation WHERE clause added
        let merge_query = format!(
            "MERGE (n:TestNode {{id: 'merge-test', tenant_id: '{}'}}) ON CREATE SET n.name = 'Merge Test' RETURN n.id", 
            tenant_id
        );
        
        match store.execute_query(&tenant_id, &trace_ctx, &merge_query, None).await {
            Ok(_) => info!("MERGE query executed successfully"),
            Err(e) => {
                error!("MERGE query failed: {}", e);
                assert!(false, "MERGE query should succeed: {}", e);
            }
        }
        
        // DELETE query should not have tenant isolation WHERE clause added
        let delete_query = format!(
            "MATCH (n:TestNode {{id: 'create-test', tenant_id: '{}'}}) DELETE n", 
            tenant_id
        );
        
        match store.execute_query(&tenant_id, &trace_ctx, &delete_query, None).await {
            Ok(_) => info!("DELETE query executed successfully"),
            Err(e) => {
                error!("DELETE query failed: {}", e);
                assert!(false, "DELETE query should succeed: {}", e);
            }
        }
    }
    
    // Test 7: Query with relationship pattern
    {
        info!("Test 7: Testing query with relationship pattern");
        
        // Create test nodes and relationship
        let query = format!(
            "CREATE (a:TestNode {{id: 'rel-source', name: 'Source Node', tenant_id: '{}'}}),
             (b:TestNode {{id: 'rel-target', name: 'Target Node', tenant_id: '{}'}}),
             (a)-[:TEST_REL {{name: 'test relationship'}}]->(b)
             RETURN a.id, b.id", 
            tenant_id, tenant_id
        );
        
        match store.execute_query(&tenant_id, &trace_ctx, &query, None).await {
            Ok(_) => info!("Created test relationship"),
            Err(e) => {
                error!("Failed to create test relationship: {}", e);
                assert!(false, "Relationship creation should succeed: {}", e);
            }
        }
        
        // Query with relationship pattern - should handle tenant isolation correctly
        let rel_query = "MATCH (a:TestNode)-[r:TEST_REL]->(b:TestNode) RETURN a.name, b.name, r.name";
        
        match store.execute_query(&tenant_id, &trace_ctx, rel_query, None).await {
            Ok(results) => {
                assert!(!results.is_empty(), "Should find relationship");
                if let Some(row) = results.first() {
                    if let Some(DataPacket::String(a_name)) = row.get("a.name") {
                        assert_eq!(a_name, "Source Node", "Source node name should match");
                    }
                    if let Some(DataPacket::String(b_name)) = row.get("b.name") {
                        assert_eq!(b_name, "Target Node", "Target node name should match");
                    }
                    if let Some(DataPacket::String(r_name)) = row.get("r.name") {
                        assert_eq!(r_name, "test relationship", "Relationship name should match");
                    }
                }
            },
            Err(e) => {
                error!("Relationship query failed: {}", e);
                assert!(false, "Query should succeed: {}", e);
            }
        }
        
        // Different tenant should not see the relationship
        let other_tenant_id = TenantId::new_v4();
        
        match store.execute_query(&other_tenant_id, &trace_ctx, rel_query, None).await {
            Ok(results) => {
                assert!(results.is_empty(), "Other tenant should not see relationship");
            },
            Err(e) => {
                error!("Other tenant relationship query failed: {}", e);
                assert!(false, "Query should succeed but return empty results: {}", e);
            }
        }
    }
    
    // Clean up all test data
    cleanup_test_data(&store, &tenant_id, &trace_ctx).await;
}

// Helper function to clean up test data
async fn cleanup_test_data(
    store: &Neo4jStateStore,
    tenant_id: &TenantId,
    trace_ctx: &TraceContext
) {
    info!("Cleaning up test data for tenant {}", tenant_id);
    
    // Delete all test nodes for this tenant
    let cleanup_query = r#"
    MATCH (n)
    WHERE n.tenant_id = $tenant_id
    DETACH DELETE n
    "#;
    
    let mut params = HashMap::new();
    params.insert("tenant_id".to_string(), DataPacket::Json(json!(tenant_id.to_string())));
    
    match store.execute_query(tenant_id, trace_ctx, cleanup_query, Some(params)).await {
        Ok(_) => info!("Successfully cleaned up test data"),
        Err(e) => error!("Failed to clean up test data: {}", e)
    }
} 
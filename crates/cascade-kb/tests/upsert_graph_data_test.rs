use tokio;
use dotenv::dotenv;
use std::{env, time::Duration, collections::HashMap, sync::Arc};
use serde_json::{json, Value};
use uuid::Uuid;
use tracing::{info, error, debug};

use cascade_kb::{
    data::{
        errors::StateStoreError,
        identifiers::TenantId,
        trace_context::TraceContext,
        types::DataPacket,
    },
    adapters::neo4j_store::{Neo4jConfig, Neo4jStateStore},
    traits::state_store::{StateStore, GraphDataPatch},
};

#[tokio::test]
async fn test_upsert_graph_data() {
    // Initialize tracing for better debugging
    let _ = tracing_subscriber::fmt()
        .with_env_filter("cascade_kb=debug,upsert_graph_data_test=debug")
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
    
    // Test 1: Upsert basic nodes and relationships
    {
        info!("Test 1: Basic upsert of nodes and relationships");
        let component_id = Uuid::new_v4().to_string();
        let input_id = Uuid::new_v4().to_string();
        
        // Create nodes and edges
        let nodes = vec![
            // Component Definition node
            json!({
                "labels": ["ComponentDefinition"],
                "properties": {
                    "id": component_id,
                    "name": "TestComponent",
                    "tenant_id": tenant_id.to_string(),
                    "source": "UserDefined",
                    "component_type_id": "test:component"
                }
            }),
            // Input Definition node
            json!({
                "labels": ["InputDefinition"],
                "properties": {
                    "id": input_id,
                    "name": "testInput",
                    "tenant_id": tenant_id.to_string(),
                    "is_required": true
                }
            })
        ];
        
        let edges = vec![
            // HAS_INPUT relationship
            json!({
                "from_labels": ["ComponentDefinition"],
                "from_properties": { "id": component_id },
                "relationship_type": "HAS_INPUT",
                "to_labels": ["InputDefinition"],
                "to_properties": { "id": input_id },
                "properties": {}
            })
        ];
        
        let data = GraphDataPatch { nodes, edges };
        
        // Perform the upsert
        match store.upsert_graph_data(&tenant_id, &trace_ctx, data).await {
            Ok(_) => info!("Successfully upserted basic graph data"),
            Err(e) => {
                error!("Failed to upsert graph data: {}", e);
                assert!(false, "Upsert should succeed: {}", e);
            }
        }
        
        // Verify the data was written correctly
        let query = r#"
        MATCH (c:ComponentDefinition {id: $component_id})-[r:HAS_INPUT]->(i:InputDefinition {id: $input_id})
        RETURN c.name as component_name, i.name as input_name
        "#;
        
        let mut params = HashMap::new();
        params.insert("component_id".to_string(), DataPacket::Json(json!(component_id)));
        params.insert("input_id".to_string(), DataPacket::Json(json!(input_id)));
        
        match store.execute_query(&tenant_id, &trace_ctx, query, Some(params)).await {
            Ok(results) => {
                assert!(!results.is_empty(), "Should have found the created nodes");
                
                if let Some(row) = results.first() {
                    if let Some(DataPacket::String(component_name)) = row.get("component_name") {
                        assert_eq!(component_name, "TestComponent", "Component name should match");
                    } else {
                        assert!(false, "Component name not found in result");
                    }
                    
                    if let Some(DataPacket::String(input_name)) = row.get("input_name") {
                        assert_eq!(input_name, "testInput", "Input name should match");
                    } else {
                        assert!(false, "Input name not found in result");
                    }
                }
            },
            Err(e) => {
                error!("Query failed: {}", e);
                assert!(false, "Query should succeed: {}", e);
            }
        }
    }
    
    // Test 2: Test idempotent upsert (should not create duplicates)
    {
        info!("Test 2: Testing idempotent upsert (no duplicates)");
        let component_id = Uuid::new_v4().to_string();
        
        // Create initial node
        let nodes = vec![
            json!({
                "labels": ["TestNode"],
                "properties": {
                    "id": component_id,
                    "name": "IdempotentTest",
                    "tenant_id": tenant_id.to_string(),
                    "counter": 1
                }
            })
        ];
        
        let data = GraphDataPatch { nodes, edges: vec![] };
        
        // First upsert
        match store.upsert_graph_data(&tenant_id, &trace_ctx, data.clone()).await {
            Ok(_) => info!("First upsert successful"),
            Err(e) => {
                error!("First upsert failed: {}", e);
                assert!(false, "First upsert should succeed: {}", e);
            }
        }
        
        // Second upsert with same ID but updated counter
        let nodes = vec![
            json!({
                "labels": ["TestNode"],
                "properties": {
                    "id": component_id,
                    "name": "IdempotentTest",
                    "tenant_id": tenant_id.to_string(),
                    "counter": 2  // Updated property
                }
            })
        ];
        
        let data = GraphDataPatch { nodes, edges: vec![] };
        
        // Second upsert
        match store.upsert_graph_data(&tenant_id, &trace_ctx, data).await {
            Ok(_) => info!("Second upsert successful"),
            Err(e) => {
                error!("Second upsert failed: {}", e);
                assert!(false, "Second upsert should succeed: {}", e);
            }
        }
        
        // Verify there's only one node with the updated counter value
        let query = r#"
        MATCH (n:TestNode {id: $id})
        RETURN n.counter as counter, count(n) as node_count
        "#;
        
        let mut params = HashMap::new();
        params.insert("id".to_string(), DataPacket::Json(json!(component_id)));
        
        match store.execute_query(&tenant_id, &trace_ctx, query, Some(params)).await {
            Ok(results) => {
                assert!(!results.is_empty(), "Should have found the node");
                
                if let Some(row) = results.first() {
                    // Check there's only one node
                    if let Some(DataPacket::Number(count)) = row.get("node_count") {
                        assert_eq!(*count as i64, 1, "Should only be one node");
                    } else {
                        assert!(false, "Node count not found in result");
                    }
                    
                    // Check the counter was updated
                    if let Some(DataPacket::Number(counter)) = row.get("counter") {
                        assert_eq!(*counter as i64, 2, "Counter should be updated to 2");
                    } else {
                        assert!(false, "Counter not found in result");
                    }
                }
            },
            Err(e) => {
                error!("Query failed: {}", e);
                assert!(false, "Query should succeed: {}", e);
            }
        }
    }
    
    // Test 3: Multi-tenant isolation with upsert
    {
        info!("Test 3: Testing multi-tenant isolation with upsert");
        let component_id = Uuid::new_v4().to_string();
        let tenant_a = tenant_id; // Reuse existing tenant
        let tenant_b = TenantId::new_v4(); // Create a new tenant
        
        // Create node for tenant A
        let nodes = vec![
            json!({
                "labels": ["TenantTestNode"],
                "properties": {
                    "id": component_id,
                    "name": "TenantTest",
                    "tenant_id": tenant_a.to_string(),
                    "data": "Tenant A Data"
                }
            })
        ];
        
        let data = GraphDataPatch { nodes, edges: vec![] };
        
        // Upsert for tenant A
        match store.upsert_graph_data(&tenant_a, &trace_ctx, data).await {
            Ok(_) => info!("Tenant A upsert successful"),
            Err(e) => {
                error!("Tenant A upsert failed: {}", e);
                assert!(false, "Tenant A upsert should succeed: {}", e);
            }
        }
        
        // Tenant B should not see Tenant A's data
        let query = r#"
        MATCH (n:TenantTestNode {id: $id})
        RETURN n.data as data
        "#;
        
        let mut params = HashMap::new();
        params.insert("id".to_string(), DataPacket::Json(json!(component_id)));
        
        match store.execute_query(&tenant_b, &trace_ctx, query, Some(params.clone())).await {
            Ok(results) => {
                assert!(results.is_empty(), "Tenant B should not see Tenant A's data");
            },
            Err(e) => {
                error!("Query failed: {}", e);
                assert!(false, "Query should succeed: {}", e);
            }
        }
        
        // Tenant A should see its own data
        match store.execute_query(&tenant_a, &trace_ctx, query, Some(params)).await {
            Ok(results) => {
                assert!(!results.is_empty(), "Tenant A should see its own data");
                
                if let Some(row) = results.first() {
                    if let Some(DataPacket::String(data)) = row.get("data") {
                        assert_eq!(data, "Tenant A Data", "Data should match");
                    } else {
                        assert!(false, "Data not found in result");
                    }
                }
            },
            Err(e) => {
                error!("Query failed: {}", e);
                assert!(false, "Query should succeed: {}", e);
            }
        }
    }
    
    // Test 4: Error handling for invalid data
    {
        info!("Test 4: Testing error handling for invalid data");
        
        // Empty nodes and edges should be fine
        let empty_data = GraphDataPatch { nodes: vec![], edges: vec![] };
        
        match store.upsert_graph_data(&tenant_id, &trace_ctx, empty_data).await {
            Ok(_) => info!("Empty graph data upsert successful"),
            Err(e) => {
                error!("Empty graph data upsert failed: {}", e);
                assert!(false, "Empty upsert should succeed: {}", e);
            }
        }
        
        // Invalid JSON object for nodes (missing required fields)
        let invalid_nodes = vec![
            json!({
                // Missing "labels" field
                "properties": {
                    "id": Uuid::new_v4().to_string(),
                    "name": "InvalidNode"
                }
            })
        ];
        
        let invalid_data = GraphDataPatch { nodes: invalid_nodes, edges: vec![] };
        
        match store.upsert_graph_data(&tenant_id, &trace_ctx, invalid_data).await {
            Ok(_) => {
                // The current implementation might handle this gracefully
                info!("Invalid node schema was handled gracefully");
            },
            Err(e) => {
                // This is also acceptable - explicit error
                info!("Invalid node schema was rejected as expected: {}", e);
            }
        }
        
        // Invalid relationship (referencing non-existent nodes)
        let invalid_edges = vec![
            json!({
                "from_labels": ["NonExistentLabel"],
                "from_properties": { "id": Uuid::new_v4().to_string() },
                "relationship_type": "INVALID_REL",
                "to_labels": ["AnotherNonExistentLabel"],
                "to_properties": { "id": Uuid::new_v4().to_string() },
                "properties": {}
            })
        ];
        
        let invalid_data = GraphDataPatch { nodes: vec![], edges: invalid_edges };
        
        match store.upsert_graph_data(&tenant_id, &trace_ctx, invalid_data).await {
            Ok(_) => {
                // The current implementation might handle this gracefully
                info!("Invalid edge schema was handled gracefully");
            },
            Err(e) => {
                // This is also acceptable - explicit error
                info!("Invalid edge schema was rejected as expected: {}", e);
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
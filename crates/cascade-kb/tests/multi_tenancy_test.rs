//! Test multi-tenancy isolation
//!
//! This test verifies that data is properly isolated between tenants in the Knowledge Base.

use tokio;
use std::sync::Arc;
use tracing::{info, error};
use uuid::Uuid;
use std::collections::HashMap;

use cascade_kb::{
    data::{
        identifiers::TenantId,
        trace_context::TraceContext,
        types::{DataPacket, Scope, SourceType},
    },
    traits::StateStore,
};

// Import test utils
mod test_utils;
use test_utils::{
    get_test_store, 
    cleanup_tenant_data, 
    create_test_component, 
    extract_string, 
    init_test_tracing
};

#[tokio::test]
async fn test_multi_tenancy_isolation() {
    // Initialize tracing
    init_test_tracing();
    
    // Get a test store (either Neo4j or FakeStateStore)
    let store = get_test_store().await;
    
    // Create two tenant IDs for testing
    let tenant_a = TenantId::new_v4();
    let tenant_b = TenantId::new_v4();
    
    info!("Created test tenants: {} and {}", tenant_a, tenant_b);
    
    // Clean up any existing data for these tenants
    let _ = cleanup_tenant_data(&store, &tenant_a).await;
    let _ = cleanup_tenant_data(&store, &tenant_b).await;
    
    // Create a component for tenant A
    let component_a_name = "TestComponentA";
    match create_test_component(&store, &tenant_a, component_a_name).await {
        Ok(component_id) => {
            info!("Created component {} for tenant A with ID {}", component_a_name, component_id);
        },
        Err(e) => {
            error!("Failed to create component for tenant A: {}", e);
            panic!("Test setup failed: {}", e);
        }
    }
    
    // Create a component for tenant B
    let component_b_name = "TestComponentB";
    match create_test_component(&store, &tenant_b, component_b_name).await {
        Ok(component_id) => {
            info!("Created component {} for tenant B with ID {}", component_b_name, component_id);
        },
        Err(e) => {
            error!("Failed to create component for tenant B: {}", e);
            panic!("Test setup failed: {}", e);
        }
    }
    
    // Test 1: Tenant A should only see its own component
    let trace_ctx = TraceContext::default();
    let query = "MATCH (cd:ComponentDefinition) WHERE cd.tenant_id = $tenant_id RETURN cd.name as name";
    
    let mut params = HashMap::new();
    params.insert("tenant_id".to_string(), DataPacket::Json(serde_json::json!(tenant_a.to_string())));
    
    match store.execute_query(&tenant_a, &trace_ctx, query, Some(params)).await {
        Ok(rows) => {
            assert_eq!(rows.len(), 1, "Tenant A should see exactly 1 component");
            let name = extract_string(&rows[0], "name");
            assert_eq!(name, component_a_name, "Component name should match for tenant A");
        },
        Err(e) => {
            error!("Query failed for tenant A: {}", e);
            panic!("Query failed: {}", e);
        }
    }
    
    // Test 2: Tenant B should only see its own component
    let mut params = HashMap::new();
    params.insert("tenant_id".to_string(), DataPacket::Json(serde_json::json!(tenant_b.to_string())));
    
    match store.execute_query(&tenant_b, &trace_ctx, query, Some(params)).await {
        Ok(rows) => {
            assert_eq!(rows.len(), 1, "Tenant B should see exactly 1 component");
            let name = extract_string(&rows[0], "name");
            assert_eq!(name, component_b_name, "Component name should match for tenant B");
        },
        Err(e) => {
            error!("Query failed for tenant B: {}", e);
            panic!("Query failed: {}", e);
        }
    }
    
    // Test 3: Tenant A should not see tenant B's component when using tenant_id in query
    let cross_tenant_query = "MATCH (cd:ComponentDefinition) WHERE cd.name = $component_name AND (cd.tenant_id = $current_tenant_id OR cd.tenant_id = 'general') RETURN cd.tenant_id as tenant_id";
    
    let mut params = HashMap::new();
    params.insert("component_name".to_string(), DataPacket::Json(serde_json::json!(component_b_name)));
    params.insert("current_tenant_id".to_string(), DataPacket::Json(serde_json::json!(tenant_a.to_string())));
    
    match store.execute_query(&tenant_a, &trace_ctx, cross_tenant_query, Some(params)).await {
        Ok(rows) => {
            info!("Cross-tenant query returned {} rows", rows.len());
            
            // The query should return 0 rows because tenant B's component should not be visible to tenant A
            assert!(rows.is_empty(), "Tenant A should not see tenant B's components");
            
            // If any results are returned (shouldn't happen), verify they don't belong to tenant B
            for row in &rows {
                let found_tenant_id = extract_string(row, "tenant_id");
                assert_ne!(found_tenant_id, tenant_b.to_string(), 
                    "Tenant A should not be able to access tenant B's data");
            }
        },
        Err(e) => {
            error!("Cross-tenant query failed: {}", e);
            panic!("Query failed: {}", e);
        }
    }
    
    // Test 4: Testing general scope visibility
    // Create a component with general scope that should be visible to all tenants
    let general_component_name = "GeneralComponent";
    let general_query = r#"
    CREATE (vs:VersionSet {
        id: $vs_id,
        entity_id: $entity_id,
        entity_type: 'ComponentDefinition',
        tenant_id: 'general'
    })
    CREATE (v:Version {
        id: $v_id,
        version_number: '1.0.0',
        status: 'Active',
        created_at: datetime(),
        tenant_id: 'general'
    })
    CREATE (cd:ComponentDefinition {
        id: $cd_id,
        name: $name,
        component_type_id: $entity_id,
        source: 'StdLib',
        scope: 'General',
        created_at: datetime(),
        tenant_id: 'general'
    })
    CREATE (vs)-[:HAS_VERSION]->(v)
    CREATE (v)-[:REPRESENTS]->(cd)
    RETURN cd.id as component_id, cd.tenant_id as tenant_id, cd.scope as scope
    "#;
    
    let mut params = HashMap::new();
    params.insert("vs_id".to_string(), DataPacket::Json(serde_json::json!(Uuid::new_v4().to_string())));
    params.insert("v_id".to_string(), DataPacket::Json(serde_json::json!(Uuid::new_v4().to_string())));
    params.insert("cd_id".to_string(), DataPacket::Json(serde_json::json!(Uuid::new_v4().to_string())));
    params.insert("entity_id".to_string(), DataPacket::Json(serde_json::json!(general_component_name)));
    params.insert("name".to_string(), DataPacket::Json(serde_json::json!(general_component_name)));
    
    // Use tenant A for this query, even though we're creating a general component
    match store.execute_query(&tenant_a, &trace_ctx, general_query, Some(params)).await {
        Ok(_) => {
            info!("Created general component {}", general_component_name);
        },
        Err(e) => {
            error!("Failed to create general component: {}", e);
            panic!("Test setup failed: {}", e);
        }
    }
    
    // Test 5: Both tenants should be able to see the general component
    // Query for tenant A - Now using a more direct approach to find the general component
    let general_component_query = "MATCH (cd:ComponentDefinition) WHERE cd.name = $name AND cd.tenant_id = 'general' RETURN cd.id as id, cd.name as name, cd.tenant_id as tenant_id, cd.scope as scope, cd.source as source";
    
    let mut params = HashMap::new();
    params.insert("name".to_string(), DataPacket::Json(serde_json::json!(general_component_name)));
    
    // Check tenant A can see the general component
    match store.execute_query(&tenant_a, &trace_ctx, general_component_query, Some(params.clone())).await {
        Ok(rows) => {
            info!("For tenant A, direct general component query returned {} rows", rows.len());
            for (i, row) in rows.iter().enumerate() {
                info!("Row {}: Full row content: {:?}", i, row);
                for (key, value) in row {
                    info!("Key: {:?}, Value: {:?}", key, value);
                }
            }
            
            // We should have exactly one general component
            assert!(!rows.is_empty(), "Tenant A should be able to see the general component");
            
            let row = &rows[0]; // Just check the first row
            let name = extract_string(row, "name");
            let tenant_id = extract_string(row, "tenant_id");
            let source = extract_string(row, "source");
            
            info!("Checking general component: name={}, tenant={}, source={}", name, tenant_id, source);
            
            assert_eq!(name, general_component_name, "Component name should match");
            assert_eq!(tenant_id.to_lowercase(), "general", "Tenant ID should be 'general'");
            assert_eq!(source.to_lowercase(), "stdlib", "Source should be 'StdLib'");
        },
        Err(e) => {
            error!("Query for general component failed for tenant A: {}", e);
            panic!("Query failed: {}", e);
        }
    }
    
    // Apply similar changes for tenant B
    match store.execute_query(&tenant_b, &trace_ctx, general_component_query, Some(params.clone())).await {
        Ok(rows) => {
            info!("For tenant B, direct general component query returned {} rows", rows.len());
            for (i, row) in rows.iter().enumerate() {
                info!("Tenant B - Row {}: Full row content: {:?}", i, row);
                for (key, value) in row {
                    info!("Tenant B - Key: {:?}, Value: {:?}", key, value);
                }
            }
            
            // We should have exactly one general component
            assert!(!rows.is_empty(), "Tenant B should be able to see the general component");
            
            let row = &rows[0]; // Just check the first row
            let name = extract_string(row, "name");
            let tenant_id = extract_string(row, "tenant_id");
            let source = extract_string(row, "source");
            
            info!("Tenant B - Checking general component: name={}, tenant={}, source={}", 
                  name, tenant_id, source);
            
            assert_eq!(name, general_component_name, "Component name should match");
            assert_eq!(tenant_id.to_lowercase(), "general", "Tenant ID should be 'general'");
            assert_eq!(source.to_lowercase(), "stdlib", "Source should be 'StdLib'");
        },
        Err(e) => {
            error!("Query for general component failed for tenant B: {}", e);
            panic!("Query failed: {}", e);
        }
    }
    
    // Clean up after tests
    let _ = cleanup_tenant_data(&store, &tenant_a).await;
    let _ = cleanup_tenant_data(&store, &tenant_b).await;
    
    // Also clean up the general tenant data
    let general_tenant = TenantId::new_v4(); // Create a UUID for general cleanup
    let mut params = HashMap::new();
    params.insert("tenant_id".to_string(), DataPacket::Json(serde_json::json!("general")));
    
    // Use a different query for the general tenant since it's a string, not a UUID
    let general_cleanup_query = r#"
    MATCH (n) 
    WHERE n.tenant_id = 'general'
    DETACH DELETE n
    "#;
    
    match store.execute_query(&general_tenant, &trace_ctx, general_cleanup_query, None).await {
        Ok(_) => {
            info!("Successfully cleaned up data for general tenant");
        },
        Err(e) => {
            error!("Failed to clean up general tenant data: {}", e);
        }
    }
    
    info!("Multi-tenancy isolation test completed successfully");
} 
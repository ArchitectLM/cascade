//! Test version management in Neo4j
//!
//! This test verifies that version set and version nodes are created and managed correctly.

use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, error};
use uuid::Uuid;

use cascade_kb::{
    data::{
        identifiers::TenantId,
        trace_context::TraceContext,
        types::{DataPacket, DataPacketMapExt},
    },
    traits::StateStore,
};

// Import test utils
mod test_utils;
use test_utils::{get_test_store, cleanup_tenant_data};

#[tokio::test]
async fn test_version_management() {
    // Initialize tracing
    let _ = tracing_subscriber::fmt()
        .with_env_filter("cascade_kb=debug,version_management_test=debug")
        .try_init();
    
    // Get a test store using our helper
    let store = get_test_store().await;
    
    // Create test tenant
    let tenant_id = TenantId::new_v4();
    let trace_ctx = TraceContext::default();
    
    // Clean up any existing test data for this tenant
    let _ = cleanup_tenant_data(&store, &tenant_id).await;
    
    // Create a test component definition with multiple versions
    let component_type_id = "test:VersionedComponent";
    let vs_id = Uuid::new_v4().to_string();
    
    // 1. Create VersionSet node
    info!("Creating VersionSet node...");
    let create_vs_query = r#"
    CREATE (vs:VersionSet {
        id: $id,
        entity_id: $entity_id,
        entity_type: 'ComponentDefinition',
        tenant_id: $tenant_id
    })
    RETURN vs.id
    "#;
    
    let mut params = HashMap::new();
    params.insert("id".to_string(), DataPacket::Json(serde_json::json!(vs_id)));
    params.insert("entity_id".to_string(), DataPacket::Json(serde_json::json!(component_type_id)));
    params.insert("tenant_id".to_string(), DataPacket::Json(serde_json::json!(tenant_id.to_string())));
    
    match store.execute_query(&tenant_id, &trace_ctx, create_vs_query, Some(params)).await {
        Ok(_) => info!("VersionSet created successfully"),
        Err(e) => {
            error!("Failed to create VersionSet: {:?}", e);
            panic!("Test setup failed: {}", e);
        }
    }
    
    // 2. Create first version (v1.0.0)
    info!("Creating Version 1.0.0...");
    let v1_id = Uuid::new_v4().to_string();
    let cd1_id = Uuid::new_v4().to_string();
    
    let create_v1_query = r#"
    MATCH (vs:VersionSet {id: $vs_id, tenant_id: $tenant_id})
    CREATE (v:Version {
        id: $v_id,
        version_number: '1.0.0',
        status: 'Active',
        created_at: datetime(),
        tenant_id: $tenant_id
    })
    CREATE (cd:ComponentDefinition {
        id: $cd_id,
        name: $name,
        component_type_id: $entity_id,
        source: 'UserDefined',
        scope: 'UserDefined',
        created_at: datetime(),
        tenant_id: $tenant_id
    })
    CREATE (vs)-[:HAS_VERSION]->(v)
    CREATE (v)-[:REPRESENTS]->(cd)
    CREATE (vs)-[:LATEST_VERSION]->(v)
    RETURN v.id
    "#;
    
    let mut params = HashMap::new();
    params.insert("vs_id".to_string(), DataPacket::Json(serde_json::json!(vs_id)));
    params.insert("v_id".to_string(), DataPacket::Json(serde_json::json!(v1_id)));
    params.insert("cd_id".to_string(), DataPacket::Json(serde_json::json!(cd1_id)));
    params.insert("name".to_string(), DataPacket::Json(serde_json::json!("VersionedComponent v1.0.0")));
    params.insert("entity_id".to_string(), DataPacket::Json(serde_json::json!(component_type_id)));
    params.insert("tenant_id".to_string(), DataPacket::Json(serde_json::json!(tenant_id.to_string())));
    
    match store.execute_query(&tenant_id, &trace_ctx, create_v1_query, Some(params)).await {
        Ok(_) => info!("Version 1.0.0 created successfully"),
        Err(e) => {
            error!("Failed to create Version 1.0.0: {:?}", e);
            panic!("Test setup failed: {}", e);
        }
    }
    
    // 3. Create second version (v1.1.0)
    info!("Creating Version 1.1.0...");
    let v2_id = Uuid::new_v4().to_string();
    let cd2_id = Uuid::new_v4().to_string();
    
    let create_v2_query = r#"
    MATCH (vs:VersionSet {id: $vs_id, tenant_id: $tenant_id})
    MATCH (v_old:Version {id: $v1_id, tenant_id: $tenant_id})
    MATCH (vs)-[r:LATEST_VERSION]->(v_old)
    DELETE r
    CREATE (v:Version {
        id: $v_id,
        version_number: '1.1.0',
        status: 'Active',
        created_at: datetime(),
        tenant_id: $tenant_id
    })
    CREATE (cd:ComponentDefinition {
        id: $cd_id,
        name: $name,
        component_type_id: $entity_id,
        source: 'UserDefined',
        scope: 'UserDefined',
        created_at: datetime(),
        tenant_id: $tenant_id
    })
    CREATE (vs)-[:HAS_VERSION]->(v)
    CREATE (v)-[:REPRESENTS]->(cd)
    CREATE (vs)-[:LATEST_VERSION]->(v)
    CREATE (v)-[:PREVIOUS_VERSION]->(v_old)
    RETURN v.id
    "#;
    
    let mut params = HashMap::new();
    params.insert("vs_id".to_string(), DataPacket::Json(serde_json::json!(vs_id)));
    params.insert("v1_id".to_string(), DataPacket::Json(serde_json::json!(v1_id)));
    params.insert("v_id".to_string(), DataPacket::Json(serde_json::json!(v2_id)));
    params.insert("cd_id".to_string(), DataPacket::Json(serde_json::json!(cd2_id)));
    params.insert("name".to_string(), DataPacket::Json(serde_json::json!("VersionedComponent v1.1.0")));
    params.insert("entity_id".to_string(), DataPacket::Json(serde_json::json!(component_type_id)));
    params.insert("tenant_id".to_string(), DataPacket::Json(serde_json::json!(tenant_id.to_string())));
    
    match store.execute_query(&tenant_id, &trace_ctx, create_v2_query, Some(params)).await {
        Ok(_) => info!("Version 1.1.0 created successfully"),
        Err(e) => {
            error!("Failed to create Version 1.1.0: {:?}", e);
            panic!("Test setup failed: {}", e);
        }
    }
    
    // 4. Create third version (v2.0.0)
    info!("Creating Version 2.0.0...");
    let v3_id = Uuid::new_v4().to_string();
    let cd3_id = Uuid::new_v4().to_string();
    
    let create_v3_query = r#"
    MATCH (vs:VersionSet {id: $vs_id, tenant_id: $tenant_id})
    MATCH (v_old:Version {id: $v2_id, tenant_id: $tenant_id})
    MATCH (vs)-[r:LATEST_VERSION]->(v_old)
    DELETE r
    CREATE (v:Version {
        id: $v_id,
        version_number: '2.0.0',
        status: 'Active',
        created_at: datetime(),
        tenant_id: $tenant_id
    })
    CREATE (cd:ComponentDefinition {
        id: $cd_id,
        name: $name,
        component_type_id: $entity_id,
        source: 'UserDefined',
        scope: 'UserDefined',
        created_at: datetime(),
        tenant_id: $tenant_id
    })
    CREATE (vs)-[:HAS_VERSION]->(v)
    CREATE (v)-[:REPRESENTS]->(cd)
    CREATE (vs)-[:LATEST_VERSION]->(v)
    CREATE (v)-[:PREVIOUS_VERSION]->(v_old)
    RETURN v.id
    "#;
    
    let mut params = HashMap::new();
    params.insert("vs_id".to_string(), DataPacket::Json(serde_json::json!(vs_id)));
    params.insert("v2_id".to_string(), DataPacket::Json(serde_json::json!(v2_id)));
    params.insert("v_id".to_string(), DataPacket::Json(serde_json::json!(v3_id)));
    params.insert("cd_id".to_string(), DataPacket::Json(serde_json::json!(cd3_id)));
    params.insert("name".to_string(), DataPacket::Json(serde_json::json!("VersionedComponent v2.0.0")));
    params.insert("entity_id".to_string(), DataPacket::Json(serde_json::json!(component_type_id)));
    params.insert("tenant_id".to_string(), DataPacket::Json(serde_json::json!(tenant_id.to_string())));
    
    match store.execute_query(&tenant_id, &trace_ctx, create_v3_query, Some(params)).await {
        Ok(_) => info!("Version 2.0.0 created successfully"),
        Err(e) => {
            error!("Failed to create Version 2.0.0: {:?}", e);
            panic!("Test setup failed: {}", e);
        }
    }
    
    // 5. Test: Verify LATEST_VERSION relationship points to v2.0.0
    info!("Verifying LATEST_VERSION relationship...");
    let latest_query = r#"
    MATCH (vs:VersionSet {id: $vs_id, tenant_id: $tenant_id})-[:LATEST_VERSION]->(v:Version)
    RETURN v.id as id, v.version_number as version
    "#;
    
    let mut params = HashMap::new();
    params.insert("vs_id".to_string(), DataPacket::Json(serde_json::json!(vs_id)));
    params.insert("tenant_id".to_string(), DataPacket::Json(serde_json::json!(tenant_id.to_string())));
    
    match store.execute_query(&tenant_id, &trace_ctx, latest_query, Some(params)).await {
        Ok(rows) => {
            assert_eq!(rows.len(), 1, "Expected exactly one LATEST_VERSION relationship");
            let version_id = match rows[0].get("id") {
                Some(DataPacket::Json(value)) => value.as_str().unwrap_or("").to_string(),
                _ => String::new(),
            };
            let version_number = match rows[0].get("version") {
                Some(DataPacket::Json(value)) => value.as_str().unwrap_or("").to_string(),
                _ => String::new(),
            };
            info!("Latest version is: {} ({})", version_number, version_id);
            assert_eq!(version_id, v3_id, "Latest version should be v3");
            assert_eq!(version_number, "2.0.0", "Latest version should be 2.0.0");
        },
        Err(e) => {
            error!("Failed to verify LATEST_VERSION: {:?}", e);
            panic!("Test verification failed: {}", e);
        }
    }
    
    // 6. Test: Verify version chain using PREVIOUS_VERSION relationships
    info!("Verifying version chain...");
    let chain_query = r#"
    MATCH (v:Version {id: $v3_id, tenant_id: $tenant_id})
    MATCH path = (v)-[:PREVIOUS_VERSION*]->(first:Version)
    WHERE NOT (first)-[:PREVIOUS_VERSION]->()
    RETURN length(path) as chain_length, 
           [node in nodes(path) | node.version_number] as versions
    "#;
    
    let mut params = HashMap::new();
    params.insert("v3_id".to_string(), DataPacket::Json(serde_json::json!(v3_id)));
    params.insert("tenant_id".to_string(), DataPacket::Json(serde_json::json!(tenant_id.to_string())));
    
    match store.execute_query(&tenant_id, &trace_ctx, chain_query, Some(params)).await {
        Ok(rows) => {
            assert!(!rows.is_empty(), "Expected version chain results");
            let chain_length_value = match rows[0].get("chain_length") {
                Some(DataPacket::Json(value)) => {
                    if let Some(num) = value.as_i64() {
                        num as usize
                    } else {
                        0 // Default if not an integer
                    }
                },
                _ => 0, // Default if value not found
            };
            info!("Version chain length: {}", chain_length_value);
            assert_eq!(chain_length_value, 2, "Expected chain length of 2 (v2.0.0 -> v1.1.0 -> v1.0.0)");
            
            // Check the version numbers in the chain
            if let Some(DataPacket::Json(json_arr)) = rows[0].get("versions") {
                if let Some(versions) = json_arr.as_array() {
                    let version_strings: Vec<String> = versions.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    
                    info!("Version chain: {:?}", version_strings);
                    assert_eq!(version_strings.len(), 3, "Expected 3 versions in chain");
                    if version_strings.len() >= 3 {
                        assert_eq!(version_strings[0], "2.0.0", "First version in chain should be 2.0.0");
                        assert_eq!(version_strings[1], "1.1.0", "Second version in chain should be 1.1.0");
                        assert_eq!(version_strings[2], "1.0.0", "Third version in chain should be 1.0.0");
                    }
                }
            }
        },
        Err(e) => {
            error!("Failed to verify version chain: {:?}", e);
            panic!("Test verification failed: {}", e);
        }
    }
    
    // 7. Test: Looking up component by its logical entity_id (not by version)
    info!("Testing component lookup by entity_id...");
    let lookup_query = r#"
    MATCH (vs:VersionSet {entity_id: $entity_id, tenant_id: $tenant_id})
    MATCH (vs)-[:LATEST_VERSION]->(v:Version)
    MATCH (v)-[:REPRESENTS]->(cd:ComponentDefinition)
    RETURN cd.id as id, cd.name as name, v.version_number as version
    "#;
    
    let mut params = HashMap::new();
    params.insert("entity_id".to_string(), DataPacket::Json(serde_json::json!(component_type_id)));
    params.insert("tenant_id".to_string(), DataPacket::Json(serde_json::json!(tenant_id.to_string())));
    
    match store.execute_query(&tenant_id, &trace_ctx, lookup_query, Some(params)).await {
        Ok(rows) => {
            assert_eq!(rows.len(), 1, "Expected exactly one result for latest version");
            let name = match rows[0].get("name") {
                Some(DataPacket::Json(value)) => value.as_str().unwrap_or("").to_string(),
                _ => String::new(),
            };
            let version = match rows[0].get("version") {
                Some(DataPacket::Json(value)) => value.as_str().unwrap_or("").to_string(),
                _ => String::new(),
            };
            info!("Found component: {} (v{})", name, version);
            assert_eq!(version, "2.0.0", "Latest version should be 2.0.0");
        },
        Err(e) => {
            error!("Failed to lookup component: {:?}", e);
            panic!("Test verification failed: {}", e);
        }
    }
    
    // Clean up test data
    let _ = cleanup_tenant_data(&store, &tenant_id).await;
    
    info!("Version management test completed successfully");
} 
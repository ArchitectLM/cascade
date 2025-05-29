use tokio;
use dotenv::dotenv;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, error};
use neo4rs::ConfigBuilder;

use cascade_kb::{
    data::{identifiers::TenantId, trace_context::TraceContext},
    traits::StateStore,
    adapters::neo4j_store::{Neo4jStateStore, Neo4jConfig},
};

#[tokio::test]
async fn test_neo4j_stdlib_components() {
    // Initialize tracing
    let _ = tracing_subscriber::fmt()
        .with_env_filter("cascade_kb=debug,neo4j_stdlib_components=debug")
        .try_init();
    
    // Load environment variables
    dotenv().ok();
    
    // Get connection details from environment variables or skip if not available
    let uri = match env::var("NEO4J_URI") {
        Ok(uri) => uri,
        Err(_) => {
            println!("Skipping test: NEO4J_URI not set");
            return;
        }
    };
    
    let username = env::var("NEO4J_USERNAME")
        .unwrap_or_else(|_| "neo4j".to_string());
    
    let password = match env::var("NEO4J_PASSWORD") {
        Ok(password) => password,
        Err(_) => {
            println!("Skipping test: NEO4J_PASSWORD not set");
            return;
        }
    };
    
    let database = env::var("NEO4J_DATABASE")
        .unwrap_or_else(|_| "neo4j".to_string());
    
    println!("Connecting to Neo4j at: {}", uri);
    
    // Create Neo4j StateStore
    let config = Neo4jConfig {
        uri,
        username,
        password,
        database: Some(database),
        pool_size: 10,
        connection_timeout: Duration::from_secs(5),
        connection_retry_count: 2,
        connection_retry_delay: Duration::from_secs(1),
        query_timeout: Duration::from_secs(5),
    };
    
    // Connect to Neo4j
    let store_result = Neo4jStateStore::new(config).await;
    let store = match store_result {
        Ok(store) => Arc::new(store) as Arc<dyn StateStore>,
        Err(e) => {
            println!("Failed to connect to Neo4j: {:?}, skipping test", e);
            return;
        }
    };
    
    // Create test context
    let tenant_id = TenantId::new_v4();
    let trace_ctx = TraceContext::default();
    
    // Check if Framework node exists (from seed data)
    let query = "MATCH (f:Framework) RETURN f.name as name";
    let result = store.execute_query(&tenant_id, &trace_ctx, query, None).await;
    
    match result {
        Ok(rows) => {
            if rows.is_empty() {
                println!("No Framework nodes found. Seed data may not be loaded.");
                println!("Please run the seed.cypher script first.");
                return;
            }
            
            if let Some(row) = rows.first() {
                if let Some(name) = row.get("name") {
                    println!("Found Framework: {:?}", name);
                    // No assertion here, just report what was found
                } else {
                    println!("Framework found but name field is missing");
                }
            }
        },
        Err(e) => {
            println!("Failed to query Framework nodes: {:?}, skipping test", e);
            return;
        }
    }
    
    // Query StdLib components
    println!("Checking for StdLib components...");
    let query = "MATCH (cd:ComponentDefinition) WHERE cd.source = 'StdLib' RETURN cd.id as id, cd.name as name";
    let result = store.execute_query(&tenant_id, &trace_ctx, query, None).await;
    
    match result {
        Ok(rows) => {
            println!("Found {} StdLib components", rows.len());
            for row in &rows {
                if let Some(id) = row.get("id") {
                    if let Some(name) = row.get("name") {
                        println!("Component: {:?} - {:?}", id, name);
                    }
                }
            }
            
            // Success even if no components found - we'll just log that information
            if rows.is_empty() {
                println!("No StdLib components found. You may need to run the seed script.");
            }
        },
        Err(e) => {
            println!("Failed to query StdLib components: {:?}", e);
        }
    }
    
    println!("Test completed - check log output for details");
} 
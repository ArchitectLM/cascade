//! Basic test for Neo4j connection
//!
//! This test verifies that we can connect to the Neo4j database.
//! It's useful as a first step to ensure the environment is properly set up.

use std::env;
use std::time::Duration;
use dotenv::dotenv;
use tracing::{info, warn};
use neo4rs::Query;

use cascade_kb::adapters::neo4j_store::{Neo4jStateStore, Neo4jConfig};

#[tokio::test]
async fn test_neo4j_connection() {
    // Initialize tracing
    let _ = tracing_subscriber::fmt()
        .with_env_filter("cascade_kb=debug,neo4j_connection_test=debug")
        .try_init();
    
    dotenv().ok(); // Load environment variables

    info!("Starting Neo4j connection test");
    
    // Read configuration from environment variables
    let uri = env::var("NEO4J_URI").unwrap_or_else(|_| "neo4j://localhost:17687".to_string());
    let username = env::var("NEO4J_USERNAME").unwrap_or_else(|_| "neo4j".to_string());
    let password = env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "password".to_string());
    let database = env::var("NEO4J_DATABASE").ok();
    let pool_size = env::var("NEO4J_POOL_SIZE").map(|s| s.parse::<usize>().unwrap_or(10)).unwrap_or(10);
    
    info!("Connecting to Neo4j at {} (db: {:?})", uri, database);
    
    let config = Neo4jConfig {
        uri: uri.clone(),
        username: username.clone(),
        password: password.clone(),
        database: database.clone(),
        pool_size,
        connection_timeout: Duration::from_secs(30),
        connection_retry_count: 3,
        connection_retry_delay: Duration::from_secs(2),
        query_timeout: Duration::from_secs(30),
    };
    
    // Attempt to connect to Neo4j
    match Neo4jStateStore::new(config).await {
        Ok(store) => {
            info!("Successfully connected to Neo4j");
            
            // Try running a simple query to verify the connection
            match store.graph.execute(neo4rs::query("RETURN 1 as test")).await {
                Ok(_) => {
                    info!("Successfully executed a test query");
                    
                    // Try checking for APOC extension
                    match store.graph.execute(neo4rs::query("CALL apoc.help('apoc')")).await {
                        Ok(_) => {
                            info!("APOC extension is installed and working");
                        },
                        Err(e) => {
                            warn!("APOC extension check failed: {}", e);
                            warn!("Some tests requiring APOC may fail");
                        }
                    }
                    
                    // Try checking for vector capabilities
                    match store.graph.execute(neo4rs::query("SHOW INDEXES YIELD type WHERE type = 'VECTOR' RETURN count(*) as count")).await {
                        Ok(mut result) => {
                            // Use the next() method instead of into_iter()
                            match result.next().await {
                                Ok(Some(row)) => {
                                    match row.get::<i64>("count") {
                                        Ok(count) => {
                                            if count > 0 {
                                                info!("Vector indexes are available ({} found)", count);
                                            } else {
                                                warn!("No vector indexes found. Vector search tests may use fallbacks");
                                            }
                                        },
                                        Err(_) => warn!("Could not extract count from row")
                                    }
                                },
                                Ok(None) => warn!("No results returned from vector capability check"),
                                Err(e) => warn!("Error getting row from result: {}", e)
                            }
                        },
                        Err(e) => {
                            warn!("Vector capability check failed: {}", e);
                            warn!("Vector search tests may use fallbacks");
                        }
                    }
                },
                Err(e) => {
                    panic!("Failed to execute test query: {}", e);
                }
            }
        },
        Err(e) => {
            panic!("Failed to connect to Neo4j at {}: {}", uri, e);
        }
    }
    
    info!("Neo4j connection test completed successfully");
} 
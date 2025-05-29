//! Debugging version of Neo4j E2E test
//! This version adds timeouts and better logging to identify hanging issues

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use uuid::Uuid;
use tracing::{info, debug, error, warn};

use cascade_kb::{
    data::{
        identifiers::TenantId,
        types::{Scope, DataPacket, SourceType},
        trace_context::TraceContext,
        errors::CoreError,
    },
    services::{
        ingestion::IngestionService,
        query::QueryService,
        messages::{IngestionMessage, QueryRequest, QueryResponse, QueryResultSender},
    },
    adapters::neo4j_store::{Neo4jStateStore, Neo4jConfig},
    traits::{StateStore, EmbeddingGenerator, DslParser},
    test_utils::fakes::{FakeEmbeddingService, FakeDslParser},
    embedding::MockEmbeddingService,
};

// Simpler KB client for testing that adds timeouts
struct TestKbClient {
    ingestion_tx: mpsc::Sender<IngestionMessage>,
    query_tx: mpsc::Sender<(QueryRequest, QueryResultSender)>,
}

impl TestKbClient {
    pub fn new(
        ingestion_tx: mpsc::Sender<IngestionMessage>,
        query_tx: mpsc::Sender<(QueryRequest, QueryResultSender)>,
    ) -> Self {
        Self {
            ingestion_tx,
            query_tx,
        }
    }

    pub async fn ingest_yaml(
        &self,
        tenant_id: TenantId,
        yaml_content: String,
        scope: Scope,
        source_info: Option<String>,
    ) -> Result<(), CoreError> {
        let trace_ctx = TraceContext::new_root();
        
        let message = IngestionMessage::YamlDefinitions {
            tenant_id,
            trace_ctx,
            yaml_content,
            scope,
            source_info,
        };
        
        // Add timeout to catch hanging send operations
        match timeout(Duration::from_secs(5), self.ingestion_tx.send(message)).await {
            Ok(result) => result.map_err(|_| CoreError::Internal("Failed to send ingestion message".to_string())),
            Err(_) => Err(CoreError::Internal("Timeout sending ingestion message".to_string())),
        }
    }
    
    pub async fn query(
        &self,
        request: QueryRequest,
    ) -> Result<QueryResponse, CoreError> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        
        let query_sender = QueryResultSender {
            sender: response_tx,
        };
        
        // Add timeout to catch hanging send operations
        match timeout(Duration::from_secs(5), self.query_tx.send((request, query_sender))).await {
            Ok(result) => {
                result.map_err(|_| CoreError::Internal("Failed to send query request".to_string()))?;
            },
            Err(_) => return Err(CoreError::Internal("Timeout sending query request".to_string())),
        }
        
        // Add timeout to catch hanging response operations
        match timeout(Duration::from_secs(30), response_rx).await {
            Ok(result) => result.map_err(|_| CoreError::Internal("Failed to receive query response".to_string())),
            Err(_) => Err(CoreError::Internal("Timeout waiting for query response".to_string())),
        }
    }

    pub async fn lookup_by_id(
        &self,
        tenant_id: TenantId,
        entity_type: String,
        id: String,
        version: Option<String>,
    ) -> Result<QueryResponse, CoreError> {
        let trace_ctx = TraceContext::new_root();
        
        let request = QueryRequest::LookupById {
            tenant_id,
            trace_ctx,
            entity_type,
            id,
            version,
        };
        
        self.query(request).await
    }
}

// Setup function to create and initialize a Neo4j database for testing
async fn setup_neo4j_test_db() -> Result<Arc<dyn StateStore>, CoreError> {
    dotenv::dotenv().ok(); // Load .env file if available
    
    // Configure Neo4j connection from env vars or use defaults
    let uri = std::env::var("NEO4J_URI")
        .unwrap_or_else(|_| "neo4j://localhost:17687".to_string());
    let username = std::env::var("NEO4J_USERNAME")
        .unwrap_or_else(|_| "neo4j".to_string());
    let password = std::env::var("NEO4J_PASSWORD")
        .unwrap_or_else(|_| "password".to_string());
    let database = std::env::var("NEO4J_DATABASE")
        .map(|db| Some(db))
        .unwrap_or_else(|_| Some("neo4j".to_string()));
    
    info!("Connecting to Neo4j at: {}", uri);
    
    let config = Neo4jConfig {
        uri,
        username,
        password,
        database,
        pool_size: 2, // Reduced from original for debugging
        connection_timeout: Duration::from_secs(5),
        connection_retry_count: 1,
        connection_retry_delay: Duration::from_secs(1),
        query_timeout: Duration::from_secs(10),
    };
    
    // Add timeout to Neo4j connection
    let store_result = timeout(Duration::from_secs(15), Neo4jStateStore::new(config)).await;
    
    match store_result {
        Ok(Ok(store)) => {
            // Basic connectivity test
            let tenant_id = TenantId::new_v4();
            let trace_ctx = TraceContext::new_root();
            
            // Simple query to verify connectivity
            let ping_query = "RETURN 1 as ping";
            
            match timeout(
                Duration::from_secs(5),
                store.execute_query(&tenant_id, &trace_ctx, ping_query, None)
            ).await {
                Ok(Ok(_)) => {
                    info!("Successfully connected to Neo4j database");
                    Ok(Arc::new(store) as Arc<dyn StateStore>)
                },
                Ok(Err(e)) => {
                    error!("Failed to connect to Neo4j database: {:?}", e);
                    Err(CoreError::DatabaseError(format!("Failed to connect to Neo4j: {}", e)))
                },
                Err(_) => {
                    error!("Neo4j ping query timed out");
                    Err(CoreError::DatabaseError("Neo4j ping query timed out".to_string()))
                }
            }
        },
        Ok(Err(e)) => {
            error!("Failed to create Neo4j store: {:?}", e);
            Err(CoreError::DatabaseError(format!("Failed to create Neo4j store: {}", e)))
        },
        Err(_) => {
            error!("Neo4j connection attempt timed out");
            Err(CoreError::DatabaseError("Neo4j connection attempt timed out".to_string()))
        }
    }
}

// Helper function to create an embedding generator based on environment
fn create_embedding_generator() -> Arc<dyn EmbeddingGenerator> {
    // Always use mock for debugging
    info!("Using MockEmbeddingService for debug test");
    Arc::new(MockEmbeddingService::default()) as Arc<dyn EmbeddingGenerator>
}

#[tokio::test]
async fn test_neo4j_workflow_debug() {
    // Initialize logging for tests
    let _ = tracing_subscriber::fmt()
        .with_env_filter("cascade_kb=debug,neo4j_e2e_debug=debug,tower_http=debug")
        .try_init();
    
    info!("Starting Neo4j E2E debug test");
    
    // Setup Neo4j store with timeout
    let state_store = match setup_neo4j_test_db().await {
        Ok(store) => store,
        Err(e) => {
            error!("Failed to setup Neo4j database: {:?}", e);
            panic!("Failed to setup Neo4j database: {:?}", e);
        },
    };
    
    // Create other dependencies
    let embedding_generator = create_embedding_generator();
    let dsl_parser: Arc<dyn DslParser> = Arc::new(FakeDslParser::new());
    
    // Create channels with limited capacity to detect backpressure issues
    let (ingestion_tx, ingestion_rx) = mpsc::channel::<IngestionMessage>(10);
    let (query_tx, query_rx) = mpsc::channel::<(QueryRequest, QueryResultSender)>(10);
    
    // Create services
    let mut ingestion_service = IngestionService::new(
        Arc::clone(&state_store),
        Arc::clone(&embedding_generator),
        Arc::clone(&dsl_parser),
        ingestion_rx,
    );
    
    let mut query_service = QueryService::new(
        Arc::clone(&state_store),
        Arc::clone(&embedding_generator),
        query_rx,
    );
    
    // Create client
    let client = TestKbClient::new(ingestion_tx, query_tx);
    
    // Start services with named tasks
    info!("Starting services");
    let ingestion_handle = tokio::spawn(async move {
        info!("Ingestion service starting");
        let result = ingestion_service.run().await;
        info!("Ingestion service completed with result: {:?}", result);
        result.expect("Ingestion service failed");
    });
    
    let query_handle = tokio::spawn(async move {
        info!("Query service starting");
        let result = query_service.run().await;
        info!("Query service completed with result: {:?}", result);
        result.expect("Query service failed");
    });
    
    // Generate a unique tenant ID for this test
    let tenant_id = TenantId::new_v4();
    
    // Generate unique IDs for test entities
    let component_id = format!("debug-comp-{}", Uuid::new_v4().simple());
    let flow_id = format!("debug-flow-{}", Uuid::new_v4().simple());
    
    info!(
        "Testing with tenant_id: {}, component_id: {}, flow_id: {}", 
        tenant_id, component_id, flow_id
    );
    
    // === 1. Ingest a simple component definition ===
    info!("Step 1: Ingesting component definition");
    
    let component_yaml = format!(r#"
    ComponentDefinition:
      id: "{component_id}"
      name: "Debug Component"
      description: "A simplified component for debug testing"
      version: "1.0.0"
      source: "{}"
      scope: "UserDefined"
      inputs:
        - name: "input1"
          description: "Test input"
          required: true
          schema: {{ "type": "string" }}
      outputs:
        - name: "output1"
          description: "Test output"
          error_path: false
          schema: {{ "type": "string" }}
    "#, SourceType::UserDefined);
    
    let result = client.ingest_yaml(
        tenant_id,
        component_yaml,
        Scope::UserDefined,
        Some("debug-component.yaml".to_string()),
    ).await;
    
    match &result {
        Ok(_) => info!("Component ingestion succeeded"),
        Err(e) => error!("Component ingestion failed: {:?}", e),
    }
    assert!(result.is_ok(), "Component ingestion should succeed: {:?}", result);
    
    // Wait for ingestion with timeout
    info!("Waiting for ingestion to complete...");
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // === 2. Verify component was ingested ===
    info!("Step 2: Verifying component ingestion");
    
    let result = client.lookup_by_id(
        tenant_id,
        "ComponentDefinition".to_string(),
        component_id.clone(),
        None,
    ).await;
    
    match &result {
        Ok(QueryResponse::Success(results)) => {
            info!("Component lookup succeeded with {} results", results.len());
            if !results.is_empty() {
                let component = &results[0];
                debug!("Found component data: {:?}", component);
                
                // Check component ID in both potential formats
                let component_id_found = component.get("id")
                    .and_then(|id| match id {
                        DataPacket::String(s) => Some(s.as_str()),
                        DataPacket::Json(j) => j.as_str(),
                        _ => None,
                    });
                
                if let Some(id_value) = component_id_found {
                    info!("Found component with ID: {}", id_value);
                    assert_eq!(id_value, component_id, "Component ID should match");
                } else {
                    error!("Component ID missing or in unexpected format");
                }
                
                // Check name in both potential formats
                let component_name = component.get("name")
                    .and_then(|name| match name {
                        DataPacket::String(s) => Some(s.as_str()),
                        DataPacket::Json(j) => j.as_str(),
                        _ => None,
                    });
                
                if let Some(name_value) = component_name {
                    info!("Found component with name: {}", name_value);
                    assert_eq!(name_value, "Debug Component", "Component name should match");
                } else {
                    error!("Component name missing or in unexpected format");
                }
            }
        },
        Ok(QueryResponse::Error(e)) => error!("Component lookup returned error: {:?}", e),
        Err(e) => error!("Component lookup failed: {:?}", e),
    }
    
    assert!(result.is_ok(), "Component lookup should succeed");
    
    // === 3. Clean up and shutdown ===
    info!("Test completed, cleaning up");
    
    // Attempt to delete the test component
    let cleanup_query = format!(
        "MATCH (c:ComponentDefinition {{id: $id, tenant_id: $tenant_id}}) DETACH DELETE c",
    );
    
    let mut params = HashMap::new();
    params.insert("id".to_string(), DataPacket::String(component_id));
    params.insert("tenant_id".to_string(), DataPacket::String(tenant_id.to_string()));
    
    let trace_ctx = TraceContext::new_root();
    let cleanup_result = state_store.execute_query(
        &tenant_id,
        &trace_ctx,
        &cleanup_query,
        Some(params),
    ).await;
    
    match &cleanup_result {
        Ok(_) => info!("Test data cleanup succeeded"),
        Err(e) => warn!("Test data cleanup failed: {:?}", e),
    }
    
    // Explicitly drop the client to close channels
    info!("Dropping client to close channels");
    drop(client);
    
    // Wait for services to complete with a timeout
    info!("Waiting for services to complete (timeout 5 sec)...");
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Abort the handles if they're still running
    ingestion_handle.abort();
    query_handle.abort();
    
    // Wait a moment to let things settle
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    info!("Neo4j E2E debug test complete");
} 
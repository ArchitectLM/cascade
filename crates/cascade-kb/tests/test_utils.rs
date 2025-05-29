use std::sync::Arc;
use std::time::Duration;
use std::env;
use tokio::time::timeout;
use tracing::{info, error, debug, warn};
use dotenv::dotenv;
use std::collections::HashMap;
use uuid::Uuid;

use cascade_kb::{
    data::{
        identifiers::TenantId,
        trace_context::TraceContext,
        types::{DataPacket, Scope, SourceType, DataPacketMapExt},
    },
    adapters::neo4j_store::{Neo4jStateStore, Neo4jConfig},
    traits::{StateStore, EmbeddingGenerator},
    test_utils::fakes::{FakeStateStore, FakeEmbeddingService},
    embedding::{EmbeddingService, Neo4jEmbeddingService, Neo4jEmbeddingServiceConfig, MockEmbeddingService},
};

/// Sets up a Neo4j StateStore from environment variables or defaults.
/// Returns an Arc<dyn StateStore> that can be used in tests.
pub async fn setup_neo4j_store() -> Result<Arc<dyn StateStore>, String> {
    // Load environment variables
    dotenv().ok();
    
    // Get connection details from environment variables
    let uri = env::var("NEO4J_URI")
        .unwrap_or_else(|_| "neo4j://localhost:7687".to_string());
    
    let username = env::var("NEO4J_USERNAME")
        .unwrap_or_else(|_| "neo4j".to_string());
    
    let password = match env::var("NEO4J_PASSWORD") {
        Ok(pass) => pass,
        Err(_) => {
            error!("NEO4J_PASSWORD environment variable not set");
            return Err("NEO4J_PASSWORD not set".to_string());
        }
    };
    
    let database = env::var("NEO4J_DATABASE").ok();
    let pool_size = env::var("NEO4J_POOL_SIZE")
        .map(|s| s.parse::<usize>().unwrap_or(5))
        .unwrap_or(5);
    
    info!("Connecting to Neo4j at: {}", uri);
    
    // Create Neo4j configuration
    let config = Neo4jConfig {
        uri,
        username,
        password,
        database,
        pool_size,
        connection_timeout: Duration::from_secs(5),
        connection_retry_count: 3,
        connection_retry_delay: Duration::from_secs(1),
        query_timeout: Duration::from_secs(10),
    };
    
    // Connect to Neo4j with timeout
    match timeout(Duration::from_secs(15), Neo4jStateStore::new(config)).await {
        Ok(Ok(store)) => {
            // Test the connection
            let tenant_id = TenantId::new_v4();
            let trace_ctx = TraceContext::default();
            let test_query = "RETURN 1 as result";
            
            match store.execute_query(&tenant_id, &trace_ctx, test_query, None).await {
                Ok(_) => {
                    info!("Successfully connected to Neo4j");
                    Ok(Arc::new(store) as Arc<dyn StateStore>)
                },
                Err(e) => {
                    error!("Failed to execute test query: {}", e);
                    Err(format!("Connection test failed: {}", e))
                }
            }
        },
        Ok(Err(e)) => {
            error!("Failed to connect to Neo4j: {}", e);
            Err(format!("Connection failed: {}", e))
        },
        Err(_) => {
            error!("Connection to Neo4j timed out");
            Err("Connection timeout".to_string())
        }
    }
}

/// Initializes the Neo4j database with required schema and indexes
pub async fn initialize_neo4j_schema(store: &Arc<dyn StateStore>) -> Result<(), String> {
    let tenant_id = TenantId::new_v4();
    let trace_ctx = TraceContext::default();
    
    // Create indexes
    let index_queries = [
        "CREATE INDEX versionSet_entityId_idx IF NOT EXISTS FOR (vs:VersionSet) ON (vs.entityId)",
        "CREATE INDEX version_id_idx IF NOT EXISTS FOR (v:Version) ON (v._unique_version_id)",
        "CREATE INDEX compDef_name_idx IF NOT EXISTS FOR (cd:ComponentDefinition) ON (cd.name)",
        "CREATE INDEX compDef_type_idx IF NOT EXISTS FOR (cd:ComponentDefinition) ON (cd.component_type_id)",
        "CREATE INDEX scope_idx_comp IF NOT EXISTS FOR (cd:ComponentDefinition) ON (cd.scope)",
        "CREATE INDEX scope_idx_flow IF NOT EXISTS FOR (fd:FlowDefinition) ON (fd.scope)",
        "CREATE INDEX scope_idx_doc IF NOT EXISTS FOR (dc:DocumentationChunk) ON (dc.scope)",
        "CREATE INDEX scope_idx_code IF NOT EXISTS FOR (ce:CodeExample) ON (ce.scope)",
        "CREATE INDEX docChunk_id_idx IF NOT EXISTS FOR (d:DocumentationChunk) ON (d.id)",
        "CREATE INDEX codeExample_id_idx IF NOT EXISTS FOR (ex:CodeExample) ON (ex.id)",
        "CREATE INDEX tenant_id_idx_comp IF NOT EXISTS FOR (cd:ComponentDefinition) ON (cd.tenant_id)",
        "CREATE INDEX tenant_id_idx_flow IF NOT EXISTS FOR (fd:FlowDefinition) ON (fd.tenant_id)",
        "CREATE INDEX tenant_id_idx_doc IF NOT EXISTS FOR (dc:DocumentationChunk) ON (dc.tenant_id)",
        "CREATE INDEX tenant_id_idx_code IF NOT EXISTS FOR (ce:CodeExample) ON (ce.tenant_id)",
    ];
    
    for query in index_queries {
        if let Err(e) = store.execute_query(&tenant_id, &trace_ctx, query, None).await {
            error!("Failed to create index: {}", e);
            return Err(format!("Schema initialization failed: {}", e));
        }
    }
    
    // Try to create the vector index - may fail if Neo4j version doesn't support it
    let vector_index_query = "CALL db.index.vector.createNodeIndex('docEmbeddings', 'DocumentationChunk', 'embedding', 1536, 'cosine')";
    
    match store.execute_query(&tenant_id, &trace_ctx, vector_index_query, None).await {
        Ok(_) => info!("Created vector index"),
        Err(e) => {
            // Don't fail the entire initialization if vector index creation fails
            // Some Neo4j versions or configurations might not support it
            info!("Vector index creation failed (this is ok if using FakeStateStore or Neo4j without vector plugin): {}", e);
        }
    }
    
    // Create Framework node
    let framework_query = "MERGE (f:Framework {name: 'Cascade Platform'}) RETURN f";
    if let Err(e) = store.execute_query(&tenant_id, &trace_ctx, framework_query, None).await {
        error!("Failed to create Framework node: {}", e);
        return Err(format!("Schema initialization failed: {}", e));
    }
    
    // Create DSL spec node
    let dsl_spec_query = r#"
    MERGE (spec:DSLSpec {version: '1.0'}) 
    ON CREATE SET spec += { 
        core_keywords: ['dsl_version', 'definitions', 'components', 'flows', 'trigger', 'steps'], 
        spec_url: 'https://docs.cascade.io/dsl/v1.0', 
        created_at: datetime() 
    }
    RETURN spec"#;
    
    if let Err(e) = store.execute_query(&tenant_id, &trace_ctx, dsl_spec_query, None).await {
        error!("Failed to create DSLSpec node: {}", e);
        return Err(format!("Schema initialization failed: {}", e));
    }
    
    // Connect Framework to DSLSpec
    let connect_query = "MATCH (f:Framework {name: 'Cascade Platform'}), (spec:DSLSpec {version: '1.0'}) MERGE (f)-[:DEFINES]->(spec) RETURN f, spec";
    if let Err(e) = store.execute_query(&tenant_id, &trace_ctx, connect_query, None).await {
        error!("Failed to connect Framework to DSLSpec: {}", e);
        return Err(format!("Schema initialization failed: {}", e));
    }
    
    info!("Neo4j schema initialized successfully");
    Ok(())
}

/// Creates a fake state store for testing when Neo4j is not available
pub fn create_fake_store() -> Arc<dyn StateStore> {
    Arc::new(FakeStateStore::new()) as Arc<dyn StateStore>
}

/// Tries to connect to Neo4j, falls back to FakeStateStore if not available
/// Initializes the schema if Neo4j is available
pub async fn get_test_store() -> Arc<dyn StateStore> {
    dotenv().ok();
    
    // Check if we should use fake store
    let use_fake = std::env::var("USE_FAKE_STORE")
        .map(|val| val == "true")
        .unwrap_or(false);
    
    if use_fake {
        info!("Using FakeStateStore as requested by environment variable");
        return Arc::new(FakeStateStore::new()) as Arc<dyn StateStore>;
    }
    
    // Try to connect to Docker Neo4j first
    let uri = env::var("NEO4J_URI")
        .unwrap_or_else(|_| "neo4j://localhost:17687".to_string());
    let username = env::var("NEO4J_USERNAME")
        .unwrap_or_else(|_| "neo4j".to_string());
    let password = env::var("NEO4J_PASSWORD")
        .unwrap_or_else(|_| "password".to_string());
    let database = env::var("NEO4J_DATABASE").ok();
    let pool_size = env::var("NEO4J_POOL_SIZE")
        .map(|s| s.parse::<usize>().unwrap_or(10))
        .unwrap_or(10);
    
    info!("Connecting to Neo4j at {} for testing", uri);
    
    let config = Neo4jConfig {
        uri,
        username,
        password,
        database,
        pool_size,
        connection_timeout: Duration::from_secs(5),
        connection_retry_count: 3,
        connection_retry_delay: Duration::from_secs(1),
        query_timeout: Duration::from_secs(10),
    };
    
    // Attempt to connect with timeout
    match timeout(Duration::from_secs(10), Neo4jStateStore::new(config)).await {
        Ok(Ok(store)) => {
            info!("Successfully connected to Neo4j for testing");
            
            // Verify connectivity with simple query
            let tenant_id = TenantId::new_v4();
            let trace_ctx = TraceContext::new_root();
            match store.execute_query(&tenant_id, &trace_ctx, "RETURN 1 as test", None).await {
                Ok(_) => {
                    info!("Neo4j connection verified with test query");
                    // Initialize schema using the store directly
                    let store_arc = Arc::new(store) as Arc<dyn StateStore>;
                    if let Err(e) = initialize_neo4j_schema(&store_arc).await {
                        error!("Failed to initialize Neo4j schema: {}", e);
                        warn!("Some tests may fail due to missing schema");
                    }
                    store_arc
                },
                Err(e) => {
                    error!("Neo4j connection succeeded but test query failed: {}", e);
                    warn!("Falling back to FakeStateStore");
                    Arc::new(FakeStateStore::new()) as Arc<dyn StateStore>
                }
            }
        },
        Ok(Err(e)) => {
            error!("Failed to connect to Neo4j: {}", e);
            warn!("Falling back to FakeStateStore");
            Arc::new(FakeStateStore::new()) as Arc<dyn StateStore>
        },
        Err(_) => {
            error!("Connection attempt to Neo4j timed out");
            warn!("Falling back to FakeStateStore");
            Arc::new(FakeStateStore::new()) as Arc<dyn StateStore>
        }
    }
}

/// Cleanup test data for a specific tenant
pub async fn cleanup_tenant_data(store: &Arc<dyn StateStore>, tenant_id: &TenantId) -> Result<(), String> {
    let trace_ctx = TraceContext::default();
    
    // Query to delete all nodes with the specified tenant_id
    let cleanup_query = r#"
    MATCH (n) 
    WHERE n.tenant_id = $tenant_id
    DETACH DELETE n
    "#;
    
    // Create parameters with tenant_id
    let mut params = HashMap::new();
    params.insert("tenant_id".to_string(), DataPacket::Json(serde_json::json!(tenant_id.to_string())));
    
    match store.execute_query(tenant_id, &trace_ctx, cleanup_query, Some(params)).await {
        Ok(_) => {
            info!("Successfully cleaned up data for tenant {}", tenant_id);
            Ok(())
        },
        Err(e) => {
            error!("Failed to clean up tenant data: {}", e);
            Err(format!("Cleanup failed: {}", e))
        }
    }
}

/// Helper function to safely extract a string from a DataPacket row
/// Handles different DataPacket variants appropriately
pub fn extract_string(row: &HashMap<String, DataPacket>, key: &str) -> String {
    match row.get(key) {
        Some(DataPacket::String(s)) => s.clone(),
        Some(DataPacket::Json(value)) => {
            if let Some(s) = value.as_str() {
                s.to_string()
            } else {
                value.to_string()
            }
        },
        Some(packet) => packet.as_string(),
        None => String::new(),
    }
}

/// Helper function to safely extract an integer from a DataPacket row
/// Handles different DataPacket variants appropriately
pub fn extract_i64(row: &HashMap<String, DataPacket>, key: &str) -> i64 {
    match row.get(key) {
        Some(DataPacket::Number(n)) => *n as i64,
        Some(DataPacket::Json(value)) => {
            if let Some(n) = value.as_i64() {
                n
            } else if let Some(n) = value.as_f64() {
                n as i64
            } else {
                0
            }
        },
        Some(packet) => packet.to_i64(),
        None => 0,
    }
}

/// Helper function to safely extract a boolean from a DataPacket row
/// Handles different DataPacket variants appropriately
pub fn extract_bool(row: &HashMap<String, DataPacket>, key: &str) -> bool {
    match row.get(key) {
        Some(DataPacket::Bool(b)) => *b,
        Some(DataPacket::Json(value)) => {
            if let Some(b) = value.as_bool() {
                b
            } else {
                false
            }
        },
        Some(packet) => packet.to_bool(),
        None => false,
    }
}

/// Helper to create a test component definition in the database
pub async fn create_test_component(
    store: &Arc<dyn StateStore>, 
    tenant_id: &TenantId, 
    component_name: &str
) -> Result<String, String> {
    let trace_ctx = TraceContext::default();
    let component_id = Uuid::new_v4().to_string();
    
    // Create a basic component definition
    let query = r#"
    CREATE (vs:VersionSet {
        id: $vs_id,
        entity_id: $entity_id,
        entity_type: 'ComponentDefinition',
        tenant_id: $tenant_id
    })
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
    RETURN cd.id as component_id
    "#;
    
    let mut params = HashMap::new();
    params.insert("vs_id".to_string(), DataPacket::Json(serde_json::json!(Uuid::new_v4().to_string())));
    params.insert("v_id".to_string(), DataPacket::Json(serde_json::json!(Uuid::new_v4().to_string())));
    params.insert("cd_id".to_string(), DataPacket::Json(serde_json::json!(component_id.clone())));
    params.insert("entity_id".to_string(), DataPacket::Json(serde_json::json!(component_name)));
    params.insert("name".to_string(), DataPacket::Json(serde_json::json!(component_name)));
    params.insert("tenant_id".to_string(), DataPacket::Json(serde_json::json!(tenant_id.to_string())));
    
    match store.execute_query(tenant_id, &trace_ctx, query, Some(params)).await {
        Ok(_) => {
            info!("Created test component {} for tenant {}", component_name, tenant_id);
            Ok(component_id)
        },
        Err(e) => {
            error!("Failed to create test component: {}", e);
            Err(format!("Component creation failed: {}", e))
        }
    }
}

/// Initialize tracing for tests with a default configuration
pub fn init_test_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("cascade_kb=debug,test=debug")
        .try_init();
}

/// Creates an embedding service for testing, preferring Neo4j over mock
pub async fn get_test_embedding_service() -> Arc<dyn EmbeddingService> {
    dotenv().ok();
    
    // Check if we're required to use mocks
    let use_mock = std::env::var("USE_MOCK_EMBEDDING")
        .map(|val| val == "true")
        .unwrap_or(false);
    
    if use_mock {
        info!("Using MockEmbeddingService as requested by environment variable");
        return Arc::new(MockEmbeddingService::default()) as Arc<dyn EmbeddingService>;
    }
    
    // Try to use Neo4j Docker with its embedding capabilities
    let uri = env::var("NEO4J_URI")
        .unwrap_or_else(|_| "neo4j://localhost:17687".to_string());
    let username = env::var("NEO4J_USERNAME")
        .unwrap_or_else(|_| "neo4j".to_string());
    let password = env::var("NEO4J_PASSWORD")
        .unwrap_or_else(|_| "password".to_string());
    
    info!("Trying to create Neo4j embedding service for testing");
    
    let config = Neo4jEmbeddingServiceConfig {
        uri,
        username,
        password,
        model_name: "text-embedding-ada-002".to_string(),
        embedding_dimension: 1536,
        index_name: "docEmbeddings".to_string(),
    };
    
    match timeout(
        Duration::from_secs(5), 
        Neo4jEmbeddingService::new(&config)
    ).await {
        Ok(Ok(service)) => {
            // Verify service operation with a test
            let trace_ctx = TraceContext::new_root();
            match timeout(
                Duration::from_secs(5),
                service.embed_text("test embedding", &trace_ctx)
            ).await {
                Ok(Ok(_)) => {
                    info!("Successfully created Neo4j embedding service");
                    Arc::new(service) as Arc<dyn EmbeddingService>
                },
                Ok(Err(e)) => {
                    warn!("Neo4j embedding service failed to generate test embedding: {}", e);
                    warn!("This is likely because the APOC ML plugin is not installed or properly configured");
                    warn!("Falling back to mock embedding service");
                    Arc::new(MockEmbeddingService::default()) as Arc<dyn EmbeddingService>
                },
                Err(_) => {
                    warn!("Neo4j embedding service test timed out");
                    warn!("Falling back to mock embedding service");
                    Arc::new(MockEmbeddingService::default()) as Arc<dyn EmbeddingService>
                }
            }
        },
        Ok(Err(e)) => {
            warn!("Failed to create Neo4j embedding service: {}", e);
            warn!("Falling back to mock embedding service");
            Arc::new(MockEmbeddingService::default()) as Arc<dyn EmbeddingService>
        },
        Err(_) => {
            warn!("Neo4j embedding service creation timed out");
            warn!("Falling back to mock embedding service");
            Arc::new(MockEmbeddingService::default()) as Arc<dyn EmbeddingService>
        }
    }
}

/// Creates a compatible pair of embedding service and generator for testing
pub fn get_test_embedding_service_pair() -> (Arc<dyn EmbeddingService>, Arc<dyn EmbeddingGenerator>) {
    // Create a pair of compatible embedding services
    // This ensures both EmbeddingService and EmbeddingGenerator traits are properly implemented
    // and return consistent results
    let mock = MockEmbeddingService::default();
    (Arc::new(mock.clone()) as Arc<dyn EmbeddingService>, 
     Arc::new(mock) as Arc<dyn EmbeddingGenerator>)
}

/// Gets a tenant ID for testing, either from environment or random
pub fn get_test_tenant_id() -> TenantId {
    dotenv().ok();
    
    // Get tenant ID from environment if available, otherwise generate a random one
    match env::var("TEST_TENANT_ID") {
        Ok(id_str) => {
            match Uuid::parse_str(&id_str) {
                Ok(id) => TenantId(id),
                Err(_) => {
                    warn!("Invalid TEST_TENANT_ID format, using random instead");
                    TenantId::new_v4()
                }
            }
        },
        Err(_) => TenantId::new_v4()
    }
}

/// Creates a properly configured Neo4jConfig from environment variables
/// Centralizes configuration logic to ensure consistency across tests
pub fn create_neo4j_config() -> Neo4jConfig {
    dotenv().ok(); // Load environment variables
    
    // Get connection details with proper defaults
    let uri = env::var("NEO4J_URI")
        .unwrap_or_else(|_| "neo4j://localhost:17687".to_string());
    let username = env::var("NEO4J_USERNAME")
        .unwrap_or_else(|_| "neo4j".to_string());
    let password = env::var("NEO4J_PASSWORD")
        .unwrap_or_else(|_| "password".to_string());
    let database = env::var("NEO4J_DATABASE").ok();
    let pool_size = env::var("NEO4J_POOL_SIZE")
        .map(|s| s.parse::<usize>().unwrap_or(10))
        .unwrap_or(10);
    
    // Set reasonable timeouts and retry settings
    Neo4jConfig {
        uri,
        username,
        password,
        database,
        pool_size,
        connection_timeout: Duration::from_secs(5),
        connection_retry_count: 3,
        connection_retry_delay: Duration::from_secs(1),
        query_timeout: Duration::from_secs(30),
    }
}

/// Creates a mock or fake embedding generator based on environment settings
pub fn create_test_embedding_generator() -> Arc<dyn EmbeddingGenerator> {
    // Check environment variable to determine if we should use mock
    let use_mock = std::env::var("USE_MOCK_EMBEDDING")
        .map(|val| val == "true")
        .unwrap_or(true); // Default to true for safety in tests
    
    if use_mock {
        info!("Using MockEmbeddingService for tests");
        Arc::new(MockEmbeddingService::default()) as Arc<dyn EmbeddingGenerator>
    } else {
        info!("Using FakeEmbeddingService for tests (with deterministic output)");
        Arc::new(FakeEmbeddingService::new()) as Arc<dyn EmbeddingGenerator>
    }
}

/// Creates and initializes vector indexes in Neo4j for testing
pub async fn ensure_vector_indexes(store: &Neo4jStateStore) -> Result<(), String> {
    let tenant_id = TenantId::new_v4();
    let trace_ctx = TraceContext::default();
    
    // Create vector indexes
    let indexes = vec![
        "docEmbeddings",
        "contentEmbeddings", 
        "testEmbeddings"
    ];
    
    for index_name in indexes {
        match store.ensure_vector_index_exists(index_name).await {
            Ok(_) => {
                info!("Vector index '{}' created or verified", index_name);
            },
            Err(e) => {
                warn!("Failed to create vector index '{}': {}", index_name, e);
                warn!("Vector search tests may fail, but fallback mechanism should work");
                // Don't fail the entire process as the store has fallbacks
            }
        }
    }
    
    // Create test embedding node with known value for verification
    let test_query = r#"
    MERGE (t:TestEmbedding {id: $id})
    SET t.tenant_id = $tenant_id,
        t.text = $text,
        t.embedding = $embedding
    RETURN t.id
    "#;
    
    let mut params = HashMap::new();
    params.insert("id".to_string(), DataPacket::String(Uuid::new_v4().to_string()));
    params.insert("tenant_id".to_string(), DataPacket::String(tenant_id.to_string()));
    params.insert("text".to_string(), DataPacket::String("This is a test embedding node for vector search validation".to_string()));
    
    // Create a simple embedding vector for testing - 1536 dimensions
    let test_embedding: Vec<f32> = (0..1536).map(|i| (i % 100) as f32 / 100.0).collect();
    params.insert("embedding".to_string(), DataPacket::FloatArray(test_embedding));
    
    match store.execute_query(&tenant_id, &trace_ctx, test_query, Some(params)).await {
        Ok(_) => {
            info!("Test embedding node created successfully");
        },
        Err(e) => {
            warn!("Failed to create test embedding node: {}", e);
            warn!("Vector search tests may use fallback mechanism");
            // Don't fail the entire process as the store has fallbacks
        }
    }
    
    // Create a DocumentationChunk with embedding for semantic search tests
    let doc_query = r#"
    MERGE (d:DocumentationChunk {id: $id})
    SET d.tenant_id = $tenant_id,
        d.text = $text,
        d.scope = $scope,
        d.embedding = $embedding,
        d.created_at = datetime()
    RETURN d.id
    "#;
    
    let mut doc_params = HashMap::new();
    let doc_id = Uuid::new_v4().to_string();
    doc_params.insert("id".to_string(), DataPacket::String(doc_id.clone()));
    doc_params.insert("tenant_id".to_string(), DataPacket::String(tenant_id.to_string()));
    doc_params.insert("text".to_string(), DataPacket::String("This is a test documentation chunk about authentication and API configuration".to_string()));
    doc_params.insert("scope".to_string(), DataPacket::String("General".to_string()));
    
    // Create an embedding vector optimized for "authentication" related searches
    let doc_embedding: Vec<f32> = (0..1536).map(|i| if i < 10 { 0.9 } else { (i % 100) as f32 / 100.0 }).collect();
    doc_params.insert("embedding".to_string(), DataPacket::FloatArray(doc_embedding));
    
    match store.execute_query(&tenant_id, &trace_ctx, doc_query, Some(doc_params)).await {
        Ok(_) => {
            info!("Test documentation chunk created successfully with ID: {}", doc_id);
        },
        Err(e) => {
            warn!("Failed to create test documentation chunk: {}", e);
            warn!("Semantic search tests may use fallback mechanism");
        }
    }
    
    Ok(())
} 
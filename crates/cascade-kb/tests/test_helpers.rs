//! Shared test helper functions for Cascade KB tests
//!
//! This module provides utility functions for setting up test environments
//! consistently across tests. It includes helpers for creating Neo4j
//! connections, embedding generators, and test data.

use std::sync::Arc;
use std::collections::HashMap;
use dotenv::dotenv;
use tracing::{info, warn};
use tokio::sync::mpsc;
use uuid::Uuid;

use cascade_kb::{
    traits::{StateStore, DslParser, EmbeddingGenerator},
    data::{
        TenantId,
        entities::DocumentationChunk,
        identifiers::DocumentationChunkId,
        types::{DataPacket, Scope},
        trace_context::TraceContext,
    },
    embedding::MockEmbeddingService,
    adapters::neo4j_store::{Neo4jStateStore, Neo4jConfig},
    test_utils::fakes::{FakeStateStore, FakeEmbeddingService, FakeDslParser},
    services::{
        ingestion::IngestionService,
        query::QueryService,
        messages::{IngestionMessage, QueryRequest, QueryResponse, QueryResultSender},
    },
};

/// Configuration for testing
pub struct TestConfig {
    /// Whether to use a real Neo4j connection
    pub use_real_neo4j: bool,
    /// Whether to use a mock embedding service
    pub use_mock_embedding: bool,
    /// Whether to use a predictable embedding generator
    pub use_predictable_embedding: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        // Load environment variables
        dotenv().ok();
        
        // Parse environment configuration
        let use_real_neo4j = std::env::var("USE_REAL_NEO4J")
            .map(|val| val.to_lowercase() == "true")
            .unwrap_or(false);
            
        let use_mock_embedding = std::env::var("USE_MOCK_EMBEDDING")
            .map(|val| val.to_lowercase() == "true")
            .unwrap_or(true); // Default to mock for safety
            
        let use_predictable_embedding = std::env::var("USE_PREDICTABLE_EMBEDDING")
            .map(|val| val.to_lowercase() == "true")
            .unwrap_or(false);
            
        TestConfig {
            use_real_neo4j,
            use_mock_embedding,
            use_predictable_embedding,
        }
    }
}

/// Sets up standard test configuration including state store, embedding, parser
pub async fn setup_test_services() -> (
    Arc<dyn StateStore>,
    Arc<dyn EmbeddingGenerator>,
    Arc<dyn DslParser>,
    TestConfig
) {
    // Get test configuration
    let config = TestConfig::default();
    
    // Set up state store
    let state_store = if config.use_real_neo4j {
        setup_neo4j_store().await
    } else {
        info!("Using FakeStateStore for test");
        Arc::new(FakeStateStore::new()) as Arc<dyn StateStore>
    };
    
    // Set up embedding generator
    let embedding_generator = if config.use_mock_embedding {
        info!("Using MockEmbeddingService for test");
        Arc::new(MockEmbeddingService::default()) as Arc<dyn EmbeddingGenerator>
    } else if config.use_predictable_embedding {
        info!("Using PredictableEmbeddingGenerator for test");
        Arc::new(FakeEmbeddingService::new()) as Arc<dyn EmbeddingGenerator>
    } else {
        info!("Using standard FakeEmbeddingGenerator for test");
        Arc::new(FakeEmbeddingService::new()) as Arc<dyn EmbeddingGenerator>
    };
    
    // Set up DSL parser
    let dsl_parser = Arc::new(FakeDslParser::new()) as Arc<dyn DslParser>;
    
    (state_store, embedding_generator, dsl_parser, config)
}

/// Sets up a Neo4j state store from environment variables
async fn setup_neo4j_store() -> Arc<dyn StateStore> {
    let config = create_neo4j_config();
    
    info!("Connecting to Neo4j at: {}", config.uri);
    
    match Neo4jStateStore::new(config).await {
        Ok(store) => {
            info!("Successfully connected to Neo4j");
            Arc::new(store)
        },
        Err(e) => {
            warn!("Failed to connect to Neo4j: {}, falling back to FakeStateStore", e);
            Arc::new(FakeStateStore::new())
        }
    }
}

/// Creates a properly configured Neo4jConfig from environment variables
pub fn create_neo4j_config() -> Neo4jConfig {
    dotenv().ok(); // Load environment variables
    
    // Get connection details with proper defaults
    let uri = std::env::var("NEO4J_URI")
        .unwrap_or_else(|_| "neo4j://localhost:17687".to_string());
    let username = std::env::var("NEO4J_USERNAME")
        .unwrap_or_else(|_| "neo4j".to_string());
    let password = std::env::var("NEO4J_PASSWORD")
        .unwrap_or_else(|_| "password".to_string());
    let database = std::env::var("NEO4J_DATABASE").ok();
    let pool_size = std::env::var("NEO4J_POOL_SIZE")
        .map(|s| s.parse::<usize>().unwrap_or(10))
        .unwrap_or(10);
    
    // Set reasonable timeouts and retry settings
    Neo4jConfig {
        uri,
        username,
        password,
        database,
        pool_size,
        connection_timeout: std::time::Duration::from_secs(5),
        connection_retry_count: 3,
        connection_retry_delay: std::time::Duration::from_secs(1),
        query_timeout: std::time::Duration::from_secs(30),
    }
}

/// Sets up the ingestion and query services with channels
pub fn setup_kb_services(
    state_store: Arc<dyn StateStore>,
    embedding_generator: Arc<dyn EmbeddingGenerator>,
    dsl_parser: Arc<dyn DslParser>
) -> (
    mpsc::Sender<IngestionMessage>,
    mpsc::Sender<(QueryRequest, QueryResultSender)>,
    tokio::task::JoinHandle<()>,
    tokio::task::JoinHandle<()>
) {
    // Create channels for service communication
    let (ingestion_tx, ingestion_rx) = mpsc::channel::<IngestionMessage>(100);
    let (query_tx, query_rx) = mpsc::channel::<(QueryRequest, QueryResultSender)>(100);
    
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
    
    // Start services
    let ingestion_handle = tokio::spawn(async move {
        info!("Ingestion service starting");
        ingestion_service.run().await.expect("Ingestion service failed");
    });
    
    let query_handle = tokio::spawn(async move {
        info!("Query service starting");
        query_service.run().await.expect("Query service failed");
    });
    
    (ingestion_tx, query_tx, ingestion_handle, query_handle)
}

/// Generate test documentation chunks with realistic data
pub fn generate_test_docs(_tenant_id: &TenantId) -> Vec<DocumentationChunk> {
    // Create documentation samples covering different topics
    let docs = vec![
        (
            "Database connection pooling optimize queries",
            "Learn how to optimize your database connection pooling in Cascade. Proper connection management can significantly improve performance by reusing connections and reducing overhead. Set appropriate pool sizes based on your workload."
        ),
        (
            "HTTP component configuration",
            "The HttpCall component allows you to make external API requests. Configure the URL, method, headers, and body parameters. You can use the response in subsequent steps of your flow."
        ),
        (
            "Error handling in flows",
            "Implement robust error handling in your Cascade flows using error outputs and conditional branches. Each component can have both success and error outputs to handle different scenarios."
        ),
        (
            "Data transformation techniques",
            "Transform data between steps using the DataTransform component. You can map fields, apply transformations, and restructure your data to match the requirements of subsequent steps."
        ),
    ];
    
    // Convert to DocumentationChunk objects
    docs.into_iter().map(|(title, text)| {
        cascade_kb::data::entities::DocumentationChunk {
            id: DocumentationChunkId(Uuid::new_v4()),
            text: format!("{}: {}", title, text),
            source_url: None,
            scope: Scope::General,
            embedding: Vec::new(), // Will be filled by embedding service
            chunk_seq: None,
            created_at: chrono::Utc::now(),
        }
    }).collect()
}

/// Simple KB client for testing that wraps channel senders
pub struct TestKbClient {
    pub ingestion_tx: mpsc::Sender<IngestionMessage>,
    pub query_tx: mpsc::Sender<(QueryRequest, QueryResultSender)>,
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
    
    /// Ingest documentation chunks into the KB
    pub async fn ingest_docs(
        &self,
        tenant_id: TenantId,
        docs: Vec<cascade_kb::data::entities::DocumentationChunk>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let trace_ctx = TraceContext::default();
        
        // Convert to YAML-like format for ingestion
        let yaml_content = format!(
            "documentation:\n{}",
            docs.iter()
                .map(|doc| format!("  - text: \"{}\"", doc.text.replace("\"", "\\\"")))
                .collect::<Vec<_>>()
                .join("\n")
        );
        
        // Send as YAML definitions
        self.ingestion_tx.send(IngestionMessage::YamlDefinitions {
            tenant_id,
            trace_ctx,
            yaml_content,
            scope: Scope::General,
            source_info: Some("test_docs".to_string()),
        }).await?;
        
        Ok(())
    }
    
    /// Perform a semantic search
    pub async fn semantic_search(
        &self,
        tenant_id: TenantId,
        query_text: &str,
        k: usize,
        scope_filter: Option<Scope>,
    ) -> Result<Vec<HashMap<String, DataPacket>>, Box<dyn std::error::Error>> {
        let trace_ctx = TraceContext::default();
        
        // Create the query request
        let query_request = QueryRequest::SemanticSearch {
            tenant_id,
            trace_ctx,
            query_text: query_text.to_string(),
            k,
            scope_filter,
            entity_type_filter: None,
        };
        
        // Send the query and wait for response
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.query_tx.send((query_request, QueryResultSender { sender: response_tx })).await?;
        
        // Handle the response
        match response_rx.await? {
            QueryResponse::Success(results) => Ok(results),
            QueryResponse::Error(e) => Err(Box::new(e)),
        }
    }
} 
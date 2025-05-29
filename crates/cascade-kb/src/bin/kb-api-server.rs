//! KB API Server for remote mode testing
//! 
//! This binary provides an HTTP API for interacting with the KB.
//! It is used for testing the remote KB provider mode.

use std::net::SocketAddr;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;
use axum::{
    routing::{get, post},
    Router, Json, extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use tracing::{info, error, debug};
use uuid::Uuid;

// Add dotenv dependency
use dotenv;

use cascade_kb::{
    services::{
        IngestionService, 
        QueryService,
        messages::{IngestionMessage, QueryRequest, QueryResultSender},
    },
    adapters::neo4j_store::{Neo4jStateStore, Neo4jConfig},
    embedding::{create_embedding_service, EmbeddingServiceConfig},
    data::trace_context::TraceContext,
    data::identifiers::TenantId,
    data::errors::CoreError,
    data::types::{DataPacket, Scope},
    traits::{StateStore, EmbeddingGenerator, DslParser},
};
use cascade_interfaces::kb::{
    GraphElementDetails, RetrievedContext, ValidationErrorDetail,
};
use tokio::sync::{mpsc, oneshot};

// Shared application state
#[derive(Clone)]
struct AppState {
    query_service: mpsc::Sender<(QueryRequest, QueryResultSender)>,
    ingestion_service: mpsc::Sender<IngestionMessage>,
}

// Main entry point
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("cascade_kb=debug,kb_api_server=debug")
        .init();
    
    // Load environment for Neo4j connection
    dotenv::dotenv().ok();
    
    // Get Neo4j connection details from environment variables
    let uri = std::env::var("NEO4J_URI")
        .unwrap_or_else(|_| "neo4j://localhost:7687".to_string());
    
    let username = std::env::var("NEO4J_USERNAME")
        .unwrap_or_else(|_| "neo4j".to_string());
    
    let password = std::env::var("NEO4J_PASSWORD")
        .unwrap_or_else(|_| "password".to_string());
    
    let database = std::env::var("NEO4J_DATABASE").ok();
    
    info!("Connecting to Neo4j at: {}", uri);
    
    // Create Neo4j configuration
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
    
    // Create Neo4j state store
    let store = match Neo4jStateStore::new(config).await {
        Ok(store) => Arc::new(store) as Arc<dyn StateStore>,
        Err(e) => {
            error!("Failed to connect to Neo4j: {}", e);
            return Err(Box::new(e));
        }
    };
    
    // Configure embedding service
    let embedding_config = EmbeddingServiceConfig::default();
    let embedding_config = EmbeddingServiceConfig {
        model: "text-embedding-ada-002".to_string(),
        api_key: std::env::var("OPENAI_API_KEY").ok(),
        ..embedding_config
    };
    
    // Create embedding service
    let embedding_service = create_embedding_service(embedding_config);
    
    // For QueryService and IngestionService we need to convert to EmbeddingGenerator trait
    // Since EmbeddingService implements EmbeddingGenerator, we can use as_ref() combined with Arc::new
    let embedding_generator: Arc<dyn EmbeddingGenerator> = embedding_service.clone();
    
    // Create message channels for services
    let (query_tx, query_rx) = mpsc::channel(100);
    let (ingestion_tx, ingestion_rx) = mpsc::channel(100);
    
    // Create services
    let mut query_service = QueryService::new(
        Arc::clone(&store),
        Arc::clone(&embedding_generator),
        query_rx,
    );
    
    let mut ingestion_service = IngestionService::new(
        Arc::clone(&store),
        Arc::clone(&embedding_generator),
        Arc::new(test_utils::fakes::FakeDslParser::new()),
        ingestion_rx,
    );
    
    // Shared application state
    let app_state = AppState {
        query_service: query_tx,
        ingestion_service: ingestion_tx,
    };
    
    // Spawn services as background tasks
    tokio::spawn(async move {
        if let Err(e) = query_service.run().await {
            error!("QueryService error: {}", e);
        }
    });
    
    tokio::spawn(async move {
        if let Err(e) = ingestion_service.run().await {
            error!("IngestionService error: {}", e);
        }
    });
    
    // Create API routes
    let app = Router::new()
        .route("/", get(|| async { "Cascade Knowledge Base API Server" }))
        .route("/health", get(health_check))
        .route("/api/v1/search", post(semantic_search))
        .route("/api/v1/elements/:id", get(get_element))
        .route("/api/v1/validate-dsl", post(validate_dsl))
        .route("/api/v1/trace-input/:flow_id/:step_id/:input_name", get(trace_input))
        .route("/api/v1/related-artefacts", get(find_related_artefacts))
        .with_state(app_state);
    
    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Starting server on {}", addr);
    
    match axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
        }
}

// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

// Request structure for semantic search
#[derive(Debug, Deserialize)]
struct SearchRequest {
    tenant_id: String,
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
    scope: Option<String>,
    entity_type: Option<String>,
}

fn default_limit() -> usize {
    10
}

// Query parameters for element retrieval
#[derive(Debug, Deserialize)]
struct ElementQuery {
    tenant_id: String,
    version: Option<String>,
}

// Query parameters for related artefacts
#[derive(Debug, Deserialize)]
struct RelatedArtefactsQuery {
    tenant_id: String,
    query: Option<String>,
    entity_id: Option<String>,
    version: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

// Request structure for DSL validation
#[derive(Debug, Deserialize)]
struct ValidationRequest {
    tenant_id: String,
    dsl_code: String,
}

// Query parameters for tracing input sources
#[derive(Debug, Deserialize)]
struct TraceInputQuery {
    tenant_id: String,
    version: Option<String>,
}

// Handler for semantic search
async fn semantic_search(
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<Vec<HashMap<String, DataPacket>>>, Json<CoreError>> {
    let trace_ctx = TraceContext::new_root();
    
    // Convert scope string to enum if provided
    let scope_filter = match request.scope {
        Some(scope_str) => match scope_str.as_str() {
            "Component" => Some(Scope::Component),
            "UserDefined" => Some(Scope::UserDefined),
            "General" => Some(Scope::General),
            "ApplicationState" => Some(Scope::ApplicationState),
            _ => None,
        },
        None => None,
    };
    
    // Parse tenant ID
    let tenant_id = match Uuid::parse_str(&request.tenant_id) {
        Ok(uuid) => TenantId(uuid),
        Err(_) => return Err(Json(CoreError::Internal(format!("Invalid tenant ID format: {}", request.tenant_id)))),
    };
    
    // Create query request
    let query_req = QueryRequest::SemanticSearch {
        tenant_id,
        trace_ctx,
        query_text: request.query,
        k: request.limit,
        scope_filter,
        entity_type_filter: request.entity_type,
    };
    
    // Execute query and wait for response
    let (tx, rx) = oneshot::channel();
    let result_sender = QueryResultSender { sender: tx };
    
    if let Err(_) = state.query_service.send((query_req, result_sender)).await {
        return Err(Json(CoreError::Internal("Failed to send query to service".to_string())));
    }
    
    // Wait for response
    match rx.await {
        Ok(QueryResponse::Success(results)) => Ok(Json(results)),
        Ok(QueryResponse::Error(e)) => Err(Json(e)),
        Err(_) => Err(Json(CoreError::Internal("Service response channel closed".to_string()))),
    }
}

// Handler for getting element details
async fn get_element(
    State(_state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ElementQuery>,
) -> Result<Json<Option<cascade_interfaces::kb::GraphElementDetails>>, Json<CoreError>> {
    let trace_ctx = TraceContext::new_root();
    
    // Create QueryService instance to call directly
    let (_tx, rx) = mpsc::channel(1);
    let query_service = QueryService::new(
        // These Arcs won't be used directly since we're calling the method
        Arc::new(FakeStateStore::new()) as Arc<dyn StateStore>,
        Arc::new(FakeEmbeddingService::new()) as Arc<dyn EmbeddingGenerator>,
        rx,
    );
    
    // Call method directly
    match query_service.get_element_details(
        &query.tenant_id,
        &id,
        query.version.as_deref(),
        &trace_ctx,
    ).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err(Json(e)),
    }
}

// Handler for validating DSL code
async fn validate_dsl(
    State(_state): State<AppState>,
    Json(request): Json<ValidationRequest>,
) -> Result<Json<Vec<cascade_interfaces::kb::ValidationErrorDetail>>, Json<CoreError>> {
    let trace_ctx = TraceContext::new_root();
    
    // Create QueryService instance to call directly
    let (_tx, rx) = mpsc::channel(1);
    let query_service = QueryService::new(
        // These Arcs won't be used directly since we're calling the method
        Arc::new(FakeStateStore::new()) as Arc<dyn StateStore>,
        Arc::new(FakeEmbeddingService::new()) as Arc<dyn EmbeddingGenerator>,
        rx,
    );
    
    // Call method directly
    match query_service.validate_dsl(
        &request.tenant_id,
        &request.dsl_code,
        &trace_ctx,
    ).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err(Json(e)),
    }
}

// Handler for tracing input sources
async fn trace_input(
    State(_state): State<AppState>,
    Path((flow_id, step_id, input_name)): Path<(String, String, String)>,
    Query(query): Query<TraceInputQuery>,
) -> Result<Json<Vec<(String, String)>>, Json<CoreError>> {
    let trace_ctx = TraceContext::new_root();
    
    // Create QueryService instance to call directly
    let (_tx, rx) = mpsc::channel(1);
    let query_service = QueryService::new(
        // These Arcs won't be used directly since we're calling the method
        Arc::new(FakeStateStore::new()) as Arc<dyn StateStore>,
        Arc::new(FakeEmbeddingService::new()) as Arc<dyn EmbeddingGenerator>,
        rx,
    );
    
    // Call method directly
    match query_service.trace_step_input_source(
        &query.tenant_id,
        &flow_id,
        query.version.as_deref(),
        &step_id,
        &input_name,
        &trace_ctx,
    ).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err(Json(e)),
    }
}

// Handler for finding related artefacts
async fn find_related_artefacts(
    State(_state): State<AppState>,
    Query(query): Query<RelatedArtefactsQuery>,
) -> Result<Json<Vec<cascade_interfaces::kb::RetrievedContext>>, Json<CoreError>> {
    let trace_ctx = TraceContext::new_root();
    
    // Create QueryService instance to call directly
    let (_tx, rx) = mpsc::channel(1);
    let query_service = QueryService::new(
        // These Arcs won't be used directly since we're calling the method
        Arc::new(FakeStateStore::new()) as Arc<dyn StateStore>,
        Arc::new(FakeEmbeddingService::new()) as Arc<dyn EmbeddingGenerator>,
        rx,
    );
    
    // Call method directly
    match query_service.search_related_artefacts(
        &query.tenant_id,
        query.query.as_deref(),
        query.entity_id.as_deref(),
        query.version.as_deref(),
        query.limit,
        &trace_ctx,
    ).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err(Json(e)),
    }
}

// Fake implementations for direct method calls
use cascade_kb::test_utils::fakes::{FakeStateStore, FakeEmbeddingService, FakeDslParser};
use cascade_kb::services::messages::QueryResponse; 
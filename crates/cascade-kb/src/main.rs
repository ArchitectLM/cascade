use std::sync::Arc;
use tokio::sync::mpsc;
use tracing_subscriber;

use cascade_kb::{
    data::{
        identifiers::TenantId,
        types::Scope,
        trace_context::TraceContext,
    },
    services::{
        ingestion::IngestionService,
        query::QueryService,
        client::KbClient,
        messages::{IngestionMessage, QueryRequest, QueryResultSender},
    },
    test_utils::fakes::{FakeStateStore, FakeEmbeddingService, FakeDslParser},
    traits::{StateStore, EmbeddingGenerator, DslParser},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create channels for service communication
    let (ingestion_tx, ingestion_rx) = mpsc::channel::<IngestionMessage>(100);
    let (query_tx, query_rx) = mpsc::channel::<(QueryRequest, QueryResultSender)>(100);

    // Create fake implementations of the core traits
    let state_store: Arc<dyn StateStore> = Arc::new(FakeStateStore::new());
    let embedding_generator: Arc<dyn EmbeddingGenerator> = Arc::new(FakeEmbeddingService::new());
    let dsl_parser: Arc<dyn DslParser> = Arc::new(FakeDslParser::new());

    // Create the services
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

    // Create a client for testing
    let client = KbClient::new(ingestion_tx.clone(), query_tx.clone());

    // Spawn the services
    let ingestion_handle = tokio::spawn(async move {
        if let Err(e) = ingestion_service.run().await {
            tracing::error!("Ingestion service error: {:?}", e);
        }
    });

    let query_handle = tokio::spawn(async move {
        if let Err(e) = query_service.run().await {
            tracing::error!("Query service error: {:?}", e);
        }
    });

    tracing::info!("Cascade KB core services started with fake implementations");

    // Example: Test the services with some basic operations
    let tenant_id = TenantId::new_v4();
    let trace_ctx = TraceContext::new_root();

    // Test ingestion
    let result = client.ingest_yaml(
        tenant_id.clone(),
        "test: content".to_string(),
        Scope::UserDefined,
        Some("test.yaml".to_string()),
    ).await;
    match result {
        Ok(_) => tracing::info!("Test ingestion successful"),
        Err(e) => tracing::error!("Test ingestion failed: {:?}", e),
    }

    // Test query
    let result = client.lookup_by_id(
        tenant_id,
        trace_ctx,
        "ComponentDefinition".to_string(),
        "test_id".to_string(),
        None,
    ).await;
    match result {
        Ok(_) => tracing::info!("Test query successful"),
        Err(e) => tracing::error!("Test query failed: {:?}", e),
    }

    // Wait for services to complete (they won't in this example)
    let _ = tokio::join!(ingestion_handle, query_handle);

    Ok(())
} 
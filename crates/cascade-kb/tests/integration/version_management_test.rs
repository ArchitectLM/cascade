//! Integration test for version management
//!
//! Tests the creation and querying of version sets and versions in the KB.

use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;
use cascade_kb::{
    data::{
        identifiers::TenantId,
        types::Scope,
        trace_context::TraceContext,
    },
    services::{
        ingestion::IngestionService,
        query::QueryService,
        messages::{IngestionMessage, QueryRequest, QueryResponse, QueryResultSender},
    },
    traits::{StateStore, EmbeddingGenerator, DslParser},
    test_utils::fakes::{FakeStateStore, FakeEmbeddingService, FakeDslParser},
};

#[tokio::test]
async fn test_version_management() {
    // Setup test environment
    let tenant_id = TenantId::new_v4();
    let scope = Scope::UserDefined;
    let trace_context = TraceContext::new_root();

    // Create fake services
    let (ingestion_tx, ingestion_rx) = mpsc::channel(100);
    let (query_tx, query_rx) = mpsc::channel(100);
    
    let state_store: Arc<dyn StateStore> = Arc::new(FakeStateStore::new());
    let embedding_gen: Arc<dyn EmbeddingGenerator> = Arc::new(FakeEmbeddingService::new());
    let dsl_parser: Arc<dyn DslParser> = Arc::new(FakeDslParser::new());
    
    let mut ingestion_service = IngestionService::new(
        Arc::clone(&state_store),
        Arc::clone(&embedding_gen),
        Arc::clone(&dsl_parser),
        ingestion_rx,
    );
    
    let mut query_service = QueryService::new(
        Arc::clone(&state_store),
        Arc::clone(&embedding_gen),
        query_rx,
    );

    // Start services
    let ingestion_service_handle = tokio::spawn(async move {
        ingestion_service.run().await.expect("Ingestion service failed");
    });
    
    let query_service_handle = tokio::spawn(async move {
        query_service.run().await.expect("Query service failed");
    });

    // Test version creation
    let version_set_id = Uuid::new_v4();
    let version_id = Uuid::new_v4();

    // Create a version set
    let create_version_set = IngestionMessage::CreateVersionSet {
        tenant_id: tenant_id.clone(),
        version_set_id,
        scope: scope.clone(),
        trace_context: trace_context.clone(),
    };

    ingestion_tx.send(create_version_set).await.unwrap();

    // Create a version
    let create_version = IngestionMessage::CreateVersion {
        tenant_id: tenant_id.clone(),
        version_set_id,
        version_id,
        scope: scope.clone(),
        trace_context: trace_context.clone(),
    };

    ingestion_tx.send(create_version).await.unwrap();

    // Query the version
    let query = QueryRequest::GraphTraversal {
        tenant_id: tenant_id.clone(),
        trace_ctx: trace_context.clone(),
        cypher_query: format!("MATCH (v:Version {{id: '{}'}}) RETURN v", version_id),
        params: std::collections::HashMap::new(),
    };

    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    query_tx.send((query, QueryResultSender { sender: response_tx })).await.unwrap();

    // Verify the version exists
    let response = response_rx.await.unwrap();
    assert!(matches!(response, QueryResponse::Success(_)));
    
    // Clean up resources
    drop(ingestion_tx);
    drop(query_tx);
    
    // Don't need to wait for services to terminate in this test
    // as we're just checking functionality and will let the tasks be aborted
} 
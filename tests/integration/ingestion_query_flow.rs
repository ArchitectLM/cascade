use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

use cascade_kb::{
    data::{
        TenantId,
        TraceContext,
        Scope,
        CoreError,
    },
    services::{
        ingestion::{
            IngestionService,
            IngestionServiceConfig,
        },
        query::QueryService,
        client::KbClient,
        messages::{
            IngestionMessage,
            QueryRequest,
            QueryResponse,
            QueryResultSender,
        },
    },
    test_utils::{
        helpers,
        mocks::{
            DeterministicEmbeddingGenerator,
            TestStateStore,
        },
    },
    traits::{
        StateStore,
        EmbeddingGenerator,
        DslParser,
    },
};

// Integration test for the full flow: YAML ingestion -> Query
#[tokio::test]
async fn test_full_ingestion_query_flow() {
    // Create test tenant and trace context
    let tenant_id = TenantId::new_v4();
    let trace_ctx = TraceContext::new_root();

    // Set up test dependencies with simple implementations
    let state_store = Arc::new(TestStateStore::new());
    let embedding_generator = Arc::new(DeterministicEmbeddingGenerator::new(384)); // Smaller dimension for tests
    
    // Create a simple implementation of DslParser that returns test data
    struct TestDslParser;
    #[async_trait::async_trait]
    impl DslParser for TestDslParser {
        async fn parse_and_validate_yaml(
            &self,
            _yaml_content: &str,
            tenant_id: &TenantId,
            _trace_ctx: &TraceContext,
        ) -> Result<cascade_kb::data::ParsedDslDefinitions, CoreError> {
            // Return test data
            let component = helpers::test_component_definition(tenant_id);
            let flow = helpers::test_flow_definition(tenant_id);
            let doc_chunk = helpers::test_documentation_chunk(tenant_id);
            
            Ok(cascade_kb::data::ParsedDslDefinitions {
                components: vec![component],
                flows: vec![flow],
                documentation_chunks: vec![doc_chunk],
                input_definitions: vec![],
                output_definitions: vec![],
                step_definitions: vec![],
                trigger_definitions: vec![],
                condition_definitions: vec![],
                code_examples: vec![],
            })
        }
    }
    
    let dsl_parser = Arc::new(TestDslParser);

    // Create channels for services
    let (ingestion_tx, ingestion_rx) = mpsc::channel(32);
    let (query_tx, query_rx) = mpsc::channel(32);

    // Create and start services
    let ingestion_config = IngestionServiceConfig {
        max_embedding_batch_size: 5,
        continue_on_errors: true,
        batch_timeout_ms: 100,
    };
    
    let mut ingestion_service = IngestionService::with_config(
        Arc::clone(&state_store),
        Arc::clone(&embedding_generator),
        Arc::clone(&dsl_parser),
        ingestion_rx,
        ingestion_config,
    );
    
    let mut query_service = QueryService::new(
        Arc::clone(&state_store),
        Arc::clone(&embedding_generator),
        query_rx,
    );
    
    // Start services in background tasks
    let ingestion_handle = tokio::spawn(async move {
        ingestion_service.run().await.unwrap();
    });
    
    let query_handle = tokio::spawn(async move {
        query_service.run().await.unwrap();
    });
    
    // Create client
    let client = KbClient::new(ingestion_tx, query_tx);
    
    // Test data
    let yaml_content = helpers::test_component_yaml();
    
    // 1. Ingest YAML definition
    let ingest_result = client.ingest_yaml(
        tenant_id,
        yaml_content,
        Scope::UserDefined,
        Some("test.yaml".to_string()),
    ).await;
    
    assert!(ingest_result.is_ok(), "Ingestion failed: {:?}", ingest_result);
    
    // Wait a bit for processing to complete
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // 2. Query for the ingested component
    let query_result = client.lookup_by_id(
        tenant_id,
        "ComponentDefinition".to_string(),
        "StdLib:TestComponent".to_string(),
        None,
    ).await;
    
    assert!(query_result.is_ok(), "Query failed: {:?}", query_result);
    
    // 3. Perform semantic search
    let semantic_query = QueryRequest::SemanticSearch {
        tenant_id,
        trace_ctx: TraceContext::new_root(),
        query_text: "test document".to_string(),
        k: 5,
        scope_filter: Some(Scope::UserDefined),
        entity_type_filter: None,
    };
    
    let semantic_result = client.query(semantic_query).await;
    assert!(semantic_result.is_ok(), "Semantic search failed: {:?}", semantic_result);
    
    // Clean up
    ingestion_handle.abort();
    query_handle.abort();
}

// Integration test for multi-tenant isolation
#[tokio::test]
async fn test_multi_tenant_isolation() {
    // Create two test tenants
    let tenant_a = TenantId::new_v4();
    let tenant_b = TenantId::new_v4();
    
    // Set up test dependencies
    let state_store = Arc::new(TestStateStore::new());
    let embedding_generator = Arc::new(DeterministicEmbeddingGenerator::new(384));
    
    // Create a simple implementation of DslParser that returns tenant-specific test data
    struct TestDslParser;
    #[async_trait::async_trait]
    impl DslParser for TestDslParser {
        async fn parse_and_validate_yaml(
            &self,
            yaml_content: &str,
            tenant_id: &TenantId,
            _trace_ctx: &TraceContext,
        ) -> Result<cascade_kb::data::ParsedDslDefinitions, CoreError> {
            // Create a component with tenant-specific name
            let mut component = helpers::test_component_definition(tenant_id);
            let tenant_suffix = if yaml_content.contains("TenantA") { "A" } else { "B" };
            component.name = format!("Component_Tenant{}", tenant_suffix);
            
            Ok(cascade_kb::data::ParsedDslDefinitions {
                components: vec![component],
                flows: vec![],
                documentation_chunks: vec![],
                input_definitions: vec![],
                output_definitions: vec![],
                step_definitions: vec![],
                trigger_definitions: vec![],
                condition_definitions: vec![],
                code_examples: vec![],
            })
        }
    }
    
    let dsl_parser = Arc::new(TestDslParser);

    // Create channels for services
    let (ingestion_tx, ingestion_rx) = mpsc::channel(32);
    let (query_tx, query_rx) = mpsc::channel(32);

    // Create and start services
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
    
    // Start services in background tasks
    let ingestion_handle = tokio::spawn(async move {
        ingestion_service.run().await.unwrap();
    });
    
    let query_handle = tokio::spawn(async move {
        query_service.run().await.unwrap();
    });
    
    // Create client
    let client = KbClient::new(ingestion_tx, query_tx);
    
    // 1. Ingest data for tenant A
    let yaml_a = "component: TenantA";
    let ingest_a = client.ingest_yaml(
        tenant_a,
        yaml_a.to_string(),
        Scope::UserDefined,
        Some("tenant_a.yaml".to_string()),
    ).await;
    assert!(ingest_a.is_ok(), "Ingestion for tenant A failed: {:?}", ingest_a);
    
    // 2. Ingest data for tenant B
    let yaml_b = "component: TenantB";
    let ingest_b = client.ingest_yaml(
        tenant_b,
        yaml_b.to_string(),
        Scope::UserDefined,
        Some("tenant_b.yaml".to_string()),
    ).await;
    assert!(ingest_b.is_ok(), "Ingestion for tenant B failed: {:?}", ingest_b);
    
    // Wait for processing
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // 3. Query for tenant A's data should only return tenant A's data
    let query_a = QueryRequest::GraphTraversal {
        tenant_id: tenant_a,
        trace_ctx: TraceContext::new_root(),
        cypher_query: "MATCH (c:ComponentDefinition) RETURN c".to_string(),
        params: std::collections::HashMap::new(),
    };
    
    let result_a = client.query(query_a).await.unwrap();
    match result_a {
        QueryResponse::Success(data) => {
            // Validate that we only got tenant A's data
            assert!(data.iter().any(|node| {
                node.values().any(|value| {
                    match value {
                        cascade_kb::data::DataPacket::String(s) => s.contains("TenantA"),
                        _ => false,
                    }
                })
            }));
            
            // Validate that we don't have tenant B's data
            assert!(!data.iter().any(|node| {
                node.values().any(|value| {
                    match value {
                        cascade_kb::data::DataPacket::String(s) => s.contains("TenantB"),
                        _ => false,
                    }
                })
            }));
        },
        QueryResponse::Error(e) => {
            panic!("Query for tenant A failed: {:?}", e);
        }
    }
    
    // Clean up
    ingestion_handle.abort();
    query_handle.abort();
}

// Test for error recovery during ingestion
#[tokio::test]
async fn test_error_recovery_during_ingestion() {
    // Create test tenant
    let tenant_id = TenantId::new_v4();
    
    // Set up test dependencies
    let state_store = Arc::new(TestStateStore::new());
    let embedding_generator = Arc::new(DeterministicEmbeddingGenerator::new(384));
    
    // Create a DslParser that sometimes fails
    struct FlakeyDslParser {
        fail_count: std::sync::atomic::AtomicUsize,
    }
    
    #[async_trait::async_trait]
    impl DslParser for FlakeyDslParser {
        async fn parse_and_validate_yaml(
            &self,
            yaml_content: &str,
            tenant_id: &TenantId,
            _trace_ctx: &TraceContext,
        ) -> Result<cascade_kb::data::ParsedDslDefinitions, CoreError> {
            let count = self.fail_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            
            // Fail every other request
            if count % 2 == 0 {
                return Err(CoreError::ingestion_error(
                    "Simulated parser failure".to_string(),
                    "test".to_string(),
                ));
            }
            
            // Return test data
            let component = helpers::test_component_definition(tenant_id);
            
            Ok(cascade_kb::data::ParsedDslDefinitions {
                components: vec![component],
                flows: vec![],
                documentation_chunks: vec![],
                input_definitions: vec![],
                output_definitions: vec![],
                step_definitions: vec![],
                trigger_definitions: vec![],
                condition_definitions: vec![],
                code_examples: vec![],
            })
        }
    }
    
    let dsl_parser = Arc::new(FlakeyDslParser {
        fail_count: std::sync::atomic::AtomicUsize::new(0),
    });

    // Create channels for services
    let (ingestion_tx, ingestion_rx) = mpsc::channel(32);
    let (query_tx, query_rx) = mpsc::channel(32);

    // Create and start services
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
    
    // Start services in background tasks
    let ingestion_handle = tokio::spawn(async move {
        ingestion_service.run().await.unwrap();
    });
    
    let query_handle = tokio::spawn(async move {
        query_service.run().await.unwrap();
    });
    
    // Create client
    let client = KbClient::new(ingestion_tx, query_tx);
    
    // 1. First ingestion should fail
    let yaml1 = "component: Test1";
    let ingest1 = client.ingest_yaml(
        tenant_id,
        yaml1.to_string(),
        Scope::UserDefined,
        Some("test1.yaml".to_string()),
    ).await;
    
    assert!(ingest1.is_ok(), "Ingestion channel send failed");
    
    // 2. Second ingestion should succeed
    let yaml2 = "component: Test2";
    let ingest2 = client.ingest_yaml(
        tenant_id,
        yaml2.to_string(),
        Scope::UserDefined,
        Some("test2.yaml".to_string()),
    ).await;
    
    assert!(ingest2.is_ok(), "Ingestion channel send failed");
    
    // Wait for processing
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // 3. Query to verify the second ingestion worked
    let query = QueryRequest::GraphTraversal {
        tenant_id,
        trace_ctx: TraceContext::new_root(),
        cypher_query: "MATCH (c:ComponentDefinition) RETURN c".to_string(),
        params: std::collections::HashMap::new(),
    };
    
    let result = client.query(query).await;
    assert!(result.is_ok(), "Query failed: {:?}", result);
    
    // Clean up
    ingestion_handle.abort();
    query_handle.abort();
} 
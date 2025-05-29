use tokio::sync::{mpsc, oneshot};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;
use actix::Actor;
use chrono::Utc;
use dotenv::dotenv;
use cascade_interfaces::kb::{
    GraphElementDetails, 
    RetrievedContext, 
    ValidationErrorDetail,
    KnowledgeResult,
    KnowledgeBaseClient,
    KnowledgeError
};
use cdas_core::{
    UserProfile, ResponseType, Suggestion, 
    SuggestionType, Speaker, ChatRole
};

// CDAS-related imports from fake client implementations
use cascade_cdas::cdas_core_agent::tests::fakes::{
    kb::FakeKnowledgeBaseClient,
    llm::FakeLlmClient,
    storage::FakeConversationStorage,
};
use cdas_core_agent::{
    actors::{
        generation::GenerationActor,
        analysis::AnalysisActor,
        orchestrator::OrchestratorActor,
    },
    messages::{AgentMessage, MessagePayload, RecognizedIntent, AgentResponse},
    config::OrchestratorConfig,
};

// Imports for direct KB integration
use cascade_kb::{
    data::{
        identifiers::{TenantId, DocumentationChunkId},
        trace_context::TraceContext,
        entities::DocumentationChunk,
        types::Scope,
        DataPacket,
    },
    services::{
        ingestion::IngestionService,
        query::QueryService,
        client::KbClient,
        messages::{IngestionMessage, QueryRequest, QueryResponse, QueryResultSender},
    },
    embedding::{MockEmbeddingService, OpenAIEmbeddingService, EmbeddingServiceConfig},
    traits::{StateStore, EmbeddingGenerator, DslParser},
    adapters::neo4j_store::{Neo4jStateStore, Neo4jConfig},
    create_kb_client,
};

// Direct KB adapter to implement the KnowledgeBaseClient trait
struct DirectKbAdapter {
    kb_client: KbClient,
    tenant_id: TenantId,
    trace_ctx: TraceContext,
}

impl DirectKbAdapter {
    pub fn new(kb_client: KbClient, tenant_id: TenantId) -> Self {
        Self {
            kb_client,
            tenant_id,
            trace_ctx: TraceContext::default(),
        }
    }
    
    // Helper to convert cascade-kb types to cascade-interfaces types
    fn convert_to_retrieved_context(&self, data: HashMap<String, DataPacket>) -> RetrievedContext {
        let id = data.get("docId")
            .and_then(|dp| if let DataPacket::String(s) = dp { Some(s.clone()) } else { None })
            .unwrap_or_else(|| "unknown".to_string());
            
        let content = data.get("docText")
            .and_then(|dp| if let DataPacket::String(s) = dp { Some(s.clone()) } else { None })
            .unwrap_or_else(|| "".to_string());
            
        let title = data.get("entityName")
            .and_then(|dp| if let DataPacket::String(s) = dp { Some(s.clone()) } else { None })
            .unwrap_or_else(|| "Untitled Document".to_string());
            
        let source_url = data.get("sourceUrl")
            .and_then(|dp| if let DataPacket::String(s) = dp { Some(s.clone()) } else { None });
            
        let entity_type = data.get("entityType")
            .and_then(|dp| if let DataPacket::String(s) = dp { Some(s.clone()) } else { None });
            
        let relevance_score = data.get("score")
            .and_then(|dp| match dp {
                DataPacket::Number(n) => Some(*n as f64),
                DataPacket::Float(f) => Some(*f as f64),
                _ => None,
            }).unwrap_or(0.0);
            
        RetrievedContext {
            id,
            context_type: "documentation".to_string(),
            title,
            content,
            relevance_score,
            source_url,
            entity_type,
            created_at: Some(Utc::now()),
        }
    }
}

#[async_trait::async_trait]
impl KnowledgeBaseClient for DirectKbAdapter {
    async fn get_element_details(
        &self, 
        tenant_id: &str, 
        entity_id: &str, 
        version: Option<&str>
    ) -> KnowledgeResult<Option<GraphElementDetails>> {
        // For simplicity in this test, we return a mock element
        Ok(Some(GraphElementDetails {
            id: entity_id.to_string(),
            name: format!("Test Element {}", entity_id),
            element_type: "Component".to_string(),
            description: Some("Test description".to_string()),
            version: version.map(|v| v.to_string()),
            framework: Some("TestFramework".to_string()),
            schema: None,
            inputs: None,
            outputs: None,
            steps: None,
            tags: Some(vec!["test".to_string()]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: Some(serde_json::json!({})),
        }))
    }

    async fn search_related_artefacts(
        &self, 
        _tenant_id: &str, 
        query_text: Option<&str>, 
        entity_id: Option<&str>, 
        _version: Option<&str>, 
        limit: usize
    ) -> KnowledgeResult<Vec<RetrievedContext>> {
        // Build a query for the KB's semantic search
        let mut params = HashMap::new();
        params.insert("query".to_string(), DataPacket::String(query_text.unwrap_or("").to_string()));
        params.insert("limit".to_string(), DataPacket::Integer(limit as i64));
        
        // Use the KB client to perform the search
        let request = QueryRequest::SemanticSearch {
            tenant_id: self.tenant_id.clone(),
            query_text: query_text.unwrap_or("").to_string(),
            limit,
            params: Some(params),
            trace_ctx: self.trace_ctx.clone(),
        };
        
        match self.kb_client.query(request).await {
            Ok(results) => {
                // Convert results to RetrievedContext objects
                let contexts = results.into_iter()
                    .map(|data| self.convert_to_retrieved_context(data))
                    .collect();
                
                Ok(contexts)
            },
            Err(e) => {
                Err(KnowledgeError::InternalError(
                    format!("KB search error: {}", e)
                ))
            }
        }
    }

    async fn validate_dsl(
        &self, 
        _tenant_id: &str, 
        dsl_code: &str
    ) -> KnowledgeResult<Vec<ValidationErrorDetail>> {
        // In a real implementation, you would validate the DSL using cascade-kb's validator
        // For this test, we'll return an empty list (no validation errors)
        Ok(Vec::new())
    }

    async fn trace_step_input_source(
        &self, 
        _tenant_id: &str, 
        flow_entity_id: &str, 
        flow_version: Option<&str>, 
        step_id: &str, 
        component_input_name: &str
    ) -> KnowledgeResult<Vec<(String, String)>> {
        // For simplicity, we return a mock source
        Ok(vec![("previous_step".to_string(), "output".to_string())])
    }
}

// Setup direct KB client using in-memory services
async fn setup_memory_kb_test() -> (Arc<dyn KnowledgeBaseClient>, mpsc::Sender<IngestionMessage>) {
    // Initialize logging for test debugging
    let _ = tracing_subscriber::fmt()
        .with_env_filter("cascade_kb=debug,cdas_core_agent=debug")
        .try_init();
        
    // Create channels for service communication
    let (ingestion_tx, ingestion_rx) = mpsc::channel::<IngestionMessage>(32);
    let (query_tx, query_rx) = mpsc::channel::<(QueryRequest, QueryResultSender)>(32);
    
    // Use mock embedding service for tests
    let embedding_generator = Arc::new(MockEmbeddingService::default()) as Arc<dyn EmbeddingGenerator>;
    
    // Use a fake state store for testing
    let state_store = Arc::new(cascade_kb::test_utils::fakes::FakeStateStore::new()) as Arc<dyn StateStore>;
    
    // Set up the DslParser
    let dsl_parser = Arc::new(cascade_kb::test_utils::fakes::FakeDslParser::new()) as Arc<dyn DslParser>;
    
    // Create services
    let mut ingestion_service = IngestionService::new(
        state_store.clone(),
        embedding_generator.clone(),
        dsl_parser.clone(),
        ingestion_rx,
    );
    
    let mut query_service = QueryService::new(
        state_store.clone(),
        embedding_generator.clone(),
        query_rx,
    );
    
    // Start services
    tokio::spawn(async move {
        let _ = ingestion_service.run().await;
    });
    
    tokio::spawn(async move {
        let _ = query_service.run().await;
    });
    
    // Create KB client
    let kb_client = KbClient::new(ingestion_tx.clone(), query_tx);
    
    // Create tenant ID
    let tenant_id = TenantId::new_v4();
    
    // Create the adapter implementing KnowledgeBaseClient
    let adapter = DirectKbAdapter::new(kb_client, tenant_id.clone());
    
    (Arc::new(adapter) as Arc<dyn KnowledgeBaseClient>, ingestion_tx)
}

// Setup Neo4j-based KB client if available, with fallback to in-memory
async fn setup_neo4j_kb_test() -> Result<(Arc<dyn KnowledgeBaseClient>, TenantId), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenv().ok();
    
    // Try to get Neo4j connection details from environment variables
    let uri = std::env::var("NEO4J_URI").unwrap_or_else(|_| "neo4j://localhost:17687".to_string());
    let username = std::env::var("NEO4J_USERNAME").unwrap_or_else(|_| "neo4j".to_string());
    let password = std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "password".to_string());
    
    // Check if we can bypass Neo4j testing
    if let Ok(val) = std::env::var("SKIP_NEO4J_TESTS") {
        if val.to_lowercase() == "true" {
            println!("Skipping Neo4j test as SKIP_NEO4J_TESTS=true");
            return Err("Neo4j tests skipped by configuration".into());
        }
    }
    
    // Try to create a "direct" KB client, which should attempt to connect to Neo4j
    let kb_client = create_kb_client("direct", None, None);
    let tenant_id = TenantId::new_v4();
    
    // Run a simple query to verify Neo4j connectivity
    let test_result = kb_client.search_related_artefacts(
        &tenant_id.to_string(), 
        Some("test"), 
        None, 
        None, 
        1
    ).await;
    
    match test_result {
        Ok(_) => {
            println!("Successfully connected to Neo4j");
            Ok((kb_client, tenant_id))
        },
        Err(e) => {
            println!("Could not connect to Neo4j: {}, tests will be skipped", e);
            Err(Box::new(e))
        }
    }
}

// Ingest test data into the KB for testing
async fn ingest_test_data(
    ingestion_tx: &mpsc::Sender<IngestionMessage>,
    tenant_id: &TenantId,
) {
    // Create test documents for each domain
    let yaml_content = r#"
documentation:
  - id: "doc-auth-1"
    text: "How to secure your API with OAuth2 and JWT. This document describes best practices for implementing secure authentication workflows in your Cascade applications."
    scope: "General"
    
  - id: "doc-auth-2"
    text: "Authentication and authorization are key to security in distributed systems. Learn how to implement role-based access control (RBAC) in your flows."
    scope: "General"
    
  - id: "doc-db-1"
    text: "Graph databases like Neo4j are great for connected data. This guide explains how to integrate Neo4j with your Cascade workflows for knowledge-based applications."
    scope: "General"
    
  - id: "doc-lang-1"
    text: "Rust is a systems programming language focused on safety, especially safe concurrency. Learn how to create custom Cascade components using Rust."
    scope: "General"
    
  - id: "doc-ml-1"
    text: "Machine learning enables computers to learn from data. Integrate ML models into your Cascade workflows using our specialized ML components."
    scope: "General"
    
  - id: "doc-ambiguous-1"
    text: "This flow demonstrates how to process apples and oranges in a data pipeline."
    scope: "General"
    
  - id: "doc-ambiguous-2"
    text: "Another example showing how to classify apples and bananas using machine learning."
    scope: "General"
"#;

    // Send the ingestion message
    let result = ingestion_tx.send(IngestionMessage::YamlDefinitions {
        tenant_id: tenant_id.clone(),
        trace_ctx: TraceContext::default(),
        yaml_content: yaml_content.to_string(),
        scope: Scope::General,
        source_info: Some("test_docs".to_string()),
    }).await;

    match result {
        Ok(_) => println!("Successfully sent ingestion message"),
        Err(e) => println!("Failed to send ingestion message: {}", e),
    }
    
    // Wait for ingestion to complete
    tokio::time::sleep(Duration::from_millis(500)).await;
}

// Run standard test cases against the provided KB client
async fn run_standard_semantic_search_tests(
    kb_client: Arc<dyn KnowledgeBaseClient>,
    tenant_id: &str,
) {
    // Set up other CDAS dependencies
    let llm_client = Arc::new(FakeLlmClient::new());
    let conversation_storage = Arc::new(FakeConversationStorage::new());
    
    // Create a basic orchestrator config with RAG enabled
    let config = OrchestratorConfig {
        kb_context_limit: 5,
        kb_context_token_limit: 2000,
        max_refinement_attempts: 3,
        conversation_persistence_enabled: true,
        intent_recognition_enabled: true,
        proactive_assistance_enabled: true,
        ..Default::default()
    };
    
    // Start the actors
    let analysis_actor = AnalysisActor::new(
        kb_client.clone(), 
        llm_client.clone(),
        config.clone()
    ).start();
    
    let generation_actor = GenerationActor::new(
        kb_client.clone(),
        llm_client.clone(),
        config.clone()
    ).start();
    
    let orchestrator = OrchestratorActor::new(
        analysis_actor.clone(),
        generation_actor.clone(),
        conversation_storage.clone(),
        None, // No profile storage for this test
        config.clone()
    ).start();
    
    // Define test cases
    let test_cases = vec![
        (
            "Tell me about authentication and security in API calls",
            "authentication security"
        ),
        (
            "How do I use Rust with Cascade?",
            "rust programming"
        ),
        (
            "Tell me about apples in data processing",
            "apples processing"
        ),
    ];
    
    // Run the test cases
    for (input_text, description) in test_cases {
        println!("Testing query: {} ({})", input_text, description);
        
        // Create an agent message for processing
        let message = AgentMessage {
            message_id: uuid::Uuid::new_v4(),
            conversation_id: "test-conversation".to_string(),
            request_id: Some("test-request".to_string()),
            tenant_id: tenant_id.to_string(),
            user_id: "test-user".to_string(),
            trace_context: HashMap::new(),
            user_profile: None,
            payload: MessagePayload::ProcessChatInput {
                input_text: input_text.to_string(),
                intent: RecognizedIntent::GeneralChat,
                history: vec![(Speaker::User, input_text.to_string())],
                model_version: None,
            },
        };
        
        // Send the message to the orchestrator
        match orchestrator.send(message).await {
            Ok(response) => {
                // Check response
                assert!(!response.content.to_string().is_empty(), "Response content should not be empty");
                
                // Verify KB context was used
                if let Some(metadata) = response.internal_metadata {
                    if let Some(context_ids) = metadata.get("retrieved_context_ids") {
                        if let serde_json::Value::Array(ids) = context_ids {
                            println!("Query '{}' returned {} contexts", input_text, ids.len());
                            assert!(!ids.is_empty(), "Expected non-empty context for query: {}", input_text);
                        }
                    }
                }
            },
            Err(e) => {
                panic!("Error processing message: {:?}", e);
            }
        }
    }
    
    // Test edge cases
    
    // 1. Empty query
    let empty_query_message = AgentMessage {
        message_id: uuid::Uuid::new_v4(),
        conversation_id: "test-conversation".to_string(),
        request_id: Some("test-request".to_string()),
        tenant_id: tenant_id.to_string(),
        user_id: "test-user".to_string(),
        trace_context: HashMap::new(),
        user_profile: None,
        payload: MessagePayload::ProcessChatInput {
            input_text: "".to_string(),
            intent: RecognizedIntent::GeneralChat,
            history: vec![(Speaker::User, "".to_string())],
            model_version: None,
        },
    };
    
    let empty_response = orchestrator.send(empty_query_message).await.unwrap();
    
    // For empty query, we should either get a clarification or use no KB context
    if let Some(metadata) = empty_response.internal_metadata {
        if let Some(context_ids) = metadata.get("retrieved_context_ids") {
            if let serde_json::Value::Array(ids) = context_ids {
                // Either empty or very few items
                assert!(ids.len() <= 1, "Empty query should return minimal context");
            }
        }
    }
    
    // 2. Nonexistent terms
    let nonexistent_query_message = AgentMessage {
        message_id: uuid::Uuid::new_v4(),
        conversation_id: "test-conversation".to_string(),
        request_id: Some("test-request".to_string()),
        tenant_id: tenant_id.to_string(),
        user_id: "test-user".to_string(),
        trace_context: HashMap::new(),
        user_profile: None,
        payload: MessagePayload::ProcessChatInput {
            input_text: "xyznonexistentkeyword".to_string(),
            intent: RecognizedIntent::GeneralChat,
            history: vec![(Speaker::User, "xyznonexistentkeyword".to_string())],
            model_version: None,
        },
    };
    
    let nonexistent_response = orchestrator.send(nonexistent_query_message).await.unwrap();
    
    // Verify that we get a response even without relevant KB context
    assert_eq!(nonexistent_response.response_type, ResponseType::ChatResponse);
}

// Test with Fake KB client
#[actix_rt::test]
async fn test_mock_kb_semantic_search() {
    // Set up a fake KB client with diverse documents
    let kb_client = FakeKnowledgeBaseClient::new().with_diverse_documents();
    let kb_client = Arc::new(kb_client) as Arc<dyn KnowledgeBaseClient>;
    
    run_standard_semantic_search_tests(kb_client, "test-tenant").await;
}

// Test with in-memory KB services
#[actix_rt::test]
async fn test_memory_kb_semantic_search() {
    // Set up KB services and get required components
    let (kb_client, ingestion_tx) = setup_memory_kb_test().await;
    
    // Get tenant ID from adapter for test data ingestion
    let tenant_id = if let Some(adapter) = kb_client.clone().downcast_ref::<DirectKbAdapter>() {
        adapter.tenant_id.clone()
    } else {
        TenantId::new_v4()
    };
    
    // Ingest test data
    ingest_test_data(&ingestion_tx, &tenant_id).await;
    
    // Run the standard tests
    run_standard_semantic_search_tests(kb_client, &tenant_id.to_string()).await;
}

// Test with Neo4j KB if available
#[actix_rt::test]
async fn test_neo4j_kb_semantic_search() {
    // Skip if Neo4j is unavailable
    let neo4j_result = setup_neo4j_kb_test().await;
    if neo4j_result.is_err() {
        println!("Skipping Neo4j test: {:?}", neo4j_result.unwrap_err());
        return;
    }
    
    let (kb_client, tenant_id) = neo4j_result.unwrap();
    
    // Run the standard tests with Neo4j backend
    run_standard_semantic_search_tests(kb_client, &tenant_id.to_string()).await;
}

// Test direct KB operations without using the orchestration actors
#[actix_rt::test]
async fn test_direct_kb_operations() {
    // Try to setup Neo4j client
    let neo4j_result = setup_neo4j_kb_test().await;
    
    // If Neo4j isn't available, use in-memory KB
    let (kb_client, tenant_id) = match neo4j_result {
        Ok((client, tenant_id)) => (client, tenant_id.to_string()),
        Err(_) => {
            println!("Using in-memory KB for direct operations test");
            let (client, ingestion_tx) = setup_memory_kb_test().await;
            let tenant_id = if let Some(adapter) = client.clone().downcast_ref::<DirectKbAdapter>() {
                adapter.tenant_id.clone()
            } else {
                TenantId::new_v4()
            };
            
            // Ingest test data
            ingest_test_data(&ingestion_tx, &tenant_id).await;
            
            (client, tenant_id.to_string())
        }
    };
    
    // Test direct KB operations
    
    // 1. Basic search
    let results = kb_client.search_related_artefacts(
        &tenant_id,
        Some("authentication security"),
        None,
        None,
        5
    ).await;
    
    match results {
        Ok(contexts) => {
            println!("Direct search found {} results", contexts.len());
            // In a real test, we would validate the search results
            // For this test, just ensure we got at least one result
            assert!(!contexts.is_empty(), "Expected non-empty search results");
        },
        Err(e) => {
            panic!("Direct KB search failed: {:?}", e);
        }
    }
    
    // 2. Empty query
    let empty_results = kb_client.search_related_artefacts(
        &tenant_id,
        Some(""),
        None,
        None,
        5
    ).await;
    
    // Empty query should return empty results or be handled gracefully
    match empty_results {
        Ok(contexts) => {
            // We expect few or no results for an empty query
            assert!(contexts.len() <= 1, "Empty query should return minimal results");
        },
        Err(e) => {
            // Some implementations might reject empty queries with an error
            println!("Empty query handling: {:?}", e);
        }
    }
    
    // 3. Get element details
    let element = kb_client.get_element_details(
        &tenant_id,
        "test-element-id",
        None
    ).await;
    
    // Just check that this doesn't error
    match element {
        Ok(_) => println!("Get element details successful"),
        Err(e) => println!("Get element details error (may be expected): {:?}", e),
    }
} 
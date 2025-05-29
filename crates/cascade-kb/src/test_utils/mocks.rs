//! Mock implementations of core interfaces for unit testing
//! 
//! This module contains mock implementations that can be used in standard unit tests
//! without requiring external dependencies like Neo4j.

#[cfg(feature = "mocks")]
pub mod mock_implementations {
    use serde_json::Value;
    use std::collections::HashMap;
    use async_trait::async_trait;
    use uuid::Uuid;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use std::any::Any;
    
    use crate::data::{
        identifiers::{TenantId, VersionId, VersionSetId},
        trace_context::TraceContext,
        types::DataPacket, 
        errors::{StateStoreError, CoreError}
    };
    use crate::traits::{StateStore, EmbeddingGenerator, DslParser, ParsedDslDefinitions, GraphDataPatch};
    
    // Type alias for embedding vector
    pub type EmbeddingVector = Vec<f32>;
    
    // Define simplified types for the mocks
    #[derive(Debug, Clone)]
    pub struct Node {
        pub id: String,
        pub labels: Vec<String>,
        pub properties: HashMap<String, DataPacket>,
    }
    
    #[derive(Debug, Clone)]
    pub struct Edge {
        pub from_id: String,
        pub to_id: String,
        pub type_name: String,
        pub properties: HashMap<String, DataPacket>,
    }
    
    // Define a simplified SearchResult type
    #[derive(Debug, Clone)]
    pub struct SearchResult {
        pub id: String,
        pub entity_type: String,
        pub score: f32,
        pub data: serde_json::Value,
    }
    
    // Mock for StateStore
    #[derive(Debug, Clone)]
    pub struct MockStateStore {
        // We'll use a Vec to store expected results for queries
        expected_query_results: Arc<Mutex<Vec<Result<Vec<HashMap<String, DataPacket>>, StateStoreError>>>>,
        expected_upsert_results: Arc<Mutex<Vec<Result<(), StateStoreError>>>>,
        expected_vector_search_results: Arc<Mutex<Vec<Result<Vec<(Uuid, f32)>, StateStoreError>>>>,
        call_counts: Arc<Mutex<HashMap<String, usize>>>,
    }
    
    impl MockStateStore {
        pub fn new() -> Self {
            Self {
                expected_query_results: Arc::new(Mutex::new(Vec::new())),
                expected_upsert_results: Arc::new(Mutex::new(Vec::new())),
                expected_vector_search_results: Arc::new(Mutex::new(Vec::new())),
                call_counts: Arc::new(Mutex::new(HashMap::new())),
            }
        }
        
        pub fn expect_execute_query(&self) -> &Self {
            // Increment expected call count
            let mut call_counts = self.call_counts.lock().unwrap();
            let count = call_counts.entry("execute_query".to_string()).or_insert(0);
            *count += 1;
            self
        }
        
        pub fn expect_upsert_graph_data(&self) -> &Self {
            let mut call_counts = self.call_counts.lock().unwrap();
            let count = call_counts.entry("upsert_graph_data".to_string()).or_insert(0);
            *count += 1;
            self
        }
        
        pub fn expect_vector_search(&self) -> &Self {
            let mut call_counts = self.call_counts.lock().unwrap();
            let count = call_counts.entry("vector_search".to_string()).or_insert(0);
            *count += 1;
            self
        }
        
        pub fn returning(&self, result: impl Into<Result<Vec<HashMap<String, DataPacket>>, StateStoreError>>) -> &Self {
            let mut results = self.expected_query_results.lock().unwrap();
            results.push(result.into());
            self
        }
        
        pub fn returning_for_upsert(&self, result: impl Into<Result<(), StateStoreError>>) -> &Self {
            let mut results = self.expected_upsert_results.lock().unwrap();
            results.push(result.into());
            self
        }
        
        pub fn returning_for_vector_search(&self, result: impl Into<Result<Vec<(Uuid, f32)>, StateStoreError>>) -> &Self {
            let mut results = self.expected_vector_search_results.lock().unwrap();
            results.push(result.into());
            self
        }
        
        pub fn times(&self, _: usize) -> &Self {
            // This is just a stub for mockall-style syntax
            self
        }
        
        pub fn with<P>(&self, _: P) -> &Self {
            // This is just a stub for mockall-style syntax
            self
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }
    
    #[async_trait]
    impl StateStore for MockStateStore {
        async fn execute_query(
            &self,
            _tenant_id: &TenantId,
            _trace_ctx: &TraceContext,
            _query: &str,
            _params: Option<HashMap<String, DataPacket>>,
        ) -> Result<Vec<HashMap<String, DataPacket>>, StateStoreError> {
            // Increment call count
            let mut call_counts = self.call_counts.lock().unwrap();
            let count = call_counts.entry("execute_query".to_string()).or_insert(0);
            *count = count.saturating_sub(1);

            // Get the next result
            let mut results = self.expected_query_results.lock().unwrap();
            if !results.is_empty() {
                results.remove(0)
            } else {
                // Default empty result
                Ok(vec![])
            }
        }
        
        async fn upsert_graph_data(
            &self,
            _tenant_id: &TenantId,
            _trace_ctx: &TraceContext,
            _data: GraphDataPatch,
        ) -> Result<(), StateStoreError> {
            // Increment call count
            let mut call_counts = self.call_counts.lock().unwrap();
            let count = call_counts.entry("upsert_graph_data".to_string()).or_insert(0);
            *count = count.saturating_sub(1);

            // Get the next result
            let mut results = self.expected_upsert_results.lock().unwrap();
            if !results.is_empty() {
                results.remove(0)
            } else {
                // Default success result
                Ok(())
            }
        }
        
        async fn vector_search(
            &self,
            _tenant_id: &TenantId,
            _trace_ctx: &TraceContext,
            _index_name: &str,
            _embedding: &[f32],
            _k: usize,
            _filter_params: Option<HashMap<String, DataPacket>>,
        ) -> Result<Vec<(Uuid, f32)>, StateStoreError> {
            // Increment call count
            let mut call_counts = self.call_counts.lock().unwrap();
            let count = call_counts.entry("vector_search".to_string()).or_insert(0);
            *count = count.saturating_sub(1);

            // Get the next result
            let mut results = self.expected_vector_search_results.lock().unwrap();
            if !results.is_empty() {
                results.remove(0)
            } else {
                // Default empty result
                Ok(vec![])
            }
        }
        
        fn as_any(&self) -> &dyn Any {
            self
        }
    }
    
    // Mock for EmbeddingGenerator
    #[derive(Debug, Clone)]
    pub struct MockEmbeddingGenerator {
        expected_results: Arc<Mutex<Vec<Result<Vec<f32>, CoreError>>>>,
        call_counts: Arc<Mutex<usize>>,
    }
    
    impl MockEmbeddingGenerator {
        pub fn new() -> Self {
            Self {
                expected_results: Arc::new(Mutex::new(Vec::new())),
                call_counts: Arc::new(Mutex::new(0)),
            }
        }
        
        pub fn expect_generate_embedding(&self) -> &Self {
            let mut call_counts = self.call_counts.lock().unwrap();
            *call_counts += 1;
            self
        }
        
        pub fn returning(&self, result: impl Into<Result<Vec<f32>, CoreError>>) -> &Self {
            let mut results = self.expected_results.lock().unwrap();
            results.push(result.into());
            self
        }
        
        pub fn times(&self, _: usize) -> &Self {
            // This is just a stub for mockall-style syntax
            self
        }
        
        pub fn with<P>(&self, _: P) -> &Self {
            // This is just a stub for mockall-style syntax
            self
        }
    }
    
    #[async_trait]
    impl EmbeddingGenerator for MockEmbeddingGenerator {
        async fn generate_embedding(&self, _text: &str) -> Result<Vec<f32>, CoreError> {
            // Decrement call count
            let mut call_counts = self.call_counts.lock().unwrap();
            *call_counts = call_counts.saturating_sub(1);

            // Get the next result
            let mut results = self.expected_results.lock().unwrap();
            if !results.is_empty() {
                results.remove(0)
            } else {
                // Default result with simple embedding vector
                Ok(vec![0.1, 0.2, 0.3])
            }
        }
    }
    
    // Mock for DslParser
    #[derive(Debug, Clone)]
    pub struct MockDslParser {
        expected_results: Arc<Mutex<Vec<Result<ParsedDslDefinitions, CoreError>>>>,
        call_counts: Arc<Mutex<usize>>,
    }
    
    impl MockDslParser {
        pub fn new() -> Self {
            Self {
                expected_results: Arc::new(Mutex::new(Vec::new())),
                call_counts: Arc::new(Mutex::new(0)),
            }
        }
        
        pub fn expect_parse_and_validate_yaml(&self) -> &Self {
            let mut call_counts = self.call_counts.lock().unwrap();
            *call_counts += 1;
            self
        }
        
        pub fn returning(&self, result: impl Into<Result<ParsedDslDefinitions, CoreError>>) -> &Self {
            let mut results = self.expected_results.lock().unwrap();
            results.push(result.into());
            self
        }
        
        pub fn times(&self, _: usize) -> &Self {
            // This is just a stub for mockall-style syntax
            self
        }
        
        pub fn with<P>(&self, _: P) -> &Self {
            // This is just a stub for mockall-style syntax
            self
        }
    }
    
    #[async_trait]
    impl DslParser for MockDslParser {
        async fn parse_and_validate_yaml(
            &self,
            _yaml_content: &str,
            _tenant_id: &TenantId,
            _trace_ctx: &TraceContext,
        ) -> Result<ParsedDslDefinitions, CoreError> {
            // Decrement call count
            let mut call_counts = self.call_counts.lock().unwrap();
            *call_counts = call_counts.saturating_sub(1);

            // Get the next result
            let mut results = self.expected_results.lock().unwrap();
            if !results.is_empty() {
                results.remove(0)
            } else {
                // Return an empty default result
                Ok(ParsedDslDefinitions {
                    components: Vec::new(),
                    flows: Vec::new(),
                    input_definitions: Vec::new(),
                    output_definitions: Vec::new(),
                    step_definitions: Vec::new(),
                    trigger_definitions: Vec::new(),
                    condition_definitions: Vec::new(),
                    documentation_chunks: Vec::new(),
                    code_examples: Vec::new(),
                })
            }
        }
    }
    
    // MockKnowledgeBaseClient used for external API testing
    #[derive(Debug, Clone)]
    pub struct MockKnowledgeBaseClient {
        expected_results: Arc<Mutex<Vec<Result<serde_json::Value, anyhow::Error>>>>,
        call_counts: Arc<Mutex<HashMap<String, usize>>>,
    }
    
    impl MockKnowledgeBaseClient {
        pub fn new() -> Self {
            Self {
                expected_results: Arc::new(Mutex::new(Vec::new())),
                call_counts: Arc::new(Mutex::new(HashMap::new())),
            }
        }
        
        pub fn expect_semantic_search(&self) -> &Self {
            let mut call_counts = self.call_counts.lock().unwrap();
            let count = call_counts.entry("semantic_search".to_string()).or_insert(0);
            *count += 1;
            self
        }
        
        pub fn expect_lookup_by_id(&self) -> &Self {
            let mut call_counts = self.call_counts.lock().unwrap();
            let count = call_counts.entry("lookup_by_id".to_string()).or_insert(0);
            *count += 1;
            self
        }
        
        pub fn expect_ingest_yaml_definitions(&self) -> &Self {
            let mut call_counts = self.call_counts.lock().unwrap();
            let count = call_counts.entry("ingest_yaml_definitions".to_string()).or_insert(0);
            *count += 1;
            self
        }
        
        pub fn returning(&self, result: impl Into<Result<serde_json::Value, anyhow::Error>>) -> &Self {
            let mut results = self.expected_results.lock().unwrap();
            results.push(result.into());
            self
        }
        
        pub async fn semantic_search(
            &self,
            _query: &str,
            _k: usize,
            _scope_filter: Option<&str>,
            _entity_type_filter: Option<&str>,
            _tenant_id: &TenantId,
            _trace_ctx: &TraceContext,
        ) -> Result<Vec<SearchResult>, anyhow::Error> {
            let mut call_counts = self.call_counts.lock().unwrap();
            let count = call_counts.entry("semantic_search".to_string()).or_insert(0);
            *count = count.saturating_sub(1);

            // Default mock implementation
            Ok(vec![
                SearchResult {
                    id: Uuid::new_v4().to_string(),
                    entity_type: "ComponentDefinition".to_string(),
                    score: 0.95,
                    data: json!({
                        "name": "TestComponent",
                        "description": "A test component"
                    }),
                }
            ])
        }
        
        pub async fn lookup_by_id(
            &self,
            _entity_type: &str,
            _id: &str,
            _version: Option<&str>,
            _tenant_id: &TenantId,
            _trace_ctx: &TraceContext,
        ) -> Result<Option<serde_json::Value>, anyhow::Error> {
            let mut call_counts = self.call_counts.lock().unwrap();
            let count = call_counts.entry("lookup_by_id".to_string()).or_insert(0);
            *count = count.saturating_sub(1);

            // Default mock implementation
            Ok(Some(json!({
                "id": _id,
                "name": "Test Entity",
                "type": _entity_type
            })))
        }
        
        pub async fn ingest_yaml_definitions(
            &self,
            _yaml_content: &str,
            _scope: &str,
            _source_info: Option<&str>,
            _tenant_id: &TenantId,
            _trace_ctx: &TraceContext,
        ) -> Result<(), anyhow::Error> {
            let mut call_counts = self.call_counts.lock().unwrap();
            let count = call_counts.entry("ingest_yaml_definitions".to_string()).or_insert(0);
            *count = count.saturating_sub(1);

            // Default mock implementation
            Ok(())
        }
    }
}

#[cfg(feature = "mocks")]
pub use mock_implementations::*;

#[cfg(not(feature = "mocks"))]
pub mod mock_implementations {
    // Placeholder module for when mocks feature is not enabled
    
    #[derive(Clone)]
    pub struct MockStateStore;
    
    impl MockStateStore {
        pub fn new() -> Self {
            Self {}
        }
    }
    
    #[derive(Clone)]
    pub struct MockEmbeddingGenerator;
    
    impl MockEmbeddingGenerator {
        pub fn new() -> Self {
            Self {}
        }
    }
    
    #[derive(Clone)]
    pub struct MockKnowledgeBaseClient;
    
    impl MockKnowledgeBaseClient {
        pub fn new() -> Self {
            Self {}
        }
    }
}

#[cfg(not(feature = "mocks"))]
pub use mock_implementations::*; 
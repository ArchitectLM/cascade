use std::collections::HashMap;
use async_trait::async_trait;
use uuid::Uuid;
use serde_json::json;
use tracing::{info, debug};
use std::cell::Cell;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use std::any::Any;
use std::fmt;
use std::sync::{Mutex};
use serde_json::{Value};

use crate::data::{
    errors::{CoreError, StateStoreError},
    identifiers::{TenantId, ComponentDefinitionId},
    trace_context::TraceContext,
    types::{DataPacket, Scope, SourceType},
    entities::{ComponentDefinition},
};
use crate::traits::{StateStore, EmbeddingGenerator, DslParser, ParsedDslDefinitions};

// Create a simplified tenant tracking mechanism
thread_local! {
    static TENANT_A: Cell<Option<String>> = Cell::new(None);
    static TENANT_B: Cell<Option<String>> = Cell::new(None);
    static FIRST_QUERY_TENANT: Cell<Option<String>> = Cell::new(None);
}

/// Fake implementation of StateStore for testing
pub struct FakeStateStore {
    data: Mutex<HashMap<String, Vec<HashMap<String, DataPacket>>>>,
}

impl FakeStateStore {
    /// Creates a new instance of FakeStateStore
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }
    
    /// Adds test data for a specific query
    pub fn add_data_for_query(&self, query: &str, data: Vec<HashMap<String, DataPacket>>) {
        let mut store = self.data.lock().unwrap();
        store.insert(query.to_string(), data);
    }
}

impl fmt::Debug for FakeStateStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeStateStore").finish()
    }
}

#[async_trait]
impl StateStore for FakeStateStore {
    async fn execute_query(
        &self,
        _tenant_id: &TenantId,
        _trace_ctx: &TraceContext,
        query: &str,
        _params: Option<HashMap<String, DataPacket>>,
    ) -> Result<Vec<HashMap<String, DataPacket>>, StateStoreError> {
        let store = self.data.lock().unwrap();
        
        // Check if we have mock data for this query
        if let Some(data) = store.get(query) {
            Ok(data.clone())
        } else {
            // Return empty result set by default
            Ok(Vec::new())
        }
    }
    
    async fn upsert_graph_data(
        &self,
        _tenant_id: &TenantId,
        _trace_ctx: &TraceContext,
        _data: crate::traits::GraphDataPatch,
    ) -> Result<(), StateStoreError> {
        // Pretend to upsert graph data
        Ok(())
    }
    
    async fn vector_search(
        &self,
        _tenant_id: &TenantId,
        _trace_ctx: &TraceContext,
        _index_name: &str,
        _embedding: &[f32],
        _k: usize,
        _filter_params: Option<HashMap<String, DataPacket>>,
    ) -> Result<Vec<(uuid::Uuid, f32)>, StateStoreError> {
        // Return dummy results for testing
        Ok(vec![
            (Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap_or_else(|_| Uuid::new_v4()), 0.95),
            (Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap_or_else(|_| Uuid::new_v4()), 0.85),
        ])
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Fake implementation of EmbeddingGenerator for testing
pub struct FakeEmbeddingService {
    data: Mutex<HashMap<String, Vec<f32>>>,
}

impl FakeEmbeddingService {
    /// Creates a new instance of FakeEmbeddingService
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }
    
    /// Adds a precomputed embedding for a specific text
    pub fn add_embedding(&self, text: &str, embedding: Vec<f32>) {
        let mut store = self.data.lock().unwrap();
        store.insert(text.to_string(), embedding);
    }
}

impl fmt::Debug for FakeEmbeddingService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeEmbeddingService").finish()
    }
}

#[async_trait]
impl EmbeddingGenerator for FakeEmbeddingService {
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, CoreError> {
        let store = self.data.lock().unwrap();
        
        // Return pre-defined embedding if available
        if let Some(embedding) = store.get(text) {
            Ok(embedding.clone())
        } else {
            // Generate a deterministic embedding based on the hash of the text
            // This ensures consistency in testing
            let mut result = Vec::with_capacity(4);
            let hash = text.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32));
            
            // Generate 4D embedding based on hash
            result.push((hash % 100) as f32 / 100.0);
            result.push(((hash >> 8) % 100) as f32 / 100.0);
            result.push(((hash >> 16) % 100) as f32 / 100.0);
            result.push(((hash >> 24) % 100) as f32 / 100.0);
            
            Ok(result)
        }
    }
}

/// Fake implementation of DslParser for testing
#[derive(Debug, Clone, Default)]
pub struct FakeDslParser {
    // Could add fields to track calls or mock responses
}

impl FakeDslParser {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl DslParser for FakeDslParser {
    async fn parse_and_validate_yaml(
        &self,
        _yaml_content: &str,
        _tenant_id: &TenantId,
        _trace_ctx: &TraceContext,
    ) -> Result<ParsedDslDefinitions, CoreError> {
        // For testing, return an empty result
        Ok(ParsedDslDefinitions {
            components: vec![],
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
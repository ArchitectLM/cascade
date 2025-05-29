use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use uuid::Uuid;

use crate::data::{
    errors::StateStoreError,
    identifiers::TenantId,
    trace_context::TraceContext,
    types::DataPacket,
};
use crate::traits::state_store::{StateStore, GraphDataPatch};

/// A fake implementation of StateStore for testing
/// This is a simple in-memory implementation that can be used for unit tests
/// without requiring an actual Neo4j database.
#[derive(Debug, Default)]
pub struct FakeStateStore {
    // We use a HashMap to store nodes and edges
    // The key is the node/edge ID, the value is the properties
    nodes: Arc<Mutex<HashMap<String, HashMap<String, DataPacket>>>>,
    edges: Arc<Mutex<HashMap<String, HashMap<String, DataPacket>>>>,
    // Track vector indices for simulated vector search
    vector_indices: Arc<Mutex<HashMap<String, HashMap<Uuid, Vec<f32>>>>>,
}

impl FakeStateStore {
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(Mutex::new(HashMap::new())),
            edges: Arc::new(Mutex::new(HashMap::new())),
            vector_indices: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add a vector to an index for testing
    pub fn add_vector(&self, index_name: &str, id: Uuid, vector: Vec<f32>) {
        let mut indices = self.vector_indices.lock().unwrap();
        let index = indices.entry(index_name.to_string()).or_insert_with(HashMap::new);
        index.insert(id, vector);
    }

    /// Clear all data in the store
    pub fn clear(&self) {
        self.nodes.lock().unwrap().clear();
        self.edges.lock().unwrap().clear();
        self.vector_indices.lock().unwrap().clear();
    }
}

#[async_trait]
impl StateStore for FakeStateStore {
    async fn execute_query(
        &self,
        tenant_id: &TenantId,
        trace_ctx: &TraceContext,
        query: &str,
        params: Option<HashMap<String, DataPacket>>,
    ) -> Result<Vec<HashMap<String, DataPacket>>, StateStoreError> {
        // For testing, just add a simple result with a success flag
        let mut result = HashMap::new();
        result.insert("query".to_string(), DataPacket::String(query.to_string()));
        result.insert("tenant_id".to_string(), DataPacket::String(tenant_id.to_string()));
        result.insert("trace_id".to_string(), DataPacket::String(trace_ctx.trace_id.to_string()));
        result.insert("result".to_string(), DataPacket::String("success".to_string()));
        
        // Add parameters if provided
        if let Some(p) = params {
            for (key, value) in p {
                result.insert(format!("param_{}", key), value);
            }
        }
        
        Ok(vec![result])
    }

    async fn upsert_graph_data(
        &self,
        tenant_id: &TenantId,
        _trace_ctx: &TraceContext,
        data: GraphDataPatch,
    ) -> Result<(), StateStoreError> {
        // Process nodes
        let tenant_id_str = tenant_id.to_string();
        
        for node in data.nodes {
            if let Some(id) = node.get("id").and_then(|v| v.as_str()) {
                let mut properties = HashMap::new();
                if let Some(obj) = node.as_object() {
                    for (key, value) in obj {
                        // Convert serde_json::Value to DataPacket
                        match value {
                            serde_json::Value::Null => {},
                            serde_json::Value::Bool(b) => {
                                properties.insert(key.clone(), DataPacket::Bool(*b));
                            },
                            serde_json::Value::Number(n) => {
                                if let Some(i) = n.as_i64() {
                                    properties.insert(key.clone(), DataPacket::Integer(i));
                                } else if let Some(f) = n.as_f64() {
                                    properties.insert(key.clone(), DataPacket::Float(f as f32));
                                }
                            },
                            serde_json::Value::String(s) => {
                                properties.insert(key.clone(), DataPacket::String(s.clone()));
                            },
                            serde_json::Value::Array(arr) => {
                                // Handle vector embeddings
                                if key == "embedding" {
                                    let embedding: Vec<f32> = arr.iter()
                                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                                        .collect();
                                    
                                    if !embedding.is_empty() {
                                        properties.insert(key.clone(), DataPacket::FloatArray(embedding.clone()));
                                        
                                        // If this is a node with an embedding, store it in vector indices
                                        if let Some(node_id_str) = properties.get("id")
                                            .and_then(|id| {
                                                match id {
                                                    DataPacket::String(s) => Some(s.clone()),
                                                    _ => None
                                                }
                                            })
                                            .or_else(|| Some(id.to_string())) {
                                            
                                            // Try to parse as UUID
                                            if let Ok(uuid) = Uuid::parse_str(&node_id_str) {
                                                if let Some(label) = node.get("label").and_then(|l| l.as_str()) {
                                                    let index_name = match label {
                                                        "DocumentationChunk" => "docEmbeddings",
                                                        "Content" => "contentEmbeddings",
                                                        "TestEmbedding" => "testEmbeddings",
                                                        _ => "unknownEmbeddings",
                                                    };
                                                    
                                                    self.add_vector(index_name, uuid, embedding);
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    // Convert to strings for simplicity
                                    let string_arr: Vec<DataPacket> = arr.iter()
                                        .map(|v| DataPacket::String(v.to_string()))
                                        .collect();
                                    properties.insert(key.clone(), DataPacket::Array(string_arr));
                                }
                            },
                            serde_json::Value::Object(_) => {
                                // Store as JSON string
                                properties.insert(key.clone(), DataPacket::String(value.to_string()));
                            }
                        }
                    }
                }
                
                // Add tenant_id if not present
                if !properties.contains_key("tenant_id") {
                    properties.insert("tenant_id".to_string(), DataPacket::String(tenant_id_str.clone()));
                }
                
                // Store the node
                let mut nodes = self.nodes.lock().unwrap();
                nodes.insert(id.to_string(), properties);
            }
        }
        
        Ok(())
    }

    async fn vector_search(
        &self,
        tenant_id: &TenantId,
        _trace_ctx: &TraceContext,
        index_name: &str,
        embedding: &[f32],
        k: usize,
        _filter_params: Option<HashMap<String, DataPacket>>,
    ) -> Result<Vec<(Uuid, f32)>, StateStoreError> {
        // Simulate vector search with cosine similarity
        let indices = self.vector_indices.lock().unwrap();
        let index = indices.get(index_name);
        
        match index {
            Some(index_map) => {
                let _tenant_id_str = tenant_id.to_string();
                
                // Filter by tenant_id and calculate similarity scores
                let mut results: Vec<(Uuid, f32)> = Vec::new();
                
                for (id, vec) in index_map {
                    // Skip if vectors are empty
                    if vec.is_empty() || embedding.is_empty() {
                        continue;
                    }
                    
                    // Skip if dimension mismatch
                    if vec.len() != embedding.len() {
                        // Use a smaller common length
                        let min_len = vec.len().min(embedding.len());
                        if min_len == 0 {
                            continue;
                        }
                        
                        // Calculate cosine similarity with the common dimensions
                        let dot_product: f32 = (0..min_len)
                            .map(|i| vec[i] * embedding[i])
                            .sum();
                            
                        let vec_magnitude: f32 = (0..min_len)
                            .map(|i| vec[i] * vec[i])
                            .sum::<f32>()
                            .sqrt();
                            
                        let embedding_magnitude: f32 = (0..min_len)
                            .map(|i| embedding[i] * embedding[i])
                            .sum::<f32>()
                            .sqrt();
                            
                        let similarity = if vec_magnitude > 0.0 && embedding_magnitude > 0.0 {
                            dot_product / (vec_magnitude * embedding_magnitude)
                        } else {
                            0.0
                        };
                        
                        results.push((*id, similarity));
                    } else {
                        // Calculate cosine similarity
                        let dot_product: f32 = vec.iter()
                            .zip(embedding.iter())
                            .map(|(a, b)| a * b)
                            .sum();
                            
                        let vec_magnitude: f32 = vec.iter()
                            .map(|&v| v * v)
                            .sum::<f32>()
                            .sqrt();
                            
                        let embedding_magnitude: f32 = embedding.iter()
                            .map(|&v| v * v)
                            .sum::<f32>()
                            .sqrt();
                            
                        let similarity = if vec_magnitude > 0.0 && embedding_magnitude > 0.0 {
                            dot_product / (vec_magnitude * embedding_magnitude)
                        } else {
                            0.0
                        };
                        
                        results.push((*id, similarity));
                    }
                }
                
                // Sort by similarity score (descending) and take top k
                results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                results.truncate(k);
                
                Ok(results)
            },
            None => {
                // If index doesn't exist, return empty result
                Ok(Vec::new())
            }
        }
    }
    
    // Implement the as_any method required by the trait
    fn as_any(&self) -> &dyn Any {
        self
    }
} 
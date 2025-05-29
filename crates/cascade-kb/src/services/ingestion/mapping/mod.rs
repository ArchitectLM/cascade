pub mod app_state;
pub mod yaml_defs;

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use crate::traits::state_store::GraphDataPatch;

/// Represents a node in the graph database.
pub struct NodeData {
    pub label: String,
    pub properties: HashMap<String, JsonValue>,
}

/// Represents an edge in the graph database.
pub struct EdgeData {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: String,
    pub properties: HashMap<String, JsonValue>,
}

// Re-export GraphDataPatch from traits::state_store for compatibility
pub type GraphPatch = GraphDataPatch;

impl GraphPatch {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
} 
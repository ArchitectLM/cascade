//! StateStore trait definition for graph database interaction

use async_trait::async_trait;
use crate::{
    data::{
        errors::StateStoreError,
        identifiers::TenantId,
        trace_context::TraceContext,
        types::DataPacket,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::any::Any;
use uuid::Uuid;

/// Represents a set of nodes and relationships to be upserted into the graph database.
/// This structure is used by the `StateStore::upsert_graph_data` method to perform
/// idempotent updates to the graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphDataPatch {
    /// List of nodes to create or update, represented as generic JSON values
    /// to allow flexibility in node properties and labels.
    pub nodes: Vec<serde_json::Value>,
    
    /// List of relationships to create or update, represented as generic JSON values
    /// to allow flexibility in relationship types and properties.
    pub edges: Vec<serde_json::Value>,
}

/// Represents the interface for interacting with the graph database (State Store).
/// This abstracts the underlying database technology (e.g., Neo4j).
/// Based on Section 1.4 Technology and the need to store/retrieve graph data.
#[async_trait]
pub trait StateStore: Send + Sync {
    /// Executes a Cypher query.
    ///
    /// Contract: Executes the provided query string with parameters within the context
    /// of the given tenant. Returns raw graph data or a specific result structure.
    /// Handles connection pooling, transaction management (implicitly or explicitly via methods),
    /// and mapping raw database results into a usable format.
    async fn execute_query(
        &self,
        tenant_id: &TenantId,
        trace_ctx: &TraceContext,
        query: &str,
        params: Option<HashMap<String, DataPacket>>,
    ) -> Result<Vec<HashMap<String, DataPacket>>, StateStoreError>;

    /// Creates or updates nodes and relationships using an idempotent `MERGE` style operation.
    ///
    /// Contract: Takes a graph patch or a set of nodes/relationships and persists them.
    /// Uses database-specific upsert logic (`MERGE` in Cypher) to avoid duplicates and handle updates.
    /// Should be transactional. Handles mapping internal Rust structures to database representation.
    async fn upsert_graph_data(
        &self,
        tenant_id: &TenantId,
        trace_ctx: &TraceContext,
        data: GraphDataPatch,
    ) -> Result<(), StateStoreError>;

    /// Performs a vector search query against a specified index.
    ///
    /// Contract: Queries the configured vector index (Section 5 Indexing Strategy)
    /// with the given embedding vector to find similar nodes (e.g., DocumentationChunks).
    /// Returns matching nodes and their similarity scores.
    async fn vector_search(
        &self,
        tenant_id: &TenantId,
        trace_ctx: &TraceContext,
        index_name: &str,
        embedding: &[f32],
        k: usize,
        filter_params: Option<HashMap<String, DataPacket>>, // Optional filters
    ) -> Result<Vec<(Uuid, f32)>, StateStoreError>; // Return node ID and score
    
    /// Returns `self` as an `&dyn Any` for downcasting to concrete type.
    ///
    /// This method allows testing code to access implementation-specific
    /// functionality when needed, such as for setting up vector indexes.
    fn as_any(&self) -> &dyn Any;
} 
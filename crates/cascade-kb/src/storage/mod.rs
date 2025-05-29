use async_trait::async_trait;
use std::collections::HashMap;

use crate::data::{CoreError, Entity, EntityRef, Scope};

/// Storage query parameters
#[derive(Debug, Clone)]
pub struct QueryParams {
    pub filters: HashMap<String, String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Storage interface for the Knowledge Base
#[async_trait]
pub trait KnowledgeStore: Send + Sync + 'static {
    /// Store a new entity
    async fn store_entity(&self, entity: Entity) -> Result<(), CoreError>;

    /// Retrieve an entity by its reference
    async fn get_entity(&self, entity_ref: &EntityRef) -> Result<Option<Entity>, CoreError>;

    /// Delete an entity by its reference
    async fn delete_entity(&self, entity_ref: &EntityRef) -> Result<(), CoreError>;

    /// List entities of a specific type within a scope
    async fn list_entities(
        &self,
        entity_type: &str,
        scope: &Scope,
        params: &QueryParams,
    ) -> Result<Vec<Entity>, CoreError>;

    /// Search entities using semantic similarity
    async fn semantic_search(
        &self,
        query_text: &str,
        entity_type: Option<&str>,
        scope: &Scope,
        limit: usize,
    ) -> Result<Vec<Entity>, CoreError>;

    /// Query entities using a graph traversal
    async fn graph_query(
        &self,
        start_entity: &EntityRef,
        traversal_params: HashMap<String, String>,
    ) -> Result<Vec<Entity>, CoreError>;
}

/// Storage factory trait for creating new storage instances
#[async_trait]
pub trait StorageFactory: Send + Sync + 'static {
    /// Create a new storage instance
    async fn create_storage(&self) -> Result<Box<dyn KnowledgeStore>, CoreError>;
}

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub storage_type: String,
    pub connection_string: String,
    pub max_connections: u32,
    pub timeout_seconds: u64,
} 
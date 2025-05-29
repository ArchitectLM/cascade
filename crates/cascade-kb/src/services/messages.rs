//! Message types for service communication

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use std::collections::HashMap;
use crate::data::{CoreError, TenantId, TraceContext, DataPacket, Scope};
use uuid;

/// Message type for ingestion operations
#[derive(Debug, Clone)]
pub enum IngestionMessage {
    YamlDefinitions {
        tenant_id: TenantId,
        trace_ctx: TraceContext,
        yaml_content: String,
        scope: Scope,
        source_info: Option<String>,
    },
    ApplicationState {
        tenant_id: TenantId,
        trace_ctx: TraceContext,
        snapshot_data: DataPacket,
        source_info: String,
    },
    DocumentChunk {
        tenant_id: TenantId,
        trace_ctx: TraceContext,
        chunk: crate::data::entities::DocumentationChunk,
        linked_entity_type: Option<String>,
        linked_entity_id: Option<String>,
    },
    CreateVersionSet {
        tenant_id: TenantId,
        version_set_id: uuid::Uuid,
        scope: Scope,
        trace_context: TraceContext,
    },
    CreateVersion {
        tenant_id: TenantId,
        version_set_id: uuid::Uuid,
        version_id: uuid::Uuid,
        scope: Scope,
        trace_context: TraceContext,
    },
}

/// Request type for queries
#[derive(Debug)]
pub enum QueryRequest {
    LookupById {
        tenant_id: TenantId,
        trace_ctx: TraceContext,
        entity_type: String,
        id: String,
        version: Option<String>,
    },
    SemanticSearch {
        tenant_id: TenantId,
        trace_ctx: TraceContext,
        query_text: String,
        k: usize,
        scope_filter: Option<Scope>,
        entity_type_filter: Option<String>,
    },
    GraphTraversal {
        tenant_id: TenantId,
        trace_ctx: TraceContext,
        cypher_query: String,
        params: HashMap<String, DataPacket>,
    },
}

/// Response type for queries
#[derive(Debug)]
pub enum QueryResponse {
    Success(Vec<HashMap<String, DataPacket>>),
    Error(CoreError),
}

/// Wrapper for the oneshot sender to return query results
#[derive(Debug)]
pub struct QueryResultSender {
    pub sender: oneshot::Sender<QueryResponse>,
}

/// Application state snapshot for ingestion
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplicationStateSnapshot {
    pub tenant_id: String,
    pub trace_id: String,
    pub components: Vec<serde_json::Value>,
    pub connections: Vec<serde_json::Value>,
} 
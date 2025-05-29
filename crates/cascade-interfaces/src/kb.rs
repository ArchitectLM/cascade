//! Knowledge Base interfaces for Cascade
//!
//! This module defines the interfaces for interacting with the Cascade Knowledge Base,
//! particularly for the Conversational DSL Agent System (CDAS).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use serde_json::Value;

/// Result type for Knowledge Base operations
pub type KnowledgeResult<T> = Result<T, KnowledgeError>;

/// Errors that can occur when interacting with the Knowledge Base
#[derive(Error, Debug, Clone)]
pub enum KnowledgeError {
    /// Access denied to the requested resource
    #[error("Access denied: {0}")]
    AccessDenied(String),
    
    /// The requested resource was not found
    #[error("Resource not found: {0}")]
    NotFound(String),
    
    /// Error during validation
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    /// Error during serialization or deserialization
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    /// Error communicating with the KB service
    #[error("Communication error: {0}")]
    CommunicationError(String),
    
    /// Internal KB error
    #[error("Internal KB error: {0}")]
    InternalError(String),
    
    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),
    
    /// Invalid input parameters
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

/// Detailed information about a DSL element from the graph database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphElementDetails {
    /// Unique identifier of the element
    pub id: String,
    
    /// Type of the element (ComponentDefinition, FlowDefinition, etc)
    pub element_type: String,
    
    /// Display name of the element
    pub name: String,
    
    /// Optional description
    pub description: Option<String>,
    
    /// Optional version string
    pub version: Option<String>,
    
    /// Optional framework identifier
    pub framework: Option<String>,
    
    /// Optional JSON schema for the element
    pub schema: Option<Value>,
    
    /// Optional list of input definitions
    pub inputs: Option<Value>,
    
    /// Optional list of output definitions
    pub outputs: Option<Value>,
    
    /// Optional list of step definitions (for flows)
    pub steps: Option<Value>,
    
    /// Optional list of tags
    pub tags: Option<Vec<String>>,
    
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
    
    /// Additional metadata as a JSON object
    pub metadata: Option<Value>,
}

/// Retrieved context from knowledge base artefacts like documentation or examples
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedContext {
    /// Unique identifier
    pub id: String,
    
    /// Type of context (documentation, example, etc)
    pub context_type: String,
    
    /// Title of the document or example
    pub title: String,
    
    /// Content or snippet from the document or example
    pub content: String,
    
    /// Source URL or reference
    pub source_url: Option<String>,
    
    /// Type of entity (Component, Flow, etc)
    pub entity_type: Option<String>,
    
    /// Creation timestamp
    pub created_at: Option<DateTime<Utc>>,
    
    /// Relevance score (0.0 to 1.0)
    pub relevance_score: f64,
}

/// Detailed information about a validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationErrorDetail {
    /// Type of error (SyntaxError, SchemaError, ReferenceError, etc)
    pub error_type: String,
    
    /// Human-readable error message
    pub message: String,
    
    /// Optional JSON path or location where the error occurred
    pub location: Option<String>,
    
    /// Optional additional context about the error
    pub context: Option<Value>,
}

/// Input definition for components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDefinition {
    /// Name of the input
    pub name: String,
    
    /// Type of the input
    pub data_type: String,
    
    /// Whether the input is required
    pub required: bool,
    
    /// Description of the input
    pub description: Option<String>,
    
    /// Default value, if any
    pub default_value: Option<serde_json::Value>,
}

/// Output definition for components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDefinition {
    /// Name of the output
    pub name: String,
    
    /// Type of the output
    pub data_type: String,
    
    /// Description of the output
    pub description: Option<String>,
}

/// Summary information about a step in a flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepSummary {
    /// ID of the step
    pub id: String,
    
    /// Name of the step
    pub name: String,
    
    /// Type of component used in the step
    pub component_type: String,
    
    /// Description of the step
    pub description: Option<String>,
}

/// Contract for the KB client for DSL operations
#[async_trait]
pub trait KnowledgeBaseClient: Send + Sync {
    /// Contract: Retrieves detailed information about a specific DSL graph element.
    /// - `tenant_id`: The tenant scope for the request.
    /// - `entity_id`: The unique identifier of the DSL element.
    /// - `version`: Optional specific version of the element.
    /// - Returns: `KnowledgeResult<Option<GraphElementDetails>>`. `Ok(None)` if not found. `Err` on KB access error.
    async fn get_element_details(&self, tenant_id: &str, entity_id: &str, version: Option<&str>) -> KnowledgeResult<Option<GraphElementDetails>>;

    /// Contract: Searches for KB artefacts (docs, examples, related elements) relevant to a query or entity. Used for RAG context retrieval.
    /// - `tenant_id`: The tenant scope.
    /// - `query_text`: Optional natural language query for semantic search.
    /// - `entity_id`/`version`: Optional specific element to find related artefacts for.
    /// - `limit`: Maximum number of results to return.
    /// - Returns: `KnowledgeResult<Vec<RetrievedContext>>`.
    /// - CRITICAL: Each `RetrievedContext` *must* contain a unique, persistent identifier (e.g., URI, DB ID) that can be stored in `FeedbackLog.retrieved_context_ids`.
    async fn search_related_artefacts(&self, tenant_id: &str, query_text: Option<&str>, entity_id: Option<&str>, version: Option<&str>, limit: usize) -> KnowledgeResult<Vec<RetrievedContext>>;

    /// Contract: Validates a given DSL code snippet against the KB's schema/rules (potentially using cascade-dsl).
    /// - `tenant_id`: The tenant scope.
    /// - `dsl_code`: The DSL code string to validate.
    /// - Returns: `KnowledgeResult<Vec<ValidationErrorDetail>>`. `Ok([])` indicates valid DSL. `Ok` with non-empty Vec contains validation errors. `Err` on KB access/validation system error.
    async fn validate_dsl(&self, tenant_id: &str, dsl_code: &str) -> KnowledgeResult<Vec<ValidationErrorDetail>>;

    /// Contract: Traces the source(s) of data for a specific input of a step within a flow definition in the KB.
    /// - `tenant_id`: The tenant scope.
    /// - `flow_entity_id`/`flow_version`: Identifies the flow definition.
    /// - `step_id`: Identifies the step within the flow.
    /// - `component_input_name`: Identifies the specific input port of the step's component.
    /// - Returns: `KnowledgeResult<Vec<(String, String)>>` where each tuple is `(source_step_id, source_output_name)`. `Err` on KB access error or invalid trace path.
    async fn trace_step_input_source(&self, tenant_id: &str, flow_entity_id: &str, flow_version: Option<&str>, step_id: &str, component_input_name: &str) -> KnowledgeResult<Vec<(String, String)>>;
} 
//! Error types for the Cascade Graph Knowledge Base

use thiserror::Error;
use serde_json;
use crate::data::TenantId;
use crate::data::TraceContext;

/// Base Error type for core operations.
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Invalid data reference: {0}")]
    InvalidDataReference(String),
    
    #[error("Entity not found: type={entity_type} id={id}")]
    NotFound {
        entity_type: String,
        id: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Version conflict: {0}")]
    VersionConflict(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Database error with context: {message}")]
    DatabaseErrorWithContext {
        message: String,
        tenant_id: Option<TenantId>,
        trace_id: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Ingestion error: {0}")]
    IngestionError(String),
    
    #[error("Ingestion error with context: {message}")]
    IngestionErrorWithContext {
        message: String,
        tenant_id: Option<TenantId>,
        trace_id: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Query error: {0}")]
    QueryError(String),
    
    #[error("Query error with context: {message}")]
    QueryErrorWithContext {
        message: String,
        tenant_id: Option<TenantId>,
        trace_id: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Serialization/Deserialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Embedding generation error: {0}")]
    EmbeddingError(String),
    
    #[error("Embedding error with context: {message}")]
    EmbeddingErrorWithContext {
        message: String,
        tenant_id: Option<TenantId>,
        trace_id: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Invalid input: {0}")]
    ValidationError(String),
    
    #[error("Operation not supported: {0}")]
    UnsupportedOperation(String),
    
    #[error("Internal system error: {0}")]
    Internal(String),
    
    #[error("Internal system error with context: {message}")]
    InternalWithContext {
        message: String,
        tenant_id: Option<TenantId>,
        trace_id: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl CoreError {
    /// Helper to create a database error with context
    pub fn db_error_with_context<E>(
        message: impl Into<String>,
        tenant_id: Option<TenantId>,
        trace_ctx: Option<&TraceContext>,
        source: Option<E>,
    ) -> Self 
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        CoreError::DatabaseErrorWithContext {
            message: message.into(),
            tenant_id,
            trace_id: trace_ctx.map(|ctx| ctx.trace_id.to_string()),
            source: source.map(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
        }
    }

    /// Helper to create an ingestion error with context
    pub fn ingestion_error_with_context<E>(
        message: impl Into<String>,
        tenant_id: Option<TenantId>,
        trace_ctx: Option<&TraceContext>,
        source: Option<E>,
    ) -> Self 
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        CoreError::IngestionErrorWithContext {
            message: message.into(),
            tenant_id,
            trace_id: trace_ctx.map(|ctx| ctx.trace_id.to_string()),
            source: source.map(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
        }
    }

    /// Helper to create a query error with context
    pub fn query_error_with_context<E>(
        message: impl Into<String>,
        tenant_id: Option<TenantId>,
        trace_ctx: Option<&TraceContext>,
        source: Option<E>,
    ) -> Self 
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        CoreError::QueryErrorWithContext {
            message: message.into(),
            tenant_id,
            trace_id: trace_ctx.map(|ctx| ctx.trace_id.to_string()),
            source: source.map(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
        }
    }

    /// Helper to create an embedding error with context
    pub fn embedding_error_with_context<E>(
        message: impl Into<String>,
        tenant_id: Option<TenantId>,
        trace_ctx: Option<&TraceContext>,
        source: Option<E>,
    ) -> Self 
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        CoreError::EmbeddingErrorWithContext {
            message: message.into(),
            tenant_id,
            trace_id: trace_ctx.map(|ctx| ctx.trace_id.to_string()),
            source: source.map(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
        }
    }

    /// Helper to create an internal error with context
    pub fn internal_with_context<E>(
        message: impl Into<String>,
        tenant_id: Option<TenantId>,
        trace_ctx: Option<&TraceContext>,
        source: Option<E>,
    ) -> Self 
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        CoreError::InternalWithContext {
            message: message.into(),
            tenant_id,
            trace_id: trace_ctx.map(|ctx| ctx.trace_id.to_string()),
            source: source.map(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
        }
    }

    /// Helper to create a not found error
    pub fn not_found<E>(
        entity_type: impl Into<String>,
        id: impl Into<String>,
        source: Option<E>,
    ) -> Self 
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        CoreError::NotFound {
            entity_type: entity_type.into(),
            id: id.into(),
            source: source.map(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
        }
    }
}

/// Specific error type for the State Store (Graph Database interaction).
#[derive(Error, Debug)]
pub enum StateStoreError {
    #[error("Graph database connection error: {0}")]
    ConnectionError(String),
    #[error("Graph query execution error: {0}")]
    QueryError(String),
    #[error("Data mapping error from graph result: {0}")]
    MappingError(String),
    #[error("Transaction error: {0}")]
    TransactionError(String),
    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),
    #[error("Optimistic locking failure: {0}")]
    OptimisticLockError(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Unknown database error: {0}")]
    Unknown(String),
}

/// Convert a String into a StateStoreError::Unknown
impl From<String> for StateStoreError {
    fn from(error: String) -> Self {
        StateStoreError::Unknown(error)
    }
}

/// Specific error type for the Component Loader.
#[derive(Error, Debug)]
pub enum ComponentLoaderError {
    #[error("Component not found: {0}")]
    NotFound(String),
    #[error("Failed to load component code: {0}")]
    LoadFailed(String),
    #[error("Component validation failed: {0}")]
    ValidationError(String),
}

/// Error type for Component API operations.
#[derive(Error, Debug)]
pub enum ComponentApiError {
    #[error("Component execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Invalid input configuration: {0}")]
    InvalidConfiguration(String),
    #[error("Invalid input data: {0}")]
    InvalidInputData(String),
    #[error("Component internal error: {0}")]
    InternalError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{TenantId, TraceContext};

    #[test]
    fn test_core_error_display() {
        let error = CoreError::InvalidDataReference("reference".into());
        assert_eq!(format!("{}", error), "Invalid data reference: reference");
    }

    #[test]
    fn test_state_store_error_display() {
        let error = StateStoreError::ConnectionError("connection failed".into());
        assert_eq!(format!("{}", error), "Graph database connection error: connection failed");
    }

    #[test]
    fn test_component_loader_error_display() {
        let error = ComponentLoaderError::NotFound("component".into());
        assert_eq!(format!("{}", error), "Component not found: component");
    }

    #[test]
    fn test_component_api_error_display() {
        let error = ComponentApiError::ExecutionFailed("execution failed".into());
        assert_eq!(format!("{}", error), "Component execution failed: execution failed");
    }

    #[test]
    fn test_error_with_context() {
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        
        let error = CoreError::db_error_with_context(
            "Database connection failed",
            Some(tenant_id),
            Some(&trace_ctx),
            None::<StateStoreError>,
        );
        
        match error {
            CoreError::DatabaseErrorWithContext { message, tenant_id: tid, trace_id, source: _ } => {
                assert_eq!(message, "Database connection failed");
                assert_eq!(tid, Some(tenant_id));
                assert_eq!(trace_id, Some(trace_ctx.trace_id.to_string()));
            },
            _ => panic!("Expected DatabaseErrorWithContext"),
        }
    }

    #[test]
    fn test_not_found_error() {
        let error = CoreError::not_found("ComponentDefinition", "comp1", None::<StateStoreError>);
        
        match error {
            CoreError::NotFound { entity_type, id, source: _ } => {
                assert_eq!(entity_type, "ComponentDefinition");
                assert_eq!(id, "comp1");
            },
            _ => panic!("Expected NotFound"),
        }
    }
} 
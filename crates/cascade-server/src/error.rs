//! Error types for the Cascade Server
//!
//! This module contains the error types used throughout the server.

use thiserror::Error;
use cascade_content_store::ContentStoreError;

/// Server error types
#[derive(Error, Debug)]
pub enum ServerError {
    /// Resource not found
    #[error("{0} not found")]
    NotFound(String),
    
    /// Validation error
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    /// DSL parsing error
    #[error("DSL parsing error: {0}")]
    DslParsingError(String),
    
    /// Unauthorized error
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    
    /// Edge platform error
    #[error("Edge platform error: {0}")]
    EdgePlatformError(String),
    
    /// Content store error
    #[error("Content store error: {0}")]
    ContentStoreError(String),
    
    /// Content store error with detailed error
    #[error("Content store error: {0}")]
    ContentStoreErrorDetailed(Box<ContentStoreError>),
    
    /// Runtime error
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    /// Internal server error
    #[error("Internal server error: {0}")]
    InternalError(String),
    
    /// Runtime creation error
    #[error("Runtime creation error: {0}")]
    RuntimeCreationError(String),
    
    /// State service error
    #[error("State service error: {0}")]
    StateServiceError(String),
    
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    /// Garbage collection error
    #[error("Garbage collection error: {0}")]
    GarbageCollectionError(String),
    
    /// Rate limit exceeded
    #[error("Rate limit exceeded for {resource}{identifier}: {max_requests} requests per {window_ms}ms")]
    RateLimitExceeded {
        /// Resource being rate limited
        resource: String,
        /// Optional identifier (user ID, API key, etc.)
        identifier: String,
        /// Maximum requests allowed in the time window
        max_requests: u32,
        /// Time window in milliseconds
        window_ms: u64,
    },
    
    /// Circuit breaker open
    #[error("Circuit breaker open for {service}{operation} after {failures} failures. Retry after {timeout_ms}ms")]
    CircuitBreakerOpen {
        /// Service being protected
        service: String,
        /// Optional operation within the service
        operation: String,
        /// Number of consecutive failures
        failures: u32,
        /// Reset timeout in milliseconds
        timeout_ms: u64,
    },
    
    /// Bulkhead rejected due to capacity
    #[error("Bulkhead rejected for {service}{operation}: {active}/{max_concurrent} active, {queue_size}/{max_queue_size} queued")]
    BulkheadRejected {
        /// Service being protected
        service: String,
        /// Optional operation within the service
        operation: String,
        /// Current active executions
        active: u32,
        /// Maximum concurrent executions
        max_concurrent: u32,
        /// Current queue size
        queue_size: u32,
        /// Maximum queue size
        max_queue_size: u32,
    },
    
    /// Bulkhead timeout while waiting in queue
    #[error("Bulkhead queue timeout for {service}{operation} after {timeout_ms}ms")]
    BulkheadTimeout {
        /// Service being protected
        service: String,
        /// Optional operation within the service
        operation: String,
        /// Timeout in milliseconds
        timeout_ms: u64,
    },
}

/// Result type for server operations
pub type ServerResult<T> = Result<T, ServerError>;

// Implement conversions from other error types
impl From<ContentStoreError> for ServerError {
    fn from(err: ContentStoreError) -> Self {
        match err {
            ContentStoreError::NotFound(hash) => {
                ServerError::NotFound(format!("Content with hash {}", hash.as_str()))
            }
            ContentStoreError::ManifestNotFound(flow_id) => {
                ServerError::NotFound(format!("Manifest for flow {}", flow_id))
            }
            ContentStoreError::InvalidHashFormat(hash) => {
                ServerError::ValidationError(format!("Invalid content hash format: {}", hash))
            }
            ContentStoreError::GarbageCollectionError(msg) => {
                ServerError::GarbageCollectionError(msg)
            }
            _ => ServerError::ContentStoreError(format!("{}", err)),
        }
    }
}

impl From<serde_json::Error> for ServerError {
    fn from(err: serde_json::Error) -> Self {
        ServerError::ValidationError(format!("JSON error: {}", err))
    }
}

impl From<serde_yaml::Error> for ServerError {
    fn from(err: serde_yaml::Error) -> Self {
        ServerError::DslParsingError(format!("YAML error: {}", err))
    }
}

impl From<reqwest::Error> for ServerError {
    fn from(err: reqwest::Error) -> Self {
        ServerError::EdgePlatformError(format!("HTTP request error: {}", err))
    }
}

impl From<std::io::Error> for ServerError {
    fn from(err: std::io::Error) -> Self {
        ServerError::InternalError(format!("IO error: {}", err))
    }
}

impl From<anyhow::Error> for ServerError {
    fn from(err: anyhow::Error) -> Self {
        ServerError::InternalError(format!("Error: {}", err))
    }
}

impl ServerError {
    /// Check if the error is a rate limit error
    pub fn is_rate_limit_error(&self) -> bool {
        matches!(self, ServerError::RateLimitExceeded { .. })
    }
    
    /// Check if the error is a circuit breaker error
    pub fn is_circuit_breaker_error(&self) -> bool {
        matches!(self, ServerError::CircuitBreakerOpen { .. })
    }
    
    /// Check if the error is a bulkhead error
    pub fn is_bulkhead_error(&self) -> bool {
        matches!(self, ServerError::BulkheadRejected { .. } | ServerError::BulkheadTimeout { .. })
    }
    
    /// Check if the error is a resilience-related error
    pub fn is_resilience_error(&self) -> bool {
        self.is_rate_limit_error() || self.is_circuit_breaker_error() || self.is_bulkhead_error()
    }
    
    /// Check if the error is a garbage collection error
    pub fn is_garbage_collection_error(&self) -> bool {
        matches!(self, ServerError::GarbageCollectionError(_) | ServerError::ContentStoreErrorDetailed(_))
    }
} 
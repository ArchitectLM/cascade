//! Error handling for the Cascade Server API
//!
//! This module contains standardized error handling for the API.

use axum::{
    response::IntoResponse,
    http::StatusCode,
    Json,
};
use serde_json::json;
use std::fmt::Debug;

use crate::error::ServerError;

/// API Error type for returning standard error responses
pub enum ApiError {
    /// Bad request (400)
    BadRequest(String),
    /// Unauthorized (401)
    Unauthorized(String),
    /// Forbidden (403)
    Forbidden(String),
    /// Not found (404)
    NotFound(String),
    /// Internal server error (500)
    InternalServerError(String),
    /// Service unavailable (503)
    ServiceUnavailable(String),
    /// Too many requests (429)
    TooManyRequests(String),
    /// Wrapped server error
    ServerError(ServerError),
}

impl From<ServerError> for ApiError {
    fn from(err: ServerError) -> Self {
        ApiError::ServerError(err)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
            ApiError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            ApiError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            ApiError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            ApiError::InternalServerError(msg) => write!(f, "Internal Server Error: {}", msg),
            ApiError::ServiceUnavailable(msg) => write!(f, "Service Unavailable: {}", msg),
            ApiError::TooManyRequests(msg) => write!(f, "Too Many Requests: {}", msg),
            ApiError::ServerError(err) => write!(f, "Server Error: {}", err),
        }
    }
}

impl std::fmt::Debug for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::BadRequest(msg) => write!(f, "BadRequest({})", msg),
            ApiError::Unauthorized(msg) => write!(f, "Unauthorized({})", msg),
            ApiError::Forbidden(msg) => write!(f, "Forbidden({})", msg),
            ApiError::NotFound(msg) => write!(f, "NotFound({})", msg),
            ApiError::InternalServerError(msg) => write!(f, "InternalServerError({})", msg),
            ApiError::ServiceUnavailable(msg) => write!(f, "ServiceUnavailable({})", msg),
            ApiError::TooManyRequests(msg) => write!(f, "TooManyRequests({})", msg),
            ApiError::ServerError(err) => write!(f, "ServerError({:?})", err),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_code, message) = match &self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "ERR_BAD_REQUEST", msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "ERR_UNAUTHORIZED", msg),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, "ERR_FORBIDDEN", msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "ERR_NOT_FOUND", msg),
            ApiError::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "ERR_INTERNAL_SERVER_ERROR", msg),
            ApiError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, "ERR_SERVICE_UNAVAILABLE", msg),
            ApiError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, "ERR_TOO_MANY_REQUESTS", msg),
            ApiError::ServerError(err) => {
                // Use our existing error handler for ServerError
                return api_error_response(err);
            }
        };

        let body = Json(json!({
            "error": message,
            "errorDetails": {
                "errorCode": error_code,
                "errorMessage": message,
            }
        }));

        (status, body).into_response()
    }
}

/// General error response handler for API errors
/// This will convert any error into a standardized API error response
pub fn api_error_response<E: Debug + std::fmt::Display + 'static>(err: &E) -> axum::response::Response<axum::body::Body> {
    let err_string = format!("{}", err);
    let err_debug = format!("{:?}", err);
    
    // Determine status code and error code based on error type
    let (status_code, error_code, error_message) = if let Some(server_err) = as_server_error(err) {
        match server_err {
            ServerError::NotFound(resource) => (
                StatusCode::NOT_FOUND,
                format!("ERR_NOT_FOUND_{}", resource.to_uppercase()),
                format!("{} not found", resource),
            ),
            ServerError::ValidationError(msg) => (
                StatusCode::BAD_REQUEST,
                "ERR_VALIDATION_ERROR".to_string(),
                msg.clone(),
            ),
            ServerError::DslParsingError(msg) => (
                StatusCode::BAD_REQUEST,
                "ERR_DSL_PARSING_INVALID_YAML".to_string(),
                msg.clone(),
            ),
            ServerError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                "ERR_UNAUTHORIZED".to_string(),
                msg.clone(),
            ),
            ServerError::EdgePlatformError(msg) => (
                StatusCode::BAD_GATEWAY,
                "ERR_EDGE_PLATFORM_ERROR".to_string(),
                msg.clone(),
            ),
            ServerError::RuntimeError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ERR_RUNTIME_ERROR".to_string(),
                msg.clone(),
            ),
            ServerError::ConfigError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ERR_CONFIG_ERROR".to_string(),
                msg.clone(),
            ),
            ServerError::InternalError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ERR_INTERNAL_SERVER_ERROR".to_string(),
                msg.clone(),
            ),
            ServerError::RuntimeCreationError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ERR_RUNTIME_CREATION_ERROR".to_string(),
                msg.clone(),
            ),
            ServerError::StateServiceError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ERR_STATE_SERVICE_ERROR".to_string(),
                msg.clone(),
            ),
            ServerError::ConfigurationError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ERR_CONFIGURATION_ERROR".to_string(),
                msg.clone(),
            ),
            ServerError::ContentStoreError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ERR_CONTENT_STORE_ERROR".to_string(),
                msg.clone(),
            ),
            ServerError::RateLimitExceeded { resource, identifier, max_requests, window_ms } => (
                StatusCode::TOO_MANY_REQUESTS,
                "ERR_RATE_LIMIT_EXCEEDED".to_string(),
                format!("Rate limit exceeded for {}{}: {} requests per {}ms", resource, identifier, max_requests, window_ms),
            ),
            ServerError::CircuitBreakerOpen { service, operation, failures, timeout_ms } => (
                StatusCode::SERVICE_UNAVAILABLE,
                "ERR_CIRCUIT_BREAKER_OPEN".to_string(),
                format!("Circuit breaker open for {}{} after {} failures. Retry after {}ms", 
                    service, operation, failures, timeout_ms),
            ),
            ServerError::ContentStoreErrorDetailed(ref err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ERR_CONTENT_STORE_ERROR".to_string(),
                format!("{}", err),
            ),
            ServerError::GarbageCollectionError(ref message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ERR_GARBAGE_COLLECTION_ERROR".to_string(),
                message.clone(),
            ),
            ServerError::BulkheadRejected { ref service, ref operation, active, max_concurrent, queue_size, max_queue_size } => (
                StatusCode::TOO_MANY_REQUESTS,
                "ERR_BULKHEAD_REJECTED".to_string(),
                format!("Bulkhead rejected for {}{}: {}/{} active, {}/{} queued", 
                    service, operation, active, max_concurrent, queue_size, max_queue_size),
            ),
            ServerError::BulkheadTimeout { ref service, ref operation, timeout_ms } => (
                StatusCode::REQUEST_TIMEOUT,
                "ERR_BULKHEAD_TIMEOUT".to_string(),
                format!("Bulkhead queue timeout for {}{} after {}ms", 
                    service, operation, timeout_ms),
            ),
        }
    } else {
        // Generic error fallback
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "ERR_UNKNOWN".to_string(),
            err_string,
        )
    };
    
    // Create standardized error response
    let error_response = json!({
        "error": error_message,
        "errorDetails": {
            "errorCode": error_code,
            "errorMessage": error_message,
            "details": {
                "debug": err_debug
            }
        }
    });
    
    (status_code, Json(error_response)).into_response()
}

/// Helper function to convert an error to a ServerError if possible
fn as_server_error<E: Debug + std::fmt::Display + 'static>(err: &E) -> Option<&ServerError> {
    // Try to cast to Any so we can downcast it
    let err_any = err as &dyn std::any::Any;
    
    // If err is a ServerError reference, return it
    if let Some(server_err) = err_any.downcast_ref::<ServerError>() {
        return Some(server_err);
    }
    
    // Try to downcast to a reference to a Result<T, ServerError>
    if let Some(Err(server_err)) = err_any.downcast_ref::<Result<(), ServerError>>() {
        return Some(server_err);
    }
    
    None
} 
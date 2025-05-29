//! API module for the Cascade Server
//!
//! This module contains the API routes and handlers for the Cascade Server.

use axum::{
    Router,
    routing::{get, post},
    extract::{Path, Json, State},
    http::{StatusCode, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
    middleware as axum_middleware,
};
use serde::{Serialize, Deserialize};
use serde_json::json;
use std::sync::Arc;
use std::collections::HashMap;
use base64;

pub mod admin;
pub mod edge;
pub mod health;
pub mod errors;
pub mod middleware;

use crate::server::CascadeServer;

/// Build the router for API endpoints
pub fn build_router(server: Arc<CascadeServer>) -> Router {
    Router::new()
        // Content management
        .route("/v1/content/:hash", get(handle_get_content))
        .route("/v1/content/batch", post(handle_batch_get_content))
        
        // Flow management
        .route("/v1/admin/flows", get(admin::list_flows_handler))
        .route("/v1/admin/flows/:flow_id", get(admin::get_flow_handler))
        .route("/v1/admin/flows/:flow_id/deploy", post(admin::deploy_flow_handler))
        .route("/v1/admin/flows/:flow_id/undeploy", post(admin::undeploy_flow_handler))
        .route("/v1/admin/flows/:flow_id/manifest", get(admin::get_flow_manifest_handler))
        
        // Content store management
        .route("/v1/admin/content/gc", post(admin::run_garbage_collection_handler))
        .route("/v1/admin/cache", post(admin::cache_operations_handler))
        
        // Edge callbacks
        .route("/v1/edge/callback", post(edge::handle_edge_callback))
        
        // Health check
        .route("/health", get(health::health_check))
        
        // Shared state
        .with_state(server)
}

/// Authentication middleware
async fn _auth_middleware(
    headers: HeaderMap,
    State(server): State<Arc<CascadeServer>>,
    request: axum::http::Request<axum::body::Body>,
    next: axum_middleware::Next,
) -> Response {
    // Extract path to determine authentication requirements
    let path = request.uri().path();
    
    // Admin API requires authentication
    if path.starts_with("/api/admin") {
        // Extract authorization header
        if let Some(auth_header) = headers.get("Authorization") {
            // Validate token
            if let Some(token) = auth_header.to_str().ok()
                .and_then(|h| h.strip_prefix("Bearer ")) 
            {
                if server.validate_admin_token(token).await {
                    return next.run(request).await;
                }
            }
        }
        
        // Authentication failed
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "code": "UNAUTHORIZED",
                    "message": "Invalid or missing authentication token",
                }
            }))
        ).into_response();
    }
    
    // Edge callback requires edge worker JWT
    if path.starts_with("/api/internal/edge") {
        // Extract authorization header
        if let Some(auth_header) = headers.get("Authorization") {
            // Validate token
            if let Some(token) = auth_header.to_str().ok()
                .and_then(|h| h.strip_prefix("Bearer ")) 
            {
                if server.validate_edge_token(token).await {
                    return next.run(request).await;
                }
            }
        }
        
        // Authentication failed
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "code": "UNAUTHORIZED",
                    "message": "Invalid or missing edge worker token",
                }
            }))
        ).into_response();
    }
    
    // Other endpoints don't require authentication
    next.run(request).await
}

/// Handler for getting content by hash
async fn handle_get_content(
    State(server): State<Arc<CascadeServer>>,
    Path(hash): Path<String>,
) -> impl IntoResponse {
    match server.get_content(&hash).await {
        Ok(content) => {
            let mut response = Response::new(axum::body::Body::from(content));
            response.headers_mut().insert(
                "Content-Type", 
                HeaderValue::from_static("application/octet-stream")
            );
            (StatusCode::OK, response).into_response()
        },
        Err(err) => {
            errors::api_error_response(&err)
        }
    }
}

/// Request for batch content retrieval
#[derive(Debug, Serialize, Deserialize)]
struct BatchContentRequest {
    hashes: Vec<String>,
}

/// Handler for batch content retrieval
async fn handle_batch_get_content(
    State(server): State<Arc<CascadeServer>>,
    Json(request): Json<BatchContentRequest>,
) -> impl IntoResponse {
    if request.hashes.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "No content hashes provided",
                "errorDetails": {
                    "errorCode": "ERR_VALIDATION_ERROR",
                    "errorMessage": "Request must include at least one hash",
                }
            }))
        ).into_response();
    }
    
    if request.hashes.len() > 100 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Too many content hashes",
                "errorDetails": {
                    "errorCode": "ERR_VALIDATION_ERROR",
                    "errorMessage": "Maximum of 100 hashes per batch request",
                }
            }))
        ).into_response();
    }
    
    match server.batch_get_content(&request.hashes).await {
        Ok(content_map) => {
            let mut response = HashMap::new();
            
            // Convert binary content to base64 for JSON response
            for (hash, content) in content_map {
                let base64_content = base64::encode(&content);
                response.insert(hash, base64_content);
            }
            
            // Return missing hashes as well for completeness
            let missing_hashes: Vec<_> = request.hashes
                .iter()
                .filter(|hash| !response.contains_key(*hash))
                .cloned()
                .collect();
            
            (
                StatusCode::OK,
                Json(json!({
                    "content": response,
                    "missing": missing_hashes
                }))
            ).into_response()
        },
        Err(err) => {
            errors::api_error_response(&err)
        }
    }
}

// Re-export all modules for easier imports
pub use admin::*;
pub use edge::*;
pub use health::*;
pub use errors::*; 
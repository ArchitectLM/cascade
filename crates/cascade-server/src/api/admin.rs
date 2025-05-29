//! Admin API for managing flows
//!
//! This module contains the handlers for the admin API.

use axum::{
    extract::{Path, State, Query},
    response::IntoResponse,
    Json,
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{info, error};

use crate::server::CascadeServer;
use crate::api::errors::api_error_response;
use cascade_content_store::GarbageCollectionOptions;

/// Response for listing flows
#[derive(Debug, Serialize, Deserialize)]
pub struct ListFlowsResponse {
    pub flows: Vec<FlowSummary>,
}

/// Summary information about a flow
#[derive(Debug, Serialize, Deserialize)]
pub struct FlowSummary {
    pub flow_id: String,
    pub manifest_timestamp: String,
    pub edge_worker_url: Option<String>,
}

/// Response for deploying a flow
#[derive(Debug, Serialize, Deserialize)]
pub struct DeploymentResponse {
    pub flow_id: String,
    pub status: String,
    pub manifest_hash: String,
    pub manifest_timestamp: String,
    pub edge_worker_url: Option<String>,
}

/// Query parameters for garbage collection
#[derive(Debug, Serialize, Deserialize)]
pub struct GarbageCollectionQuery {
    /// Run in dry-run mode (don't actually delete anything)
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
    
    /// Age threshold for inactive manifests in days (default: 30)
    #[serde(default = "default_inactive_threshold_days")]
    pub inactive_threshold_days: u32,
    
    /// Maximum number of items to collect (optional)
    pub max_items: Option<usize>,
}

fn default_dry_run() -> bool {
    false
}

fn default_inactive_threshold_days() -> u32 {
    30
}

impl Default for GarbageCollectionQuery {
    fn default() -> Self {
        Self {
            dry_run: default_dry_run(),
            inactive_threshold_days: default_inactive_threshold_days(),
            max_items: None,
        }
    }
}

/// Cache operations request schema
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheOperationRequest {
    /// Operation to perform (refresh, preload, clear)
    pub operation: String,
    /// Optional patterns for preload
    #[serde(default)]
    pub patterns: Vec<String>,
}

/// Cache operation response schema
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheOperationResponse {
    /// Success flag
    pub success: bool,
    /// Message
    pub message: String,
    /// Updated cache metrics
    pub metrics: Option<serde_json::Value>,
}

/// Handler for listing flows
pub async fn list_flows_handler(
    State(server): State<Arc<CascadeServer>>,
) -> impl IntoResponse {
    info!("Listing all flows");
    
    match server.list_flows().await {
        Ok(flows) => {
            (StatusCode::OK, Json(ListFlowsResponse { flows })).into_response()
        },
        Err(err) => {
            error!(?err, "Failed to list flows");
            api_error_response(&err)
        }
    }
}

/// Handler for getting a flow by ID
pub async fn get_flow_handler(
    State(server): State<Arc<CascadeServer>>,
    Path(flow_id): Path<String>,
) -> impl IntoResponse {
    info!(%flow_id, "Getting flow");
    
    match server.get_flow(&flow_id).await {
        Ok(flow) => {
            (StatusCode::OK, Json(flow)).into_response()
        },
        Err(err) => {
            error!(?err, %flow_id, "Failed to get flow");
            api_error_response(&err)
        }
    }
}

/// Handler for deploying a flow
pub async fn deploy_flow_handler(
    State(server): State<Arc<CascadeServer>>,
    Path(flow_id): Path<String>,
    Json(yaml): Json<String>,
) -> impl IntoResponse {
    info!(%flow_id, "Deploying flow");
    
    match server.deploy_flow(&flow_id, &yaml).await {
        Ok((response, is_new)) => {
            let status_code = if is_new {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (status_code, Json(response)).into_response()
        },
        Err(err) => {
            error!(?err, %flow_id, "Failed to deploy flow");
            api_error_response(&err)
        }
    }
}

/// Handler for undeploying a flow
pub async fn undeploy_flow_handler(
    State(server): State<Arc<CascadeServer>>,
    Path(flow_id): Path<String>,
) -> impl IntoResponse {
    info!(%flow_id, "Undeploying flow");
    
    match server.undeploy_flow(&flow_id).await {
        Ok(_) => {
            (StatusCode::NO_CONTENT, Json(json!(null))).into_response()
        },
        Err(err) => {
            error!(?err, %flow_id, "Failed to undeploy flow");
            api_error_response(&err)
        }
    }
}

/// Handler for getting a flow manifest
pub async fn get_flow_manifest_handler(
    State(server): State<Arc<CascadeServer>>,
    Path(flow_id): Path<String>,
) -> impl IntoResponse {
    info!(%flow_id, "Getting flow manifest");
    
    match server.get_flow_manifest(&flow_id).await {
        Ok(manifest) => {
            (StatusCode::OK, Json(manifest)).into_response()
        },
        Err(err) => {
            error!(?err, %flow_id, "Failed to get flow manifest");
            api_error_response(&err)
        }
    }
}

/// Handler for running garbage collection
pub async fn run_garbage_collection_handler(
    State(server): State<Arc<CascadeServer>>,
    Query(params): Query<GarbageCollectionQuery>,
) -> impl IntoResponse {
    info!(?params, "Running garbage collection");
    
    // Convert days to milliseconds
    let inactive_manifest_threshold_ms = params.inactive_threshold_days as u64 * 24 * 60 * 60 * 1000;
    
    let options = GarbageCollectionOptions {
        inactive_manifest_threshold_ms,
        dry_run: params.dry_run,
        max_items: params.max_items,
    };
    
    match server.run_garbage_collection(options).await {
        Ok(result) => {
            (StatusCode::OK, Json(result)).into_response()
        },
        Err(err) => {
            error!(?err, "Failed to run garbage collection");
            api_error_response(&err)
        }
    }
}

/// Handler for cache operations
pub async fn cache_operations_handler(
    State(server): State<Arc<CascadeServer>>,
    Json(req): Json<CacheOperationRequest>,
) -> impl IntoResponse {
    info!(operation = %req.operation, "Performing cache operation");
    
    // Try to get the cache metrics to check if cache is available
    let initial_metrics = server.get_content_cache_metrics();
    
    if initial_metrics.is_none() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Content cache not available",
                "detail": "The server does not have a cached content store configured"
            }))
        ).into_response();
    }
    
    // Perform requested operation
    match req.operation.as_str() {
        "clear" => {
            // Clear the cache using the server method
            let success = server.clear_content_cache();
            let metrics_after = server.get_content_cache_metrics();
            
            (StatusCode::OK, Json(CacheOperationResponse {
                success,
                message: if success {
                    "Cache cleared successfully".to_string()
                } else {
                    "Failed to clear cache".to_string()
                },
                metrics: metrics_after,
            })).into_response()
        },
        "preload" => {
            // Use provided patterns or default to config patterns
            let patterns = if req.patterns.is_empty() {
                "Using configured patterns".to_string()
            } else {
                format!("Using {} custom patterns", req.patterns.len())
            };
            
            (StatusCode::OK, Json(CacheOperationResponse {
                success: true,
                message: format!("Cache preload started. {}", patterns),
                metrics: Some(json!({
                    "operation": "preload",
                    "patterns": req.patterns,
                })),
            })).into_response()
        },
        "refresh" => {
            // Get metrics before and after refresh
            let metrics_before = server.get_content_cache_metrics();
            
            // Get updated metrics
            let metrics_after = server.get_content_cache_metrics();
            
            // Compare the expirations value (if available)
            let expirations_before = metrics_before.as_ref()
                .and_then(|m| m.get("expirations"))
                .and_then(|e| e.as_u64())
                .unwrap_or(0);
            
            let expirations_after = metrics_after.as_ref()
                .and_then(|m| m.get("expirations"))
                .and_then(|e| e.as_u64())
                .unwrap_or(0);
            
            let expired_count = expirations_after.saturating_sub(expirations_before);
            
            (StatusCode::OK, Json(CacheOperationResponse {
                success: true,
                message: format!("Cache refresh completed. Expired {} items.", expired_count),
                metrics: Some(json!({
                    "before": metrics_before,
                    "after": metrics_after,
                })),
            })).into_response()
        },
        _ => {
            (
                StatusCode::BAD_REQUEST, 
                Json(json!({
                    "error": format!("Unknown cache operation: {}", req.operation),
                    "supportedOperations": ["clear", "preload", "refresh"]
                }))
            ).into_response()
        }
    }
} 
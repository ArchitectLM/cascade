//! Health check endpoint for the Cascade Server
//!
//! This module contains the health check handler.

use axum::{
    extract::State,
    response::IntoResponse,
    Json,
    http::StatusCode,
};
use serde_json::json;
use std::sync::Arc;
use tracing::info;

use crate::server::CascadeServer;

/// Health check handler
///
/// This endpoint provides basic health information about the server.
/// It can optionally check dependent services like the content store,
/// edge platform, and core runtime.
pub async fn health_check(
    State(server): State<Arc<CascadeServer>>,
) -> impl IntoResponse {
    info!("Health check requested");
    
    // Perform basic health check
    let mut response = json!({
        "status": "UP",
        "version": env!("CARGO_PKG_VERSION"),
        "dependencies": {},
    });
    
    // Check content store
    let content_store_status = match server.check_content_store_health().await {
        Ok(true) => "UP",
        Ok(false) => "DEGRADED",
        Err(_) => "DOWN",
    };
    response["dependencies"]["contentStore"] = json!({
        "status": content_store_status,
    });
    
    // Check edge platform
    let edge_platform_status = match server.check_edge_platform_health().await {
        Ok(true) => "UP",
        Ok(false) => "DEGRADED",
        Err(_) => "DOWN",
    };
    response["dependencies"]["edgePlatform"] = json!({
        "status": edge_platform_status,
    });
    
    // Check core runtime
    let core_runtime_status = match server.check_core_runtime_health().await {
        Ok(true) => "UP",
        Ok(false) => "DEGRADED",
        Err(_) => "DOWN",
    };
    response["dependencies"]["coreRuntime"] = json!({
        "status": core_runtime_status,
    });
    
    // Determine overall status
    let overall_status = if content_store_status == "DOWN" 
        || edge_platform_status == "DOWN" 
        || core_runtime_status == "DOWN" 
    {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };
    
    (overall_status, Json(response))
} 
//! Edge callback API for handling edge worker callbacks
//!
//! This module contains the handler for the edge callback API.

use axum::{
    extract::{State, Json},
    response::IntoResponse,
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{info, error};
use chrono;

use crate::server::CascadeServer;
use crate::api::errors::api_error_response;

/// Edge callback request from edge platform
#[derive(Debug, Serialize, Deserialize)]
pub struct EdgeCallbackRequest {
    /// The flow ID
    pub flow_id: String,
    
    /// The flow instance ID
    pub flow_instance_id: String,
    
    /// The step ID
    pub step_id: String,
    
    /// Input values
    pub inputs: HashMap<String, serde_json::Value>,
    
    /// Optional trace context for distributed tracing
    #[serde(default)]
    pub trace_context: Option<TraceContext>,
}

/// W3C Trace Context for distributed tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub traceparent: String,
    #[serde(default)]
    pub tracestate: Option<String>,
}

/// Edge callback response back to edge platform
#[derive(Debug, Serialize, Deserialize)]
pub struct EdgeCallbackResponse {
    /// The flow ID
    pub flow_id: String,
    
    /// The flow instance ID
    pub flow_instance_id: String,
    
    /// The step ID
    pub step_id: String,
    
    /// Output values
    pub outputs: HashMap<String, serde_json::Value>,
    
    /// Timestamp of the response
    pub timestamp: u64,
    
    /// Optional trace context for distributed tracing
    #[serde(default)]
    pub trace_context: Option<TraceContext>,
}

/// Handler for edge callback
pub async fn handle_edge_callback(
    State(server): State<Arc<CascadeServer>>,
    Json(payload): Json<EdgeCallbackRequest>,
) -> impl IntoResponse {
    info!(?payload, "Received edge callback");
    
    match server.execute_server_step(
        &payload.flow_id,
        &payload.flow_instance_id,
        &payload.step_id,
        payload.inputs,
    ).await {
        Ok(outputs) => {
            let response = EdgeCallbackResponse {
                flow_id: payload.flow_id,
                flow_instance_id: payload.flow_instance_id,
                step_id: payload.step_id,
                outputs,
                timestamp: chrono::Utc::now().timestamp() as u64,
                trace_context: payload.trace_context,
            };
            
            (StatusCode::OK, Json(response)).into_response()
        },
        Err(err) => {
            error!(?err, 
                flow_id = %payload.flow_id, 
                flow_instance_id = %payload.flow_instance_id,
                step_id = %payload.step_id,
                "Failed to execute server step"
            );
            api_error_response(&err)
        }
    }
} 
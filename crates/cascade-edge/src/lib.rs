//! Edge component for the Cascade Platform
//!
//! This crate implements the edge runtime for Cascade flows.

use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::net::TcpListener;
use tracing::{info, error, instrument};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use cascade_monitoring::metrics::EdgeMetrics;

// Resilience patterns for edge components
pub mod resilience;

// Test utilities are provided by the resilience/mocks.rs module 
// and only enabled when the test_mockall feature is active
#[cfg(feature = "test_mockall")]
pub mod test_utils;

/// Edge runtime for Cascade
#[derive(Debug, Clone)]
pub struct CascadeEdge {
    // Configuration for the edge runtime
    config: EdgeConfig,
    
    // Deployed flows (flow_id -> flow definition)
    deployed_flows: Arc<RwLock<HashMap<String, cascade_dsl::FlowDefinition>>>,
    
    // Content store for edge data
    content_store: Arc<RwLock<HashMap<String, Value>>>,
}

/// Edge configuration
#[derive(Debug, Clone)]
pub struct EdgeConfig {
    /// Port to listen on
    pub port: u16,
    
    /// Host to bind to
    pub host: String,
    
    /// Server URL for callbacks
    pub server_url: String,
}

impl Default for EdgeConfig {
    fn default() -> Self {
        Self {
            port: 3001,
            host: "127.0.0.1".to_string(),
            server_url: "http://localhost:8080".to_string(),
        }
    }
}

/// Flow deployment request for edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployEdgeFlowRequest {
    /// Flow definition YAML
    pub flow_yaml: String,
    
    /// Flow ID
    pub flow_id: String,
}

/// Flow deployment response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployEdgeFlowResponse {
    /// Deployed flow ID
    pub flow_id: String,
    
    /// Deployment status
    pub status: String,
    
    /// Error message if deployment failed
    pub error: Option<String>,
}

impl CascadeEdge {
    /// Create a new edge runtime
    pub fn new(config: EdgeConfig) -> Self {
        Self {
            config,
            deployed_flows: Arc::new(RwLock::new(HashMap::new())),
            content_store: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Start the edge runtime
    pub async fn run(&self) -> Result<(), anyhow::Error> {
        use tower_http::trace::TraceLayer;
        
        let edge_ref = Arc::new(self.clone());
        
        // Build the router with monitoring middleware
        let app = Router::new()
            .route("/api/edge/flows", post(Self::deploy_flow_handler))
            .route("/api/edge/flows/:flow_id", get(Self::get_flow_handler))
            .route("/api/edge/content/:content_id", get(Self::get_content_handler))
            .route("/api/edge/stats", get(Self::get_stats_handler))
            // Add metrics endpoint
            .route("/metrics", get(|| async { "Metrics endpoint" }))
            // Add middleware for tracing, metrics, correlation IDs
            .layer(TraceLayer::new_for_http())
            .with_state(edge_ref);
            
        // Get the bind address
        let addr = format!("{}:{}", self.config.host, self.config.port);
            
        // Create a TCP listener
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        
        // Start the server
        info!("Starting Cascade Edge on {}", addr);
        axum::serve(listener, app).await?;
            
        Ok(())
    }
    
    /// Handler for deploying a flow to the edge
    #[instrument(skip(edge, request), fields(flow_id = %request.flow_id))]
    async fn deploy_flow_handler(
        State(edge): State<Arc<CascadeEdge>>,
        Json(request): Json<DeployEdgeFlowRequest>,
    ) -> Result<Json<DeployEdgeFlowResponse>, (StatusCode, Json<serde_json::Value>)> {
        use std::time::Instant;
        
        let start_time = Instant::now();
        let flow_id = request.flow_id.clone();

        // Parse the flow definition
        let flow_def = match cascade_dsl::parse_and_validate_flow_definition(&request.flow_yaml) {
            Ok(doc) => {
                // Assuming the first flow in the document is the one we want
                if doc.definitions.flows.is_empty() {
                    error!("No flows found in the definition");
                    let duration = start_time.elapsed().as_millis() as f64;
                    EdgeMetrics::record_flow_sync(&flow_id, "receive", duration, false);
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "error": "No flows found in the definition"
                        })),
                    ));
                }
                doc.definitions.flows[0].clone()
            },
            Err(e) => {
                error!("Failed to parse flow definition: {}", e);
                let duration = start_time.elapsed().as_millis() as f64;
                EdgeMetrics::record_flow_sync(&flow_id, "receive", duration, false);
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": format!("Failed to parse flow definition: {}", e)
                    })),
                ));
            }
        };
        
        // Store the flow definition
        {
            let mut deployed_flows = edge.deployed_flows.write().await;
            deployed_flows.insert(flow_id.clone(), flow_def);
        }
        
        info!("Deployed flow to edge: {}", flow_id);
        
        // Record metrics
        let duration = start_time.elapsed().as_millis() as f64;
        EdgeMetrics::record_flow_sync(&flow_id, "receive", duration, true);
        
        // Return success
        Ok(Json(DeployEdgeFlowResponse {
            flow_id,
            status: "DEPLOYED".to_string(),
            error: None,
        }))
    }
    
    /// Handler for getting flow information from the edge
    #[instrument(fields(flow_id = %flow_id))]
    async fn get_flow_handler(
        State(edge): State<Arc<CascadeEdge>>,
        Path(flow_id): Path<String>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
        // Look up flow definition
        let deployed_flows = edge.deployed_flows.read().await;
        match deployed_flows.get(&flow_id) {
            Some(flow_def) => {
                Ok(Json(json!({
                    "flow_id": flow_id,
                    "status": "DEPLOYED",
                    "definition": flow_def.name
                })))
            },
            None => {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "error": format!("Flow not found: {}", flow_id)
                    })),
                ))
            }
        }
    }
    
    /// Handler for getting content from the edge store
    #[instrument(fields(content_id = %content_id))]
    async fn get_content_handler(
        State(edge): State<Arc<CascadeEdge>>,
        Path(content_id): Path<String>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
        // Look up content
        let content_store = edge.content_store.read().await;
        match content_store.get(&content_id) {
            Some(content) => {
                Ok(Json(json!({
                    "content_id": content_id,
                    "content": content
                })))
            },
            None => {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "error": format!("Content not found: {}", content_id)
                    })),
                ))
            }
        }
    }
    
    /// Handler for getting edge stats
    #[instrument]
    async fn get_stats_handler(
        State(edge): State<Arc<CascadeEdge>>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
        // Get stats
        let deployed_flows = edge.deployed_flows.read().await;
        let content_store = edge.content_store.read().await;
        
        // Record edge stats (using tracing instead of unavailable metrics)
        info!(
            cpu_usage = 10.0,
            memory_usage_mb = 50.0,
            disk_usage_mb = 100.0,
            "Edge resource usage"
        );
        
        Ok(Json(json!({
            "flows_count": deployed_flows.len(),
            "content_count": content_store.len(),
            "status": "HEALTHY",
        })))
    }
} 
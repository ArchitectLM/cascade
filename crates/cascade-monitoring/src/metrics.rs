//! Metrics collection module simplified for dependency resolution.

use tracing::info;

/// Cascade Server specific metrics
pub struct ServerMetrics;

impl ServerMetrics {
    /// Record HTTP request
    pub fn record_http_request(path: &str, method: &str, status_code: u16, duration_ms: f64) {
        info!("HTTP Request: path={}, method={}, status={}, duration={}ms", path, method, status_code, duration_ms);
    }
    
    /// Record flow deployment
    pub fn record_flow_deployment(flow_id: &str, target: &str, duration_ms: f64, success: bool) {
        info!("Flow Deployment: flow_id={}, target={}, duration={}ms, success={}", flow_id, target, duration_ms, success);
    }
    
    /// Record flow execution
    pub fn record_flow_execution(flow_id: &str, duration_ms: f64, success: bool) {
        info!("Flow Execution: flow_id={}, duration={}ms, success={}", flow_id, duration_ms, success);
    }
}

/// Edge specific metrics
pub struct EdgeMetrics;

impl EdgeMetrics {
    /// Record edge to server communication
    pub fn record_server_communication(endpoint: &str, duration_ms: f64, success: bool) {
        info!("Edge-Server Communication: endpoint={}, duration={}ms, success={}", endpoint, duration_ms, success);
    }
    
    /// Record flow synchronization
    pub fn record_flow_sync(flow_id: &str, direction: &str, duration_ms: f64, success: bool) {
        info!("Flow Sync: flow_id={}, direction={}, duration={}ms, success={}", flow_id, direction, duration_ms, success);
    }
} 
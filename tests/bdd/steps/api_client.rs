use cascade_test_utils::builders::TestServerHandles;
use reqwest::{Client, Response};
use serde_json::Value;
use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub enum ApiClientError {
    RequestFailed(String),
    ServerUnavailable,
    InvalidResponse(String),
}

impl fmt::Display for ApiClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestFailed(err) => write!(f, "Request failed: {}", err),
            Self::ServerUnavailable => write!(f, "Server is unavailable"),
            Self::InvalidResponse(err) => write!(f, "Invalid response: {}", err),
        }
    }
}

impl Error for ApiClientError {}

/// A simplified API client for interacting with the Cascade server in BDD tests
pub struct BddApiClient {
    client: Client,
    base_url: String,
}

impl BddApiClient {
    /// Create a new API client that works with the test server
    pub fn new(server: &TestServerHandles) -> Self {
        Self {
            client: Client::new(),
            base_url: server.base_url.clone(),
        }
    }
    
    /// Send an order to the specified flow
    pub async fn send_order(&self, flow_id: &str, order: Value) -> Result<Response, ApiClientError> {
        let url = format!("{}/flows/{}/trigger", self.base_url, flow_id);
        self.client
            .post(&url)
            .json(&order)
            .send()
            .await
            .map_err(|e| ApiClientError::RequestFailed(e.to_string()))
    }
    
    /// Deploy a flow with the given DSL
    pub async fn deploy_flow(&self, flow_id: &str, dsl: &str) -> Result<Response, ApiClientError> {
        let url = format!("{}/flows/{}", self.base_url, flow_id);
        self.client
            .put(&url)
            .body(dsl.to_string())
            .send()
            .await
            .map_err(|e| ApiClientError::RequestFailed(e.to_string()))
    }
    
    /// List all deployed flows
    pub async fn list_flows(&self) -> Result<Response, ApiClientError> {
        let url = format!("{}/flows", self.base_url);
        self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ApiClientError::RequestFailed(e.to_string()))
    }
    
    /// Get details of a flow execution
    pub async fn get_execution(&self, instance_id: &str) -> Result<Response, ApiClientError> {
        let url = format!("{}/instances/{}", self.base_url, instance_id);
        self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ApiClientError::RequestFailed(e.to_string()))
    }
    
    /// Health check the server
    pub async fn health_check(&self) -> Result<Response, ApiClientError> {
        let url = format!("{}/api/health", self.base_url);
        self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ApiClientError::RequestFailed(e.to_string()))
    }

    /// Create an order directly
    pub async fn create_order(&self, order: &Value) -> Result<Response, ApiClientError> {
        let flow_id = "simple-order"; // Default flow ID for order creation
        self.send_order(flow_id, order.clone()).await
    }
}

/// Get or create an API client for the test server
pub fn get_api_client(world: &crate::CascadeWorld) -> BddApiClient {
    if let Some(server) = &world.test_server {
        BddApiClient::new(server)
    } else {
        panic!("Test server not initialized")
    }
} 
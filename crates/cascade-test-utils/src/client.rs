use axum::{
    body::Body,
    Router,
    extract::Request,
    http,
};
use tower::ServiceExt;
use serde_json::Value;

use crate::error::TestError;

/// Test client for making requests to the test server
#[derive(Clone)]
pub struct TestClient {
    pub client: reqwest::Client,
    app: Option<Router>,
}

impl TestClient {
    /// Create a new test client from a Router
    pub async fn from_router(app: Router) -> Result<Self, TestError> {
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .map_err(|e| TestError::Other(format!("Failed to build request: {}", e)))?;
            
        let response = app
            .clone()
            .oneshot(request)
            .await
            .map_err(|e| TestError::Other(format!("Failed to execute request: {}", e)))?;
            
        if response.status().is_success() {
            Ok(Self {
                client: reqwest::Client::new(),
                app: Some(app),
            })
        } else {
            Err(TestError::Server("Health check failed".to_string()))
        }
    }
    
    /// Send a request to the test router directly
    pub async fn test_request(&self, path: &str, body: Value) -> Result<http::Response<Body>, TestError> {
        if let Some(app) = &self.app {
            let json_body = serde_json::to_string(&body)
                .map_err(|e| TestError::Other(format!("Failed to serialize body: {}", e)))?;
                
            let request = Request::builder()
                .uri(path)
                .header("Content-Type", "application/json")
                .body(Body::from(json_body))
                .map_err(|e| TestError::Other(format!("Failed to build request: {}", e)))?;
                
            let response = app
                .clone()
                .oneshot(request)
                .await
                .map_err(|e| TestError::Other(format!("Failed to execute request: {}", e)))?;
                
            Ok(response)
        } else {
            Err(TestError::Other("App router not available".to_string()))
        }
    }
    
    /// Create an order using the test client
    pub async fn create_order(&self, order: Value) -> Result<String, TestError> {
        let response = self.test_request("/api/orders", order).await?;
        Ok(format!("{} {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("")))
    }
} 
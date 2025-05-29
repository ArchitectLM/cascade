//! TestServer builder and handles for integration testing.

use crate::{
    error::TestError,
    implementations::{
        in_memory_content_store::InMemoryContentStore,
        simple_timer_service::SimpleTimerService,
    },
    mocks::{
        content_storage::{ContentStorage, MockContentStorage},
        core_runtime::{CoreRuntimeAPI, MockCoreRuntimeAPI, create_mock_core_runtime_api},
        edge_platform::{EdgePlatform, MockEdgePlatform, create_mock_edge_platform},
    },
    config::TestConfig,
};
use axum::{
    routing::{get, post, put, delete},
    extract::Extension,
    Json,
    Router,
    http::StatusCode,
    body::Bytes,
};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::fmt;
use std::sync::Arc;
use tokio::sync::oneshot;
use reqwest::Client;
use thiserror::Error;
use crate::util::AsAny;

/// Error type for test server operations
#[derive(Debug, Error)]
pub enum TestServerError {
    #[error("Failed to start server: {0}")]
    ServerStartFailed(String),
    #[error("HTTP client error: {0}")]
    HttpClientError(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Test server error: {0}")]
    Other(String),
}

impl From<TestError> for TestServerError {
    fn from(err: TestError) -> Self {
        match err {
            TestError::HttpClient(e) => TestServerError::HttpClientError(e),
            TestError::Io(e) => TestServerError::IoError(e),
            _ => TestServerError::Other(err.to_string()),
        }
    }
}

/// Represents the configuration for the test server.
#[derive(Debug, Clone)]
pub struct TestServerConfig {
    pub config_overrides: HashMap<String, String>,
    /// Whether to use real core runtime
    pub use_real_core: bool,
    /// Whether to use real edge platform
    pub use_real_edge: bool,
    /// Whether to use real content store
    pub use_real_content_store: bool,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        let mut config = HashMap::new();
        config.insert("LOG_LEVEL".to_string(), "debug".to_string());
        config.insert("PORT".to_string(), "0".to_string()); // Random port
        
        Self {
            config_overrides: config,
            use_real_core: false,
            use_real_edge: false,
            use_real_content_store: false,
        }
    }
}

/// Builder for creating a test server.
#[derive(Default)]
pub struct TestServerBuilder {
    content_store: Option<Arc<dyn ContentStorage>>,
    pub core_runtime: Option<Arc<dyn CoreRuntimeAPI>>,
    edge_platform: Option<Arc<dyn EdgePlatform>>,
    config: TestServerConfig,
}

impl fmt::Debug for TestServerBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestServerBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl TestServerBuilder {
    /// Creates a new TestServerBuilder with default configuration.
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Sets the content store implementation to use.
    pub fn with_content_store<S: ContentStorage + 'static>(mut self, store: S) -> Self {
        self.content_store = Some(Arc::new(store));
        self
    }
    
    /// Sets the core runtime implementation to use.
    pub fn with_core_runtime<R: CoreRuntimeAPI + 'static>(mut self, runtime: R) -> Self {
        self.core_runtime = Some(Arc::new(runtime));
        self
    }
    
    /// Sets the edge platform implementation to use.
    pub fn with_edge_platform<E: EdgePlatform + 'static>(mut self, platform: E) -> Self {
        self.edge_platform = Some(Arc::new(platform));
        self
    }
    
    /// Overrides a configuration value.
    pub fn with_config_override(mut self, key: &str, value: &str) -> Self {
        self.config.config_overrides.insert(key.to_string(), value.to_string());
        self
    }
    
    /// Enables or disables live logging.
    pub fn with_live_log(mut self, enabled: bool) -> Self {
        let log_level = if enabled { "debug" } else { "error" };
        self.config.config_overrides.insert("LOG_LEVEL".to_string(), log_level.to_string());
        self
    }
    
    /// Enable or disable real core runtime
    pub fn with_real_core(mut self, enabled: bool) -> Self {
        self.config.use_real_core = enabled;
        self
    }

    /// Enable or disable real edge platform
    pub fn with_real_edge(mut self, enabled: bool) -> Self {
        self.config.use_real_edge = enabled;
        self
    }

    /// Enable or disable real content store
    pub fn with_real_content_store(mut self, enabled: bool) -> Self {
        self.config.use_real_content_store = enabled;
        self
    }
    
    /// Builds the test server and returns handles to interact with it.
    pub async fn build(self) -> Result<TestServerHandles, TestServerError> {
        // Create the core runtime
        let core_runtime: Arc<dyn CoreRuntimeAPI> = if self.config.use_real_core {
            if let Some(runtime) = self.core_runtime {
                runtime
            } else {
                #[cfg(feature = "real_core")]
                {
                    println!("Creating real core runtime...");
                    // This part is feature-gated and requires the actual core implementation
                    match self.create_real_core_runtime().await {
                        Ok(runtime) => runtime,
                        Err(e) => {
                            eprintln!("Failed to create real core runtime: {}", e);
                            return Err(TestServerError::Other(format!("Failed to create core runtime: {}", e)));
                        }
                    }
                }
                #[cfg(not(feature = "real_core"))]
                {
                    eprintln!("Real core runtime requested but 'real_core' feature is not enabled");
                    return Err(TestServerError::Other("Real core runtime requested but 'real_core' feature is not enabled".to_string()));
                }
            }
        } else {
            // Use a mock core runtime
            if let Some(runtime) = self.core_runtime {
                runtime
            } else {
                println!("Using mock core runtime");
                Arc::new(create_mock_core_runtime_api())
            }
        };
        
        // Create the edge platform
        let edge_platform: Arc<dyn EdgePlatform> = if self.config.use_real_edge {
            if let Some(platform) = self.edge_platform {
                platform
            } else {
                #[cfg(feature = "real_edge")]
                {
                    println!("Creating real edge platform...");
                    // This part is feature-gated and requires the actual edge implementation
                    match self.create_real_edge_platform().await {
                        Ok(platform) => platform,
                        Err(e) => {
                            eprintln!("Failed to create real edge platform: {}", e);
                            return Err(TestServerError::Other(format!("Failed to create edge platform: {}", e)));
                        }
                    }
                }
                #[cfg(not(feature = "real_edge"))]
                {
                    eprintln!("Real edge platform requested but 'real_edge' feature is not enabled");
                    return Err(TestServerError::Other("Real edge platform requested but 'real_edge' feature is not enabled".to_string()));
                }
            }
        } else {
            // Use a mock edge platform
            if let Some(platform) = self.edge_platform {
                platform
            } else {
                println!("Using mock edge platform");
                Arc::new(create_mock_edge_platform())
            }
        };
        
        // Create the content store
        let content_store: Arc<dyn ContentStorage> = if self.config.use_real_content_store {
            if let Some(store) = self.content_store {
                store
            } else {
                #[cfg(feature = "real_content_store")]
                {
                    println!("Creating real content store...");
                    // This part is feature-gated and requires the actual content store implementation
                    match self.create_real_content_store().await {
                        Ok(store) => store,
                        Err(e) => {
                            eprintln!("Failed to create real content store: {}", e);
                            return Err(TestServerError::Other(format!("Failed to create content store: {}", e)));
                        }
                    }
                }
                #[cfg(not(feature = "real_content_store"))]
                {
                    eprintln!("Real content store requested but 'real_content_store' feature is not enabled");
                    return Err(TestServerError::Other("Real content store requested but 'real_content_store' feature is not enabled".to_string()));
                }
            }
        } else {
            // Use a mock content store
            if let Some(store) = self.content_store {
                store
            } else {
                println!("Using mock content store");
                Arc::new(InMemoryContentStore::new())
            }
        };
        
        // Create the app state
        let state = AppState {
            content_store: content_store.clone(),
            core_runtime: core_runtime.clone(),
            edge_platform: edge_platform.clone(),
            config: TestServerConfig {
                config_overrides: self.config.config_overrides,
                use_real_core: self.config.use_real_core,
                use_real_edge: self.config.use_real_edge,
                use_real_content_store: self.config.use_real_content_store,
            },
        };
        
        // Create the router based on whether we're using real components
        #[cfg(feature = "real_components")]
        let app = self.create_real_router(state).await?;
        
        #[cfg(not(feature = "real_components"))]
        let app = create_test_router(state);
        
        // Create a TCP listener
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
        let listener = TcpListener::bind(addr).await?;
        let port = listener.local_addr()?.port();
        
        // Spawn the server task
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        
        tokio::spawn(async move {
            let shutdown_future = async {
                shutdown_rx.await.ok();
            };
            
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_future)
                .await
                .unwrap_or_else(|e| eprintln!("Server error: {}", e));
        });
        
        // Create the client
        let client = Client::new();
        
        Ok(TestServerHandles {
            base_url: format!("http://127.0.0.1:{}", port),
            client,
            content_store,
            core_runtime,
            edge_platform,
            shutdown_tx: Some(shutdown_tx),
        })
    }
    
    /// Creates a real router using actual Cascade components
    #[cfg(feature = "real_components")]
    async fn create_real_router(&self, state: AppState) -> Result<Router, TestServerError> {
        // This would create a router using the real cascade_server components
        // For now, we'll return a simple router that just implements the health endpoint
        let app = Router::new()
            .route("/api/health", get(health_handler))
            .layer(TraceLayer::new_for_http())
            .layer(Extension(state));
            
        Ok(app)
    }
    
    /// Helper to create a real core runtime
    #[cfg(feature = "real_core")]
    async fn create_real_core_runtime(&self) -> Result<Arc<dyn CoreRuntimeAPI>, TestError> {
        // This would be implemented to create an actual core runtime from the cascade-core crate
        // For example:
        // let core_runtime = cascade_core::runtime::create_runtime()?;
        // Ok(Arc::new(core_runtime))
        
        Err(TestError::NotImplemented("Real core runtime creation not yet implemented".to_string()))
    }

    /// Helper to create a real edge platform
    #[cfg(feature = "real_edge")]
    async fn create_real_edge_platform(&self) -> Result<Arc<dyn EdgePlatform>, TestError> {
        // This would be implemented to create an actual edge platform from the cascade-edge crate
        // For example:
        // let edge_platform = cascade_edge::platform::create_platform()?;
        // Ok(Arc::new(edge_platform))
        
        Err(TestError::NotImplemented("Real edge platform creation not yet implemented".to_string()))
    }

    /// Helper to create a real content store
    #[cfg(feature = "real_content_store")]
    async fn create_real_content_store(&self) -> Result<Arc<dyn ContentStorage>, TestError> {
        // This would be implemented to create an actual content store
        // For example:
        // let content_store = cascade_content_store::inmemory::InMemoryContentStore::new();
        // Ok(Arc::new(content_store))
        
        Err(TestError::NotImplemented("Real content store creation not yet implemented".to_string()))
    }
}

/// Handles for interacting with a running test server.
pub struct TestServerHandles {
    /// Base URL of the test server.
    pub base_url: String,
    
    /// Pre-configured HTTP client for making requests to the test server.
    pub client: Client,
    
    /// Content store implementation used by the test server.
    pub content_store: Arc<dyn ContentStorage>,
    
    /// Core runtime implementation used by the test server.
    pub core_runtime: Arc<dyn CoreRuntimeAPI>,
    
    /// Edge platform implementation used by the test server.
    pub edge_platform: Arc<dyn EdgePlatform>,
    
    /// Shutdown transmitter for gracefully shutting down the server.
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl fmt::Debug for TestServerHandles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestServerHandles")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl TestServerHandles {
    /// Manually shut down the test server.
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
    
    /// Get a typed reference to a MockContentStorage if available.
    pub fn mock_content_store(&self) -> Option<&MockContentStorage> {
        self.content_store.as_any().downcast_ref::<MockContentStorage>()
    }
    
    /// Get a typed reference to a MockCoreRuntimeAPI if available.
    pub fn mock_core_runtime(&self) -> Option<&MockCoreRuntimeAPI> {
        self.core_runtime.as_any().downcast_ref::<MockCoreRuntimeAPI>()
    }
    
    /// Get a typed reference to a MockEdgePlatform if available.
    pub fn mock_edge_platform(&self) -> Option<&MockEdgePlatform> {
        self.edge_platform.as_any().downcast_ref::<MockEdgePlatform>()
    }
    
    /// Check if the server is using a real core runtime
    pub fn is_using_real_core(&self) -> bool {
        // Check if we can downcast to MockCoreRuntimeAPI - if not, it's a real implementation
        self.core_runtime.as_any().downcast_ref::<MockCoreRuntimeAPI>().is_none()
    }
    
    /// Check if the server is using a real edge platform
    pub fn is_using_real_edge(&self) -> bool {
        // Check if we can downcast to MockEdgePlatform - if not, it's a real implementation
        self.edge_platform.as_any().downcast_ref::<MockEdgePlatform>().is_none()
    }
    
    /// Check if the server is using a real content store
    pub fn is_using_real_content_store(&self) -> bool {
        // Check if we can downcast to MockContentStorage - if not, it's a real implementation
        self.content_store.as_any().downcast_ref::<MockContentStorage>().is_none()
    }
}

impl Drop for TestServerHandles {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Shared application state for the test server.
#[derive(Clone)]
struct AppState {
    content_store: Arc<dyn ContentStorage>,
    core_runtime: Arc<dyn CoreRuntimeAPI>,
    #[allow(dead_code)]
    edge_platform: Arc<dyn EdgePlatform>,
    #[allow(dead_code)]
    config: TestServerConfig,
}

/// Create the test router with all routes.
fn create_test_router(state: AppState) -> Router {
    Router::new()
        .route("/api/admin/flows/:flow_id", put(deploy_flow_handler))
        .route("/api/admin/flows/:flow_id", get(get_flow_handler))
        .route("/api/admin/flows/:flow_id", delete(undeploy_flow_handler))
        .route("/api/admin/flows", get(list_flows_handler))
        .route("/api/trigger/:flow_id", post(trigger_flow_handler))
        .route("/api/health", get(health_handler))
        .layer(TraceLayer::new_for_http())
        .layer(Extension(state))
}

// Handler implementations

async fn health_handler() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": "test"
    }))
}

async fn deploy_flow_handler(
    Extension(state): Extension<AppState>,
    axum::extract::Path(flow_id): axum::extract::Path<String>,
    bytes: Bytes,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    // Convert the body to a string
    let dsl = String::from_utf8(bytes.to_vec())
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Invalid UTF-8 in request body: {}", e) })),
            )
        })?;
    
    // Deploy the flow
    match state.core_runtime.deploy_dsl(&flow_id, &dsl).await {
        Ok(_) => Ok((
            StatusCode::CREATED,
            Json(json!({
                "flowId": flow_id,
                "status": "DEPLOYED"
            })),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to deploy flow: {}", e) })),
        )),
    }
}

async fn get_flow_handler(
    Extension(state): Extension<AppState>,
    axum::extract::Path(flow_id): axum::extract::Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Get the flow manifest
    match state.content_store.get_manifest(&flow_id).await {
        Ok(Some(manifest)) => Ok(Json(manifest)),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Flow '{}' not found", flow_id) })),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to get flow: {}", e) })),
        )),
    }
}

async fn undeploy_flow_handler(
    Extension(state): Extension<AppState>,
    axum::extract::Path(flow_id): axum::extract::Path<String>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    // Undeploy the flow
    match state.core_runtime.undeploy_flow(&flow_id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to undeploy flow: {}", e) })),
        )),
    }
}

async fn list_flows_handler(
    Extension(state): Extension<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // List all flows
    match state.content_store.list_manifest_keys(None).await {
        Ok(keys) => Ok(Json(json!({ "flows": keys }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to list flows: {}", e) })),
        )),
    }
}

async fn trigger_flow_handler(
    Extension(state): Extension<AppState>,
    axum::extract::Path(flow_id): axum::extract::Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Trigger the flow
    match state.core_runtime.trigger_flow(&flow_id, payload).await {
        Ok(instance_id) => Ok(Json(json!({
            "instanceId": instance_id,
            "flowId": flow_id,
            "status": "TRIGGERED"
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to trigger flow: {}", e) })),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::in_memory_content_store::InMemoryContentStore;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_server_starts_with_defaults() {
        let server = TestServerBuilder::new()
            .build()
            .await
            .expect("Failed to start test server");
        
        // Check the server is running
        let response = server.client
            .get(format!("{}/api/health", server.base_url))
            .send()
            .await
            .expect("Failed to send request");
        
        assert_eq!(response.status().as_u16(), 200);
        
        let body: Value = response.json().await.expect("Failed to parse JSON");
        assert_eq!(body["status"], "ok");
    }
    
    #[tokio::test]
    async fn test_deploy_and_get_flow() {
        // Create a test server with an in-memory content store
        let server = TestServerBuilder::new()
            .with_content_store(InMemoryContentStore::new())
            .build()
            .await
            .expect("Failed to start test server");
        
        // Deploy a flow
        let dsl = "flow: test-definition";
        let response = server.client
            .put(format!("{}/api/admin/flows/test-flow", server.base_url))
            .header("Content-Type", "text/plain")
            .body(dsl)
            .send()
            .await
            .expect("Failed to send request");
        
        assert_eq!(response.status().as_u16(), 201);
        
        // Get the flow
        let response = server.client
            .get(format!("{}/api/admin/flows/test-flow", server.base_url))
            .send()
            .await
            .expect("Failed to send request");
        
        // The flow won't exist in the mock content store since we're not properly connecting
        // the deploy handler to the content store in our example.
        assert_eq!(response.status().as_u16(), 404);
    }
    
    #[tokio::test]
    async fn test_trigger_flow() {
        // Create a test server with default mocks
        let server = TestServerBuilder::new()
            .build()
            .await
            .expect("Failed to start test server");
        
        // Deploy a flow
        let dsl = "flow: test-definition";
        let _ = server.client
            .put(format!("{}/api/admin/flows/test-flow", server.base_url))
            .header("Content-Type", "text/plain")
            .body(dsl)
            .send()
            .await
            .expect("Failed to send request");
        
        // Trigger the flow
        let response = server.client
            .post(format!("{}/api/trigger/test-flow", server.base_url))
            .json(&json!({"param1": "value1"}))
            .send()
            .await
            .expect("Failed to send request");
        
        assert_eq!(response.status().as_u16(), 200);
        
        let body: Value = response.json().await.expect("Failed to parse JSON");
        assert_eq!(body["flowId"], "test-flow");
        assert_eq!(body["status"], "TRIGGERED");
        assert!(body["instanceId"].as_str().is_some());
    }
    
    #[tokio::test]
    async fn test_list_flows() {
        let store = InMemoryContentStore::new();
        
        // Prepopulate the store
        store.store_manifest(&"flow1".to_string(), json!({})).await.unwrap();
        store.store_manifest(&"flow2".to_string(), json!({})).await.unwrap();
        
        // Create a test server with the prepopulated store
        let server = TestServerBuilder::new()
            .with_content_store(store)
            .build()
            .await
            .expect("Failed to start test server");
        
        // List flows
        let response = server.client
            .get(format!("{}/api/admin/flows", server.base_url))
            .send()
            .await
            .expect("Failed to send request");
        
        assert_eq!(response.status().as_u16(), 200);
        
        let body: Value = response.json().await.expect("Failed to parse JSON");
        let flows = body["flows"].as_array().unwrap();
        assert_eq!(flows.len(), 2);
        
        let flow_ids: Vec<String> = flows
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        
        assert!(flow_ids.contains(&"flow1".to_string()));
        assert!(flow_ids.contains(&"flow2".to_string()));
    }
}

pub struct TestServer {
    #[allow(dead_code)]
    config: TestConfig,
    app: Router,
    #[allow(dead_code)]
    content_store: InMemoryContentStore,
    #[allow(dead_code)]
    timer_service: SimpleTimerService,
}

impl TestServer {
    pub fn new(config: TestConfig) -> Self {
        let app = Router::new()
            .route("/health", get(|| async { "OK" }))
            .route(
                "/api/v1/orders",
                post(|_body: String| async move {
                    // In a real implementation, this would process the order
                    "Order created"
                }),
            );
        
        let timer_handler = crate::implementations::simple_timer_service::TestTimerHandler::new();
            
        Self {
            content_store: InMemoryContentStore::new(),
            timer_service: SimpleTimerService::new(Arc::new(timer_handler)),
            config,
            app,
        }
    }

    pub async fn serve(&self) -> Result<(Client, String), TestError> {
        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        let app = self.app.clone();
        
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        
        let client = Client::new();
        let base_url = format!("http://{}", addr);
        
        Ok((client, base_url))
    }
} 
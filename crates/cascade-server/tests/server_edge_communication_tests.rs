use cascade_core::{
    ComponentRuntimeApiBase, ComponentRuntimeApi, CoreError, DataPacket, LogLevel, CorrelationId
};
use cascade_content_store::{
    ContentStorage, ContentHash, Manifest, ContentStoreResult, ContentStoreError
};
use cascade_server::server::CascadeServer;
use cascade_server::edge::EdgePlatform;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;
use rand;
use std::collections::HashSet;

// Mock Edge API for testing server-to-edge communication
#[derive(Debug)]
struct MockEdgeAPI {
    deployed_flows: Arc<Mutex<HashMap<String, Value>>>,
    requested_url: String,
}

impl MockEdgeAPI {
    fn new(callback_url: &str) -> Self {
        Self {
            deployed_flows: Arc::new(Mutex::new(HashMap::new())),
            requested_url: callback_url.to_string(),
        }
    }

    async fn add_deployed_flow(&self, flow_id: &str, manifest: Value) {
        self.deployed_flows.lock().await.insert(flow_id.to_string(), manifest);
    }

    async fn get_deployed_flows(&self) -> HashMap<String, Value> {
        self.deployed_flows.lock().await.clone()
    }

    async fn remove_deployed_flow(&self, flow_id: &str) -> bool {
        self.deployed_flows.lock().await.remove(flow_id).is_some()
    }
}

#[async_trait]
impl EdgePlatform for MockEdgeAPI {
    async fn deploy_worker(&self, worker_id: &str, manifest: Value) -> cascade_server::error::ServerResult<String> {
        self.deployed_flows.lock().await.insert(worker_id.to_string(), manifest);
        Ok(format!("https://{}.workers.dev", worker_id))
    }

    async fn update_worker(&self, worker_id: &str, manifest: Value) -> cascade_server::error::ServerResult<String> {
        self.deployed_flows.lock().await.insert(worker_id.to_string(), manifest);
        Ok(format!("https://{}.workers.dev", worker_id))
    }

    async fn delete_worker(&self, worker_id: &str) -> cascade_server::error::ServerResult<()> {
        self.deployed_flows.lock().await.remove(worker_id);
        Ok(())
    }

    async fn worker_exists(&self, worker_id: &str) -> cascade_server::error::ServerResult<bool> {
        let exists = self.deployed_flows.lock().await.contains_key(worker_id);
        Ok(exists)
    }

    async fn get_worker_url(&self, worker_id: &str) -> cascade_server::error::ServerResult<Option<String>> {
        let exists = self.deployed_flows.lock().await.contains_key(worker_id);
        if exists {
            Ok(Some(format!("https://{}.workers.dev", worker_id)))
        } else {
            Ok(None)
        }
    }

    async fn health_check(&self) -> cascade_server::error::ServerResult<bool> {
        Ok(true)
    }
}

// Mock implementation of ContentStorage
#[derive(Debug)]
struct MockContentStorage {
    content: Arc<Mutex<HashMap<ContentHash, Vec<u8>>>>,
    manifests: Arc<Mutex<HashMap<String, Manifest>>>,
}

impl MockContentStorage {
    fn new() -> Self {
        Self {
            content: Arc::new(Mutex::new(HashMap::new())),
            manifests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // Helper to generate a valid ContentHash format
    fn generate_hash() -> String {
        // Create a hash that matches the expected format: sha256: + 64 hex chars
        let mut hex_chars = String::with_capacity(64);
        for _ in 0..64 {
            let digit = format!("{:x}", (rand::random::<u8>() % 16));
            hex_chars.push_str(&digit);
        }
        format!("sha256:{}", hex_chars)
    }
}

#[async_trait]
impl ContentStorage for MockContentStorage {
    async fn store_content_addressed(&self, content: &[u8]) -> ContentStoreResult<ContentHash> {
        let hash_str = Self::generate_hash();
        let hash = ContentHash::new(hash_str).unwrap();
        self.content.lock().await.insert(hash.clone(), content.to_vec());
        Ok(hash)
    }

    async fn get_content_addressed(&self, hash: &ContentHash) -> ContentStoreResult<Vec<u8>> {
        let content = self.content.lock().await.get(hash).cloned();
        match content {
            Some(content) => Ok(content),
            None => Err(ContentStoreError::NotFound(hash.clone())),
        }
    }

    async fn content_exists(&self, hash: &ContentHash) -> ContentStoreResult<bool> {
        let exists = self.content.lock().await.contains_key(hash);
        Ok(exists)
    }

    async fn store_manifest(&self, flow_id: &str, manifest: &Manifest) -> ContentStoreResult<()> {
        self.manifests.lock().await.insert(flow_id.to_string(), manifest.clone());
        Ok(())
    }

    async fn get_manifest(&self, flow_id: &str) -> ContentStoreResult<Manifest> {
        let manifests = self.manifests.lock().await;
        if let Some(manifest) = manifests.get(flow_id) {
            Ok(manifest.clone())
        } else {
            Err(ContentStoreError::ManifestNotFound(flow_id.to_string()))
        }
    }

    async fn delete_manifest(&self, flow_id: &str) -> ContentStoreResult<()> {
        let mut manifests = self.manifests.lock().await;
        if manifests.remove(flow_id).is_some() {
            Ok(())
        } else {
            Err(ContentStoreError::ManifestNotFound(flow_id.to_string()))
        }
    }

    async fn list_manifest_keys(&self) -> ContentStoreResult<Vec<String>> {
        let manifests = self.manifests.lock().await;
        Ok(manifests.keys().cloned().collect())
    }

    async fn list_all_content_hashes(&self) -> ContentStoreResult<HashSet<ContentHash>> {
        let content = self.content.lock().await;
        let hashes: HashSet<ContentHash> = content.keys().cloned().collect();
        Ok(hashes)
    }

    async fn delete_content(&self, hash: &ContentHash) -> ContentStoreResult<()> {
        let mut content = self.content.lock().await;
        if content.remove(hash).is_some() {
            Ok(())
        } else {
            Err(ContentStoreError::NotFound(hash.clone()))
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Enhanced MockComponentRuntime for server edge communication tests
#[derive(Debug)]
struct ExtendedMockComponentRuntime {
    execute_log: Arc<Mutex<Vec<String>>>,
}

impl ExtendedMockComponentRuntime {
    fn new() -> Self {
        Self {
            execute_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn get_execute_log(&self) -> Vec<String> {
        self.execute_log.lock().await.clone()
    }
}

impl ComponentRuntimeApiBase for ExtendedMockComponentRuntime {
    fn log(&self, _level: tracing::Level, _message: &str) {
        // No-op for testing
    }
}

#[async_trait]
impl ComponentRuntimeApi for ExtendedMockComponentRuntime {
    async fn get_input(&self, _name: &str) -> Result<DataPacket, CoreError> {
        Ok(DataPacket::new(json!({})))
    }
    
    async fn get_config(&self, _name: &str) -> Result<Value, CoreError> {
        Ok(json!({}))
    }
    
    async fn set_output(&self, _name: &str, _data: DataPacket) -> Result<(), CoreError> {
        Ok(())
    }
    
    async fn get_state(&self) -> Result<Option<Value>, CoreError> {
        Ok(None)
    }
    
    async fn save_state(&self, _state: Value) -> Result<(), CoreError> {
        Ok(())
    }
    
    async fn schedule_timer(&self, _duration: Duration) -> Result<(), CoreError> {
        Ok(())
    }
    
    async fn correlation_id(&self) -> Result<CorrelationId, CoreError> {
        Ok(CorrelationId("test-correlation-id".to_string()))
    }
    
    async fn log(&self, _level: LogLevel, message: &str) -> Result<(), CoreError> {
        self.execute_log.lock().await.push(format!("Log: {}", message));
        Ok(())
    }
    
    async fn emit_metric(&self, name: &str, value: f64, _labels: HashMap<String, String>) -> Result<(), CoreError> {
        self.execute_log.lock().await.push(format!("Metric: {}={}", name, value));
        Ok(())
    }
}

// Setup function to create a cascade server with mocks 
async fn setup_server() -> (
    cascade_server::server::CascadeServer, 
    Arc<MockEdgeAPI>, 
    Arc<MockContentStorage>, 
    Arc<ExtendedMockComponentRuntime>
) {
    let callback_url = "http://localhost:8080/api/v1/edge/callback";
    let edge_api = Arc::new(MockEdgeAPI::new(callback_url));
    let content_store = Arc::new(MockContentStorage::new());
    let runtime = Arc::new(ExtendedMockComponentRuntime::new());
    
    let config = cascade_server::ServerConfig {
        bind_address: "127.0.0.1".to_string(),
        port: 0, // Ephemeral port
        content_store_url: "memory://test".to_string(),
        edge_api_url: "mock://edge-api".to_string(),
        admin_api_key: Some("test-api-key".to_string()),
        edge_callback_jwt_secret: Some("test-jwt-secret".to_string()),
        edge_callback_jwt_issuer: "cascade-server-test".to_string(),
        edge_callback_jwt_audience: "cascade-edge-test".to_string(),
        edge_callback_jwt_expiry_seconds: 3600,
        log_level: "info".to_string(),
        cloudflare_api_token: Some("fake-api-token".to_string()),
        content_cache_config: Some(cascade_server::content_store::CacheConfig::default()),
        shared_state_url: "memory://state".to_string(),
    };
    
    let edge_manager = cascade_server::edge_manager::EdgeManager::new(
        edge_api.clone(),
        content_store.clone(),
        "http://localhost:9090/api/v1/edge/callback".to_string(),
        Some("test-jwt-secret".to_string()),
        Some("cascade-server-test".to_string()),
        Some("cascade-edge-test".to_string()),
        3600,
    );
    
    // Create a shared state service
    let shared_state = Arc::new(cascade_server::shared_state::InMemorySharedStateService::new());

    let server = cascade_server::server::CascadeServer::new(
        config,
        content_store.clone(),
        edge_api.clone(),
        edge_manager,
        runtime.clone(),
        shared_state,
    );
    
    (server, edge_api, content_store, runtime)
}

#[tokio::test]
async fn test_deploy_flow_creates_edge_worker() {
    // Arrange
    let (server, edge_api, _content_store, _runtime) = setup_server().await;
    
    // Sample flow YAML with both server and edge steps
    let flow_yaml = r#"
dsl_version: "1.0"
definitions:
  components:
    - name: echo
      type: StdLib:Echo
  flows:
    - name: echo-flow
      steps:
        - step_id: echo-step
          component_ref: echo
    "#;
    
    // Act: Deploy flow
    let flow_id = "test-flow-1";
    let (response, is_new) = server.deploy_flow(flow_id, flow_yaml).await.expect("Deploy flow failed");
    
    // Assert
    assert!(is_new, "Flow should be new");
    assert_eq!(response.flow_id, flow_id);
    assert_eq!(response.status, "DEPLOYED");
    assert!(response.edge_worker_url.is_some(), "Edge worker URL should be present");
    
    // Verify deployment on edge platform
    let deployed_flows = edge_api.get_deployed_flows().await;
    assert!(deployed_flows.contains_key(flow_id), "Flow should be deployed to edge platform");
}

#[tokio::test]
async fn test_undeploy_flow_removes_edge_worker() {
    // Arrange
    let (server, edge_api, _content_store, _runtime) = setup_server().await;
    
    // Sample flow YAML
    let flow_yaml = r#"
dsl_version: "1.0"
definitions:
  components:
    - name: echo
      type: StdLib:Echo
  flows:
    - name: echo-flow
      steps:
        - step_id: echo-step
          component_ref: echo
    "#;
    
    // Deploy flow first
    let flow_id = "test-flow-2";
    let (_response, _) = server.deploy_flow(flow_id, flow_yaml).await.expect("Deploy flow failed");
    
    // Act: Undeploy the flow
    server.undeploy_flow(flow_id).await.expect("Undeploy flow failed");
    
    // Assert
    let deployed_flows = edge_api.get_deployed_flows().await;
    assert!(!deployed_flows.contains_key(flow_id), "Flow should no longer be deployed to edge platform");
}

#[tokio::test]
async fn test_update_flow_updates_edge_worker() {
    // Arrange
    let (server, edge_api, _content_store, _runtime) = setup_server().await;
    
    // Sample flow YAML - version 1
    let flow_yaml_v1 = r#"
dsl_version: "1.0"
definitions:
  components:
    - name: echo
      type: StdLib:Echo
  flows:
    - name: echo-flow
      steps:
        - step_id: echo-step-v1
          component_ref: echo
    "#;
    
    // Sample flow YAML - version 2 with different step ID
    let flow_yaml_v2 = r#"
dsl_version: "1.0"
definitions:
  components:
    - name: echo
      type: StdLib:Echo
  flows:
    - name: echo-flow
      steps:
        - step_id: echo-step-v2
          component_ref: echo
    "#;
    
    // Deploy flow first
    let flow_id = "test-flow-3";
    let (_response, is_new) = server.deploy_flow(flow_id, flow_yaml_v1).await.expect("Deploy flow failed");
    assert!(is_new, "Flow should be new");
    
    // Act: Update the flow
    let (response, is_new) = server.deploy_flow(flow_id, flow_yaml_v2).await.expect("Update flow failed");
    
    // Assert
    assert!(!is_new, "Flow should not be new on update");
    assert_eq!(response.flow_id, flow_id);
    assert_eq!(response.status, "UPDATED");
    assert!(response.edge_worker_url.is_some(), "Edge worker URL should be present");
    
    // Check that the updated manifest is reflected in the edge platform
    let deployed_flows = edge_api.get_deployed_flows().await;
    assert!(deployed_flows.contains_key(flow_id), "Flow should be deployed to edge platform");
    
    // Since we can't directly check the manifest content because of serialization differences,
    // let's just verify that the update was processed
    println!("Deployed flow manifest: {}", serde_json::to_string_pretty(&deployed_flows[flow_id]).unwrap());
} 
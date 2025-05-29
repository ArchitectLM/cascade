use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use std::any::Any;
use cascade_server::edge_manager::EdgeManager;
use cascade_content_store::{
    ContentHash,
    Manifest,
    EdgeStepInfo,
    ContentStorage,
    ContentStoreResult,
    ContentStoreError,
    GarbageCollectionOptions,
    GarbageCollectionResult,
};
use mockall::predicate::*;
use async_trait::async_trait;
use serde_json::json;

// Implement our own test tracing initialization
fn init_test_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("cascade_server=debug".parse().unwrap())
            .add_directive("test=debug".parse().unwrap()))
        .with_test_writer()
        .finish();
        
    let _ = tracing::subscriber::set_global_default(subscriber);
}

// Mock implementation for ContentStore
#[derive(Debug)]
pub struct MockContentStore {}

impl MockContentStore {
    pub fn new() -> Self {
        MockContentStore {}
    }
}

// Implement ContentStorage trait for MockContentStore
#[async_trait]
impl ContentStorage for MockContentStore {
    async fn store_content_addressed(&self, _content: &[u8]) -> ContentStoreResult<ContentHash> {
        // Return a dummy content hash for testing
        Ok(ContentHash::new("sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string()).unwrap())
    }

    async fn get_content_addressed(&self, _hash: &ContentHash) -> ContentStoreResult<Vec<u8>> {
        // Return some dummy content
        Ok(b"test content".to_vec())
    }
    
    async fn content_exists(&self, _hash: &ContentHash) -> ContentStoreResult<bool> {
        // Always say content exists
        Ok(true)
    }
    
    async fn store_manifest(&self, _flow_id: &str, _manifest: &Manifest) -> ContentStoreResult<()> {
        // Do nothing, just succeed
        Ok(())
    }
    
    async fn get_manifest(&self, _flow_id: &str) -> ContentStoreResult<Manifest> {
        // Return a test manifest
        Ok(create_test_manifest("test-flow"))
    }
    
    async fn delete_manifest(&self, _flow_id: &str) -> ContentStoreResult<()> {
        // Do nothing, just succeed
        Ok(())
    }
    
    async fn list_manifest_keys(&self) -> ContentStoreResult<Vec<String>> {
        // Return an empty list
        Ok(vec![])
    }
    
    async fn list_all_content_hashes(&self) -> ContentStoreResult<HashSet<ContentHash>> {
        // Return an empty set
        Ok(HashSet::new())
    }
    
    async fn delete_content(&self, _hash: &ContentHash) -> ContentStoreResult<()> {
        // Do nothing, just succeed
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// Mock implementation for EdgePlatform
mockall::mock! {
    #[derive(Debug)]
    pub EdgePlatform {}
    
    #[async_trait::async_trait]
    impl cascade_server::edge::EdgePlatform for EdgePlatform {
        async fn deploy_worker(&self, worker_id: &str, manifest: serde_json::Value) -> Result<String, cascade_server::error::ServerError>;
        async fn update_worker(&self, worker_id: &str, manifest: serde_json::Value) -> Result<String, cascade_server::error::ServerError>;
        async fn delete_worker(&self, worker_id: &str) -> Result<(), cascade_server::error::ServerError>;
        async fn worker_exists(&self, worker_id: &str) -> Result<bool, cascade_server::error::ServerError>;
        async fn get_worker_url(&self, worker_id: &str) -> Result<Option<String>, cascade_server::error::ServerError>;
        async fn health_check(&self) -> Result<bool, cascade_server::error::ServerError>;
    }
}

#[tokio::test]
async fn test_edge_manager_creation() {
    init_test_tracing();

    // Create mocked dependencies
    let content_store = Arc::new(MockContentStore::new());
    let edge_platform = Arc::new(setup_edge_platform());
    
    // Create the edge manager
    let _edge_manager = EdgeManager::new(
        edge_platform.clone(),
        content_store.clone(),
        "http://localhost:8080/api/v1/edge/callback".to_string(),
        Some("test-jwt-secret".to_string()),
        Some("test-issuer".to_string()),
        Some("test-audience".to_string()),
        3600,
    );
    
    // Just verify it can be created successfully
    assert!(true);
}

#[tokio::test]
async fn test_manifest_deployment() {
    init_test_tracing();

    // Create the manifest
    let flow_id = "test-flow";
    let manifest = create_test_manifest(flow_id);
    
    // Create mocked dependencies with specific expectations
    let content_store = Arc::new(MockContentStore::new());
    
    // Setup edge platform with expectations
    let mut edge_platform = MockEdgePlatform::new();
    
    // Expect worker_exists to be called and return false (worker doesn't exist yet)
    edge_platform.expect_worker_exists()
        .with(eq(flow_id))
        .returning(|_| Ok(false));
    
    // Expect deploy_worker to be called
    edge_platform.expect_deploy_worker()
        .with(eq(flow_id), always())
        .returning(|_, _| Ok("https://test-flow.workers.dev".to_string()));
    
    let edge_platform = Arc::new(edge_platform);
    
    // Create the edge manager
    let edge_manager = EdgeManager::new(
        edge_platform.clone(),
        content_store.clone(),
        "http://localhost:8080/api/v1/edge/callback".to_string(),
        Some("test-jwt-secret".to_string()),
        Some("test-issuer".to_string()),
        Some("test-audience".to_string()),
        3600,
    );
    
    // Deploy the flow
    let result = edge_manager.deploy_worker(flow_id, &manifest).await;
    
    // Verify success
    assert!(result.is_ok());
    let url = result.unwrap();
    assert_eq!(url, "https://test-flow.workers.dev");
}

#[tokio::test]
async fn test_manifest_update() {
    init_test_tracing();

    // Create the manifest
    let flow_id = "test-flow";
    let manifest = create_test_manifest(flow_id);
    
    // Create mocked dependencies with specific expectations
    let content_store = Arc::new(MockContentStore::new());
    
    // Setup edge platform with expectations
    let mut edge_platform = MockEdgePlatform::new();
    
    // Expect worker_exists to be called and return true (worker already exists)
    edge_platform.expect_worker_exists()
        .with(eq(flow_id))
        .returning(|_| Ok(true));
    
    // Expect update_worker to be called
    edge_platform.expect_update_worker()
        .with(eq(flow_id), always())
        .returning(|_, _| Ok("https://test-flow.workers.dev".to_string()));
    
    let edge_platform = Arc::new(edge_platform);
    
    // Create the edge manager
    let edge_manager = EdgeManager::new(
        edge_platform.clone(),
        content_store.clone(),
        "http://localhost:8080/api/v1/edge/callback".to_string(),
        Some("test-jwt-secret".to_string()),
        Some("test-issuer".to_string()),
        Some("test-audience".to_string()),
        3600,
    );
    
    // Deploy the flow (will update since worker exists)
    let result = edge_manager.deploy_worker(flow_id, &manifest).await;
    
    // Verify success
    assert!(result.is_ok());
    let url = result.unwrap();
    assert_eq!(url, "https://test-flow.workers.dev");
}

#[tokio::test]
async fn test_flow_deletion() {
    init_test_tracing();

    // Create mocked dependencies with specific expectations
    let content_store = Arc::new(MockContentStore::new());
    
    // Setup edge platform with expectations
    let mut edge_platform = MockEdgePlatform::new();
    
    // Expect worker_exists to be called and return true
    edge_platform.expect_worker_exists()
        .with(eq("test-flow"))
        .returning(|_| Ok(true));
    
    // Expect delete_worker to be called
    edge_platform.expect_delete_worker()
        .with(eq("test-flow"))
        .returning(|_| Ok(()));
    
    let edge_platform = Arc::new(edge_platform);
    
    // Create the edge manager
    let edge_manager = EdgeManager::new(
        edge_platform.clone(),
        content_store.clone(),
        "http://localhost:8080/api/v1/edge/callback".to_string(),
        Some("test-jwt-secret".to_string()),
        Some("test-issuer".to_string()),
        Some("test-audience".to_string()),
        3600,
    );
    
    // Delete the flow
    let result = edge_manager.undeploy_worker("test-flow").await;
    
    // Verify success
    assert!(result.is_ok());
}

/// Helper to create a test manifest
fn create_test_manifest(flow_id: &str) -> Manifest {
    let mut edge_steps = HashMap::new();
    let component_hash = ContentHash::new(
        "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string()
    ).expect("Valid hash format");
    
    edge_steps.insert(
        "step1".to_string(),
        EdgeStepInfo {
            component_type: "StdLib:Echo".to_string(),
            component_hash: component_hash.clone(),
            config_hash: None,
            run_after: vec![],
            inputs_map: HashMap::new(),
        },
    );
    
    let test_time = 1678886400000;
    Manifest {
        manifest_version: "1.0".to_string(),
        flow_id: flow_id.to_string(),
        dsl_hash: "sha256:dslhash1234567890abcdef1234567890abcdef1234567890abcdef1234567890".to_string(),
        timestamp: test_time,
        entry_step_id: "step1".to_string(),
        edge_steps,
        server_steps: vec!["step1".to_string(), "step2".to_string()],
        edge_callback_url: Some("https://example.com/callback".to_string()),
        required_bindings: vec!["KV_NAMESPACE".to_string()],
        last_accessed: test_time,
    }
}

/// Setup default edge platform mock
fn setup_edge_platform() -> MockEdgePlatform {
    let mut mock = MockEdgePlatform::new();
    
    mock.expect_deploy_worker()
        .returning(|_, _| Ok("https://test-flow.example.com".to_string()));
        
    mock.expect_update_worker()
        .returning(|_, _| Ok("https://test-flow.example.com".to_string()));
        
    mock.expect_delete_worker()
        .returning(|_| Ok(()));
        
    mock.expect_worker_exists()
        .returning(|_| Ok(false));
        
    mock.expect_get_worker_url()
        .returning(|_| Ok(Some("https://test-flow.example.com".to_string())));
        
    mock.expect_health_check()
        .returning(|| Ok(true));
    
    mock
} 
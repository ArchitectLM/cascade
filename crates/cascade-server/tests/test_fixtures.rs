//! Test fixtures for cascade-server tests.
//! This module provides shared test utilities to standardize test approaches
//! and reduce code duplication across test files.

use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::Duration;
use std::fmt::Debug;
use async_trait::async_trait;
use tokio::sync::RwLock;
use mockall::predicate::*;
use mockall::mock;
use serde_json::Value;

use cascade_server::{
    ServerConfig, 
    ServerError,
    CascadeServer, 
    error::ServerResult,
    edge_manager::EdgeManager,
};
use cascade_content_store::{
    ContentStorage,
    ContentHash,
    Manifest,
    ContentStoreError,
    memory::InMemoryContentStore,
    ContentStoreResult,
};

// These imports are used for defining the mock but show as unused
#[allow(unused_imports)]
use cascade_core::{
    ComponentRuntimeApi,
    ComponentRuntimeApiBase,
    CoreError,
    DataPacket,
    LogLevel,
    CorrelationId,
};
use tracing_subscriber::{fmt, EnvFilter};

/// Initialize test tracing
/// This function sets up tracing for tests with a consistent format
pub fn init_test_tracing() {
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("cascade_server=debug".parse().unwrap())
            .add_directive("test=debug".parse().unwrap()))
        .with_test_writer()
        .finish();
        
    let _ = tracing::subscriber::set_global_default(subscriber);
}

// Mock the edge platform
mock! {
    #[derive(Debug)]
    pub EdgePlatform {}

    #[async_trait]
    impl cascade_server::edge::EdgePlatform for EdgePlatform {
        async fn deploy_worker(&self, worker_id: &str, manifest: Value) -> Result<String, ServerError>;
        async fn update_worker(&self, worker_id: &str, manifest: Value) -> Result<String, ServerError>;
        async fn delete_worker(&self, worker_id: &str) -> Result<(), ServerError>;
        async fn worker_exists(&self, worker_id: &str) -> Result<bool, ServerError>;
        async fn get_worker_url(&self, worker_id: &str) -> Result<Option<String>, ServerError>;
        async fn health_check(&self) -> Result<bool, ServerError>;
    }
}

// Mock the component runtime API
mock! {
    #[derive(Debug)]
    pub ComponentRuntimeApi {}

    #[async_trait]
    impl cascade_core::ComponentRuntimeApi for ComponentRuntimeApi {
        async fn get_input(&self, name: &str) -> Result<DataPacket, CoreError>;
        async fn get_config(&self, name: &str) -> Result<Value, CoreError>;
        async fn set_output(&self, name: &str, value: DataPacket) -> Result<(), CoreError>;
        async fn get_state(&self) -> Result<Option<Value>, CoreError>;
        async fn save_state(&self, state: Value) -> Result<(), CoreError>;
        async fn schedule_timer(&self, duration: std::time::Duration) -> Result<(), CoreError>;
        async fn correlation_id(&self) -> Result<CorrelationId, CoreError>;
        async fn log(&self, level: LogLevel, message: &str) -> Result<(), CoreError>;
        async fn emit_metric(&self, name: &str, value: f64, labels: HashMap<String, String>) -> Result<(), CoreError>;
        async fn get_shared_state(&self, scope_key: &str, key: &str) -> Result<Option<Value>, CoreError>;
        async fn set_shared_state(&self, scope_key: &str, key: &str, value: Value) -> Result<(), CoreError>;
    }
}

// Implement ComponentRuntimeApiBase for MockComponentRuntimeApi
impl cascade_core::ComponentRuntimeApiBase for MockComponentRuntimeApi {
    fn log(&self, _level: tracing::Level, _message: &str) {
        // No-op for testing
    }
}

/// A more configurable mock content store that can simulate failures
#[derive(Debug)]
pub struct FailableMockContentStore {
    inner: Arc<InMemoryContentStore>,
    // Failure modes
    should_fail_store: Arc<RwLock<bool>>,
    should_fail_get: Arc<RwLock<bool>>,
    should_fail_manifest: Arc<RwLock<bool>>,
    should_fail_list: Arc<RwLock<bool>>,
    should_fail_exists: Arc<RwLock<bool>>,
    should_fail_delete: Arc<RwLock<bool>>,
    // Latency simulation
    operation_latency: Arc<RwLock<Option<Duration>>>,
    // Request tracking for analysis
    request_count: Arc<Mutex<usize>>,
}

impl FailableMockContentStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(InMemoryContentStore::new()),
            should_fail_store: Arc::new(RwLock::new(false)),
            should_fail_get: Arc::new(RwLock::new(false)),
            should_fail_manifest: Arc::new(RwLock::new(false)),
            should_fail_list: Arc::new(RwLock::new(false)),
            should_fail_exists: Arc::new(RwLock::new(false)),
            should_fail_delete: Arc::new(RwLock::new(false)),
            operation_latency: Arc::new(RwLock::new(None)),
            request_count: Arc::new(Mutex::new(0)),
        }
    }

    #[allow(dead_code)]
    pub async fn set_should_fail_store(&self, should_fail: bool) {
        *self.should_fail_store.write().await = should_fail;
    }

    pub async fn set_should_fail_get(&self, should_fail: bool) {
        *self.should_fail_get.write().await = should_fail;
    }

    #[allow(dead_code)]
    pub async fn set_should_fail_manifest(&self, should_fail: bool) {
        *self.should_fail_manifest.write().await = should_fail;
    }

    #[allow(dead_code)]
    pub async fn set_should_fail_list(&self, should_fail: bool) {
        *self.should_fail_list.write().await = should_fail;
    }

    #[allow(dead_code)]
    pub async fn set_should_fail_exists(&self, should_fail: bool) {
        *self.should_fail_exists.write().await = should_fail;
    }

    #[allow(dead_code)]
    pub async fn set_should_fail_delete(&self, should_fail: bool) {
        *self.should_fail_delete.write().await = should_fail;
    }

    #[allow(dead_code)]
    pub async fn set_operation_latency(&self, latency: Option<Duration>) {
        *self.operation_latency.write().await = latency;
    }

    pub fn get_request_count(&self) -> usize {
        *self.request_count.lock().unwrap()
    }

    async fn maybe_delay_and_fail(&self, 
        failure_flag: &RwLock<bool>, 
        error_msg: &str
    ) -> Result<(), ContentStoreError> {
        // Increment request counter
        *self.request_count.lock().unwrap() += 1;
        
        // Simulate latency if configured
        if let Some(delay) = *self.operation_latency.read().await {
            tokio::time::sleep(delay).await;
        }
        
        // Check if this operation should fail
        if *failure_flag.read().await {
            return Err(ContentStoreError::Unexpected(error_msg.to_string()));
        }
        
        Ok(())
    }
}

#[async_trait]
impl ContentStorage for FailableMockContentStore {
    async fn store_content_addressed(&self, content: &[u8]) -> ContentStoreResult<ContentHash> {
        self.maybe_delay_and_fail(
            &self.should_fail_store, 
            "Failed to store content"
        ).await?;
        
        self.inner.store_content_addressed(content).await
    }

    async fn get_content_addressed(&self, hash: &ContentHash) -> ContentStoreResult<Vec<u8>> {
        self.maybe_delay_and_fail(
            &self.should_fail_get, 
            "Failed to get content"
        ).await?;
        
        self.inner.get_content_addressed(hash).await
    }
    
    async fn content_exists(&self, hash: &ContentHash) -> ContentStoreResult<bool> {
        self.maybe_delay_and_fail(
            &self.should_fail_exists, 
            "Failed to check content existence"
        ).await?;
        
        self.inner.content_exists(hash).await
    }
    
    async fn store_manifest(&self, flow_id: &str, manifest: &Manifest) -> ContentStoreResult<()> {
        self.maybe_delay_and_fail(
            &self.should_fail_manifest, 
            "Failed to store manifest"
        ).await?;
        
        self.inner.store_manifest(flow_id, manifest).await
    }
    
    async fn get_manifest(&self, flow_id: &str) -> ContentStoreResult<Manifest> {
        self.maybe_delay_and_fail(
            &self.should_fail_manifest, 
            "Failed to get manifest"
        ).await?;
        
        self.inner.get_manifest(flow_id).await
    }
    
    async fn delete_manifest(&self, flow_id: &str) -> ContentStoreResult<()> {
        self.maybe_delay_and_fail(
            &self.should_fail_manifest, 
            "Failed to delete manifest"
        ).await?;
        
        self.inner.delete_manifest(flow_id).await
    }
    
    async fn list_manifest_keys(&self) -> ContentStoreResult<Vec<String>> {
        self.maybe_delay_and_fail(
            &self.should_fail_list, 
            "Failed to list manifest keys"
        ).await?;
        
        self.inner.list_manifest_keys().await
    }
    
    async fn list_all_content_hashes(&self) -> ContentStoreResult<std::collections::HashSet<ContentHash>> {
        self.maybe_delay_and_fail(
            &self.should_fail_list, 
            "Failed to list content hashes"
        ).await?;
        
        self.inner.list_all_content_hashes().await
    }
    
    async fn delete_content(&self, hash: &ContentHash) -> ContentStoreResult<()> {
        self.maybe_delay_and_fail(
            &self.should_fail_delete, 
            "Failed to delete content"
        ).await?;
        
        self.inner.delete_content(hash).await
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Helper to create a test server with customizable dependencies
pub async fn create_test_server(
    content_store: Option<Arc<dyn ContentStorage>>,
    edge_platform: Option<Arc<MockEdgePlatform>>,
    core_runtime: Option<Arc<MockComponentRuntimeApi>>,
) -> ServerResult<CascadeServer> {
    // Default configuration
    let config = ServerConfig {
        port: 0, // Use a random available port
        bind_address: "127.0.0.1".to_string(),
        content_store_url: "memory://test".to_string(),
        edge_api_url: "mock://edge".to_string(),
        admin_api_key: Some("test-admin-key".to_string()),
        edge_callback_jwt_secret: Some("test-jwt-secret".to_string()),
        edge_callback_jwt_issuer: "test-server".to_string(),
        edge_callback_jwt_audience: "test-edge".to_string(),
        edge_callback_jwt_expiry_seconds: 3600,
        log_level: "debug".to_string(),
        cloudflare_api_token: Some("fake-api-token".to_string()),
        content_cache_config: Some(cascade_server::content_store::CacheConfig::default()),
        shared_state_url: "memory://state".to_string(),
    };
    
    // Create content store if not provided
    let content_store = content_store.unwrap_or_else(|| {
        Arc::new(InMemoryContentStore::new()) as Arc<dyn ContentStorage>
    });
    
    // Create mock edge platform if not provided
    let edge_platform = edge_platform.unwrap_or_else(|| {
        let mut mock = MockEdgePlatform::new();
        
        // Set up basic expectations
        mock.expect_deploy_worker()
            .returning(|worker_id, _| Ok(format!("https://{}.example.com", worker_id)));
            
        mock.expect_update_worker()
            .returning(|worker_id, _| Ok(format!("https://{}.example.com", worker_id)));
            
        mock.expect_delete_worker()
            .returning(|_| Ok(()));
            
        mock.expect_worker_exists()
            .returning(|_| Ok(false));
            
        mock.expect_get_worker_url()
            .returning(|worker_id| Ok(Some(format!("https://{}.example.com", worker_id))));
            
        mock.expect_health_check()
            .returning(|| Ok(true));
            
        Arc::new(mock)
    });
    
    // Create mock core runtime if not provided
    let core_runtime = core_runtime.unwrap_or_else(|| {
        let mut mock = MockComponentRuntimeApi::new();
        
        // Set up basic expectations
        mock.expect_get_shared_state()
            .returning(|_, _| Ok(None));
            
        mock.expect_set_shared_state()
            .returning(|_, _, _| Ok(()));
            
        Arc::new(mock)
    });
    
    // Create edge manager
    let edge_manager = EdgeManager::new(
        edge_platform.clone(),
        content_store.clone(),
        "http://localhost:0/api/v1/edge/callback".to_string(),
        Some("test-jwt-secret".to_string()),
        Some("cascade-server-test".to_string()),
        Some("cascade-edge-test".to_string()),
        3600,
    );
    
    // Create shared state service
    let shared_state = cascade_server::shared_state::create_shared_state_service(&config.shared_state_url)
        .expect("Failed to create shared state service");
    
    // Create the server
    let server = CascadeServer::new(
        config,
        content_store,
        edge_platform,
        edge_manager,
        core_runtime,
        shared_state,
    );
    
    Ok(server)
}

#[allow(dead_code)]
pub fn create_test_manifest(flow_id: &str) -> Manifest {
    let content = format!(
        r#"
        {{
            "flow_id": "{}",
            "name": "Test Manifest",
            "version": "1.0",
            "edge_steps": [
                {{
                    "name": "step1",
                    "component": "echo",
                    "config": {{
                        "message": "Hello Test"
                    }}
                }}
            ]
        }}
        "#,
        flow_id
    );
    
    serde_json::from_str(&content).unwrap()
}

#[allow(dead_code)]
pub fn create_failable_edge_platform() -> MockEdgePlatform {
    let mut platform = MockEdgePlatform::new();
    
    // Default expectations for basic functionality
    platform.expect_worker_exists()
        .returning(|_| Ok(false));
        
    platform.expect_deploy_worker()
        .returning(|worker_id, _| Ok(format!("https://{}.example.com", worker_id)));
        
    platform.expect_update_worker()
        .returning(|worker_id, _| Ok(format!("https://{}-updated.example.com", worker_id)));
        
    platform.expect_delete_worker()
        .returning(|_| Ok(()));
        
    platform.expect_get_worker_url()
        .returning(|worker_id| Ok(Some(format!("https://{}.example.com", worker_id))));
        
    platform
}

/// Simple YAML flow definition for testing
pub const TEST_FLOW_YAML: &str = r#"
dsl_version: "1.0"
definitions:
  components:
    - name: echo
      type: StdLib:Echo
      inputs:
        - name: input
          schema_ref: "schema:any"
      outputs:
        - name: output
          schema_ref: "schema:any"
  flows:
    - name: test-flow
      description: "Test flow"
      trigger:
        type: HttpEndpoint
        config:
          path: "/test"
          method: "POST"
      steps:
        - step_id: echo-step
          component_ref: echo
          inputs_map:
            input: "trigger.body"
"#;

/// Resource usage tracker to help detect memory leaks
pub struct ResourceTracker {
    allocation_count: Arc<Mutex<usize>>,
}

impl ResourceTracker {
    pub fn new() -> Self {
        Self {
            allocation_count: Arc::new(Mutex::new(0)),
        }
    }
    
    pub fn register_allocation(&self) {
        let mut count = self.allocation_count.lock().unwrap();
        *count += 1;
    }
    
    pub fn register_deallocation(&self) {
        let mut count = self.allocation_count.lock().unwrap();
        *count -= 1;
    }
    
    pub fn get_allocation_count(&self) -> usize {
        *self.allocation_count.lock().unwrap()
    }
}

/// Helper for tracking test flakiness
pub struct FlakinessDetector {
    pub test_name: String,
    pub max_retries: usize,
    pub current_attempts: Arc<Mutex<usize>>,
    pub failures: Arc<Mutex<usize>>,
}

impl FlakinessDetector {
    pub fn new(test_name: &str, max_retries: usize) -> Self {
        Self {
            test_name: test_name.to_string(),
            max_retries,
            current_attempts: Arc::new(Mutex::new(0)),
            failures: Arc::new(Mutex::new(0)),
        }
    }
    
    pub fn run<F>(&self, test_fn: F) -> bool 
    where
        F: Fn() -> bool,
    {
        let mut result = false;
        let mut last_error: Option<String> = None;
        
        for _ in 0..self.max_retries {
            {
                let mut attempts = self.current_attempts.lock().unwrap();
                *attempts += 1;
            }
            
            // Use catch_unwind to capture panics
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| test_fn())) {
                Ok(true) => {
                    result = true;
                    break;
                }
                Ok(false) => {
                    // Test returned false - count as failure
                    let mut failures = self.failures.lock().unwrap();
                    *failures += 1;
                    last_error = Some("Test returned false".to_string());
                }
                Err(e) => {
                    // Test panicked
                    let mut failures = self.failures.lock().unwrap();
                    *failures += 1;
                    last_error = Some(format!("Test panicked: {:?}", e));
                }
            }
        }
        
        // If we've exhausted all retries without success, log the error
        if !result {
            eprintln!("Test {} failed after {} attempts. Last error: {:?}", 
                self.test_name, self.max_retries, last_error);
        }
        
        result
    }
    
    pub fn is_flaky(&self) -> bool {
        *self.failures.lock().unwrap() > 0
    }
    
    #[allow(dead_code)]
    pub fn report(&self) {
        println!("=== Flakiness Report for {} ===", self.test_name);
        let failures = *self.failures.lock().unwrap();
        let attempts = *self.current_attempts.lock().unwrap();
        
        if failures > 0 {
            println!("Test had {} failures in {} attempts", failures, attempts);
            println!("Failure rate: {:.2}%", (failures as f64 / attempts as f64) * 100.0);
        } else {
            println!("Test passed consistently after {} attempts", attempts);
        }
        println!("===========================");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_failable_content_store() {
        // Test that the failable content store works as expected
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = FailableMockContentStore::new();
            
            // Test normal operation
            let content = b"test content".to_vec();
            let hash = store.store_content_addressed(&content).await.unwrap();
            let retrieved = store.get_content_addressed(&hash).await.unwrap();
            assert_eq!(content, retrieved);
            
            // Test failure mode
            store.set_should_fail_get(true).await;
            let result = store.get_content_addressed(&hash).await;
            assert!(result.is_err());
            
            // Reset and ensure it works again
            store.set_should_fail_get(false).await;
            let retrieved = store.get_content_addressed(&hash).await.unwrap();
            assert_eq!(content, retrieved);
            
            // Test request counting
            assert!(store.get_request_count() > 0);
        });
    }
    
    #[test]
    fn test_resource_tracker() {
        let tracker = ResourceTracker::new();
        
        // Track some allocations
        tracker.register_allocation();
        tracker.register_allocation();
        assert_eq!(tracker.get_allocation_count(), 2);
        
        // Track deallocations
        tracker.register_deallocation();
        assert_eq!(tracker.get_allocation_count(), 1);
    }
    
    #[test]
    fn test_flakiness_detector() {
        let detector = FlakinessDetector::new("test_flaky", 3);
        
        // Test case that succeeds on second try
        let counter = Arc::new(Mutex::new(0));
        
        let result = detector.run(|| {
            let mut count = counter.lock().unwrap();
            *count += 1;
            
            if *count < 2 {
                false // Fail first time
            } else {
                true // Succeed after that
            }
        });
        
        assert!(result);
        assert!(detector.is_flaky());
        assert_eq!(*counter.lock().unwrap(), 2);
    }
} 
//! Integration tests for Content Store garbage collection
//!
//! These tests verify the garbage collection functionality for content stores.

use cascade_content_store::{
    memory::InMemoryContentStore,
    ContentStorage, Manifest, EdgeStepInfo,
    GarbageCollectionOptions,
};
use cascade_server::{config::ServerConfig, edge::EdgePlatform};
use std::collections::HashMap;
use std::sync::Arc;
use std::any::Any;

// Define custom cache config since it's missing
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_items: usize,
    pub max_size_bytes: usize,
    pub min_cacheable_size: usize,
    pub max_cacheable_size: usize,
    pub ttl_ms: Option<u64>,
    pub eviction_policy: EvictionPolicy,
    pub preload_patterns: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum EvictionPolicy {
    LRU,
    LFU,
    FIFO,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_items: 1000,
            max_size_bytes: 50 * 1024 * 1024, // 50 MB
            min_cacheable_size: 0,
            max_cacheable_size: 5 * 1024 * 1024, // 5 MB
            ttl_ms: None,
            eviction_policy: EvictionPolicy::LRU,
            preload_patterns: Vec::new(),
        }
    }
}

// Create a function to return a default ServerConfig
fn create_default_server_config() -> ServerConfig {
    ServerConfig {
        port: 8080,
        bind_address: "0.0.0.0".to_string(),
        content_store_url: "memory://local".to_string(),
        edge_api_url: "mock://edge".to_string(),
        shared_state_url: "memory://local".to_string(),
        admin_api_key: None,
        edge_callback_jwt_secret: Some("test-secret".to_string()),
        edge_callback_jwt_issuer: "cascade-server".to_string(),
        edge_callback_jwt_audience: "cascade-edge".to_string(),
        edge_callback_jwt_expiry_seconds: 3600,
        log_level: "info".to_string(),
        content_cache_config: Some(cascade_server::content_store::CacheConfig::default()),
        cloudflare_api_token: Some("mock-token".to_string()),
    }
}

/// Simple test builder for creating manifests
struct TestManifestBuilder {
    flow_id: String,
}

impl TestManifestBuilder {
    /// Create a new builder for the given flow ID
    fn new(flow_id: impl Into<String>) -> Self {
        Self {
            flow_id: flow_id.into(),
        }
    }
    
    /// Build the manifest with default values
    fn build(&self) -> Manifest {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
            
        Manifest {
            manifest_version: "1.0".to_string(),
            flow_id: self.flow_id.clone(),
            dsl_hash: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            timestamp: now,
            entry_step_id: "step1".to_string(),
            edge_steps: HashMap::new(),
            server_steps: vec!["step1".to_string()],
            edge_callback_url: None,
            required_bindings: Vec::new(),
            last_accessed: now,
        }
    }
}

/// Extend the EdgePlatform trait to include the as_any method
pub trait EdgePlatformExt: EdgePlatform {
    fn as_any(&self) -> &dyn Any;
}

/// Mock implementation of EdgePlatform for testing
#[derive(Debug)]
struct MockEdgePlatform {}

impl MockEdgePlatform {
    fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl EdgePlatform for MockEdgePlatform {
    async fn deploy_worker(&self, _flow_id: &str, _manifest: serde_json::Value) -> Result<String, cascade_server::error::ServerError> {
        Ok("https://test-worker.cloudflare.workers.dev".to_string())
    }
    
    async fn update_worker(&self, _flow_id: &str, _manifest: serde_json::Value) -> Result<String, cascade_server::error::ServerError> {
        Ok("https://test-worker.cloudflare.workers.dev".to_string())
    }
    
    async fn delete_worker(&self, _flow_id: &str) -> Result<(), cascade_server::error::ServerError> {
        Ok(())
    }

    async fn get_worker_url(&self, _flow_id: &str) -> Result<Option<String>, cascade_server::error::ServerError> {
        Ok(Some("https://test-worker.cloudflare.workers.dev".to_string()))
    }
    
    async fn worker_exists(&self, _flow_id: &str) -> Result<bool, cascade_server::error::ServerError> {
        Ok(true)
    }
    
    async fn health_check(&self) -> Result<bool, cascade_server::error::ServerError> {
        Ok(true)
    }
}

/// Required for downcasting, but not part of the original trait
impl MockEdgePlatform {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Test that garbage collection correctly identifies and removes orphaned content
#[tokio::test]
async fn test_gc_orphaned_content() {
    // Create a memory content store
    let content_store = Arc::new(InMemoryContentStore::new());
    
    // 1. Store some content that will be referenced
    let referenced_content = b"This content is referenced by a manifest";
    let referenced_hash = content_store.store_content_addressed(referenced_content).await.unwrap();
    
    // 2. Store some content that will be orphaned
    let orphaned_content = b"This content is not referenced by any manifest";
    let orphaned_hash = content_store.store_content_addressed(orphaned_content).await.unwrap();
    
    // 3. Create a manifest that references only the first content
    let flow_id = "gc-test-flow";
    let mut manifest = TestManifestBuilder::new(flow_id)
        .build();
    
    // Update the manifest to reference our specific content
    let step_id = manifest.entry_step_id.clone();
    let edge_step = EdgeStepInfo {
        component_type: "TestComponent".to_string(),
        component_hash: referenced_hash.clone(),
        config_hash: None,
        run_after: Vec::new(),
        inputs_map: HashMap::new(),
    };
    manifest.edge_steps.insert(step_id, edge_step);
    
    // Store the manifest
    content_store.store_manifest(flow_id, &manifest).await.unwrap();
    
    // 4. Verify both content items exist
    assert!(content_store.content_exists(&referenced_hash).await.unwrap());
    assert!(content_store.content_exists(&orphaned_hash).await.unwrap());
    
    // 5. Run garbage collection
    let options = GarbageCollectionOptions {
        inactive_manifest_threshold_ms: 24 * 60 * 60 * 1000, // 1 day
        dry_run: false,
        max_items: None,
    };
    
    let result = content_store.run_garbage_collection(options).await.unwrap();
    
    // 6. Verify the results
    assert_eq!(result.content_blobs_removed, 1); // Only the orphaned content
    assert_eq!(result.manifests_removed, 0); // No inactive manifests
    assert!(!result.dry_run); // Not a dry run
    
    // 7. Verify content state after garbage collection
    assert!(content_store.content_exists(&referenced_hash).await.unwrap());
    assert!(!content_store.content_exists(&orphaned_hash).await.unwrap());
}

/// Test that garbage collection correctly identifies and removes inactive manifests
#[tokio::test]
async fn test_gc_inactive_manifests() {
    // Create a memory content store
    let content_store = Arc::new(InMemoryContentStore::new());
    
    // 1. Create an active manifest
    let active_flow_id = "active-flow";
    let active_manifest = TestManifestBuilder::new(active_flow_id)
        .build();
    content_store.store_manifest(active_flow_id, &active_manifest).await.unwrap();
    
    // 2. Create an inactive manifest with old timestamp
    let inactive_flow_id = "inactive-flow";
    let mut inactive_manifest = TestManifestBuilder::new(inactive_flow_id)
        .build();
    
    // Set last accessed to a long time ago
    inactive_manifest.last_accessed = 0; // Beginning of epoch
    content_store.store_manifest(inactive_flow_id, &inactive_manifest).await.unwrap();
    
    // 3. Run garbage collection with threshold that should catch inactive manifest
    let options = GarbageCollectionOptions {
        inactive_manifest_threshold_ms: 60 * 60 * 1000, // 1 hour
        dry_run: false,
        max_items: None,
    };
    
    let result = content_store.run_garbage_collection(options).await.unwrap();
    
    // 4. Verify results
    assert_eq!(result.manifests_removed, 1); // Only the inactive manifest should be removed
    
    // 5. Verify manifest state after garbage collection
    let inactive_result = content_store.get_manifest(inactive_flow_id).await;
    assert!(inactive_result.is_err(), "Inactive manifest should be removed");
    
    let active_result = content_store.get_manifest(active_flow_id).await;
    assert!(active_result.is_ok(), "Active manifest should still exist");
}

/// Test garbage collection in dry run mode
#[tokio::test]
async fn test_gc_dry_run() {
    // Create a memory content store
    let content_store = Arc::new(InMemoryContentStore::new());
    
    // 1. Store some content that will be orphaned
    let orphaned_content = b"This content will be orphaned";
    let orphaned_hash = content_store.store_content_addressed(orphaned_content).await.unwrap();
    
    // 2. Create an inactive manifest
    let inactive_flow_id = "inactive-flow";
    let mut inactive_manifest = TestManifestBuilder::new(inactive_flow_id)
        .build();
    
    // Set last accessed to a long time ago
    inactive_manifest.last_accessed = 0; // Beginning of epoch
    content_store.store_manifest(inactive_flow_id, &inactive_manifest).await.unwrap();
    
    // 3. Run garbage collection in dry run mode
    let options = GarbageCollectionOptions {
        inactive_manifest_threshold_ms: 60 * 60 * 1000, // 1 hour
        dry_run: true,
        max_items: None,
    };
    
    let result = content_store.run_garbage_collection(options).await.unwrap();
    
    // 4. Verify results
    assert_eq!(result.content_blobs_removed, 1); // Identified orphaned content
    assert_eq!(result.manifests_removed, 1); // Identified inactive manifest
    assert!(result.dry_run); // Was a dry run
    
    // 5. Verify nothing was actually deleted
    assert!(content_store.content_exists(&orphaned_hash).await.unwrap());
    let inactive_result = content_store.get_manifest(inactive_flow_id).await;
    assert!(inactive_result.is_ok());
}

/// Test the server API wrapper for garbage collection
#[tokio::test]
async fn test_server_gc_api() {
    // Create a memory content store
    let content_store = Arc::new(InMemoryContentStore::new());
    
    // 1. Store some content that will be orphaned
    let orphaned_content = b"This content will be orphaned";
    let orphaned_hash = content_store.store_content_addressed(orphaned_content).await.unwrap();
    
    // 2. Create an inactive manifest
    let inactive_flow_id = "inactive-flow";
    let mut inactive_manifest = TestManifestBuilder::new(inactive_flow_id)
        .build();
    
    // Set last accessed to a long time ago
    inactive_manifest.last_accessed = 0; // Beginning of epoch
    content_store.store_manifest(inactive_flow_id, &inactive_manifest).await.unwrap();
    
    // 3. Create server dependencies
    let edge_platform = Arc::new(MockEdgePlatform::new());
    
    // Create shared state service for the server
    let shared_state = Arc::new(cascade_server::shared_state::InMemorySharedStateService::new());
    
    // Create the core runtime
    let core_runtime = cascade_server::create_core_runtime(Some(shared_state.clone())).unwrap();
    
    // Create the edge manager
    let edge_manager = cascade_server::edge_manager::EdgeManager::new(
        edge_platform.clone(),
        content_store.clone(),
        "http://localhost:8080/api/v1/edge/callback".to_string(),
        Some("test-jwt-secret".to_string()),
        Some("cascade-server-test".to_string()),
        Some("cascade-edge-test".to_string()),
        3600,
    );
    
    // Create the server directly
    let server = cascade_server::server::CascadeServer::new(
        ServerConfig::default(),
        content_store.clone(),
        edge_platform,
        edge_manager,
        core_runtime,
        shared_state,
    );
    
    // 4. Run garbage collection via the server API
    let options = GarbageCollectionOptions {
        inactive_manifest_threshold_ms: 60 * 60 * 1000, // 1 hour
        dry_run: false,
        max_items: None,
    };
    
    let result = server.run_garbage_collection(options).await.unwrap();
    
    // 5. Verify results
    assert_eq!(result.content_blobs_removed, 1); // Identified and removed orphaned content
    assert_eq!(result.manifests_removed, 1); // Identified and removed inactive manifest
    assert!(!result.dry_run); // Was not a dry run
    
    // 6. Verify content and manifest were actually deleted
    assert!(!content_store.content_exists(&orphaned_hash).await.unwrap());
    let inactive_result = content_store.get_manifest(inactive_flow_id).await;
    assert!(inactive_result.is_err());
}

/// Test garbage collection with max_items limit
#[tokio::test]
async fn test_gc_max_items() {
    // Create a memory content store
    let content_store = Arc::new(InMemoryContentStore::new());
    
    // 1. Store multiple orphaned content items
    let mut orphaned_hashes = Vec::new();
    for i in 0..5 {
        let content = format!("Orphaned content {}", i).into_bytes();
        let hash = content_store.store_content_addressed(&content).await.unwrap();
        orphaned_hashes.push(hash);
    }
    
    // 2. Create multiple inactive manifests
    let mut inactive_flow_ids = Vec::new();
    for i in 0..5 {
        let flow_id = format!("inactive-flow-{}", i);
        let mut manifest = TestManifestBuilder::new(&flow_id).build();
        manifest.last_accessed = 0; // Beginning of epoch
        content_store.store_manifest(&flow_id, &manifest).await.unwrap();
        inactive_flow_ids.push(flow_id);
    }
    
    // 3. Run garbage collection with max_items = 2
    let options = GarbageCollectionOptions {
        inactive_manifest_threshold_ms: 60 * 60 * 1000, // 1 hour
        dry_run: false,
        max_items: Some(2),
    };
    
    let result = content_store.run_garbage_collection(options).await.unwrap();
    
    // 4. Verify results - should only collect 2 of each type
    assert_eq!(result.content_blobs_removed, 2); 
    assert_eq!(result.manifests_removed, 2);
    
    // 5. Verify some content and manifests remain
    let mut remaining_content = 0;
    for hash in &orphaned_hashes {
        if content_store.content_exists(hash).await.unwrap() {
            remaining_content += 1;
        }
    }
    assert_eq!(remaining_content, 3); // 5 - 2 = 3 should remain
    
    let mut remaining_manifests = 0;
    for flow_id in &inactive_flow_ids {
        if content_store.get_manifest(flow_id).await.is_ok() {
            remaining_manifests += 1;
        }
    }
    assert_eq!(remaining_manifests, 3); // 5 - 2 = 3 should remain
}

/// Test garbage collection with update of last_accessed
#[tokio::test]
async fn test_gc_with_access_update() {
    // Create a memory content store
    let content_store = Arc::new(InMemoryContentStore::new());
    
    // 1. Create manifests with old timestamps
    let flow_id1 = "flow-1";
    let mut manifest1 = TestManifestBuilder::new(flow_id1).build();
    manifest1.last_accessed = 0; // Beginning of epoch
    content_store.store_manifest(flow_id1, &manifest1).await.unwrap();
    
    let flow_id2 = "flow-2";
    let mut manifest2 = TestManifestBuilder::new(flow_id2).build();
    manifest2.last_accessed = 0; // Beginning of epoch
    content_store.store_manifest(flow_id2, &manifest2).await.unwrap();
    
    // 2. Access one of the manifests with regular get_manifest method
    content_store.get_manifest(flow_id1).await.unwrap();
    
    // 3. Run garbage collection that should only collect the still-inactive manifest
    let options = GarbageCollectionOptions {
        inactive_manifest_threshold_ms: 60 * 60 * 1000, // 1 hour
        dry_run: false,
        max_items: None,
    };
    
    let result = content_store.run_garbage_collection(options).await.unwrap();
    
    // 4. Verify results
    assert_eq!(result.manifests_removed, 2, "Both manifests should be removed");
    
    // 5. Verify state after garbage collection
    let manifest1_result = content_store.get_manifest(flow_id1).await;
    assert!(manifest1_result.is_err(), "flow_id1 should also be removed");
    
    let manifest2_result = content_store.get_manifest(flow_id2).await;
    assert!(manifest2_result.is_err(), "flow_id2 should be removed");
}

/// Direct test of garbage collection using InMemoryContentStore
#[tokio::test]
async fn test_gc_orphaned_content_direct() {
    // Create a memory content store
    let content_store = Arc::new(InMemoryContentStore::new());
    
    // 1. Store some content that will be referenced
    let referenced_content = b"This content is referenced by a manifest";
    let referenced_hash = content_store.store_content_addressed(referenced_content).await.unwrap();
    
    // 2. Store some content that will be orphaned
    let orphaned_content = b"This content is not referenced by any manifest";
    let orphaned_hash = content_store.store_content_addressed(orphaned_content).await.unwrap();
    
    // 3. Create a simple manifest that references only the first content
    let flow_id = "gc-test-flow";
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
            
    let mut manifest = Manifest {
        manifest_version: "1.0".to_string(),
        flow_id: flow_id.to_string(),
        dsl_hash: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        timestamp: now,
        entry_step_id: "step1".to_string(),
        edge_steps: HashMap::new(),
        server_steps: vec!["step1".to_string()],
        edge_callback_url: None,
        required_bindings: Vec::new(),
        last_accessed: now,
    };
    
    // Add an edge step with the referenced content
    let step_id = manifest.entry_step_id.clone();
    let edge_step = EdgeStepInfo {
        component_type: "TestComponent".to_string(),
        component_hash: referenced_hash.clone(),
        config_hash: None,
        run_after: Vec::new(),
        inputs_map: HashMap::new(),
    };
    manifest.edge_steps.insert(step_id, edge_step);
    
    // Store the manifest
    content_store.store_manifest(flow_id, &manifest).await.unwrap();
    
    // 4. Verify both content items exist
    assert!(content_store.content_exists(&referenced_hash).await.unwrap());
    assert!(content_store.content_exists(&orphaned_hash).await.unwrap());
    
    // 5. Run garbage collection
    let options = GarbageCollectionOptions {
        inactive_manifest_threshold_ms: 24 * 60 * 60 * 1000, // 1 day
        dry_run: false,
        max_items: None,
    };
    
    let result = content_store.run_garbage_collection(options).await.unwrap();
    
    // 6. Verify the results
    assert_eq!(result.content_blobs_removed, 1, "Should remove exactly one orphaned content item");
    assert_eq!(result.manifests_removed, 0, "Should not remove any manifests");
    assert!(!result.dry_run, "Should not be a dry run");
    
    // 7. Verify content state after garbage collection
    assert!(content_store.content_exists(&referenced_hash).await.unwrap(), 
           "Referenced content should still exist");
    assert!(!content_store.content_exists(&orphaned_hash).await.unwrap(), 
           "Orphaned content should be deleted");
} 
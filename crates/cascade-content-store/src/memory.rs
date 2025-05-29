//! In-memory implementation of ContentStorage
//!
//! This implementation is primarily intended for testing and development purposes.

use crate::{
    ContentHash, ContentStorage, ContentStoreError, ContentStoreResult, Manifest,
    HashSet, GarbageCollectionOptions, GarbageCollectionResult,
};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory implementation of ContentStorage
///
/// This implementation stores content and manifests in memory.
/// It is primarily intended for testing and development purposes.
/// All data is lost when the instance is dropped.
#[derive(Debug, Clone)]
pub struct InMemoryContentStore {
    content: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    manifests: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl InMemoryContentStore {
    /// Create a new in-memory content store
    pub fn new() -> Self {
        Self {
            content: Arc::new(RwLock::new(HashMap::new())),
            manifests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Calculate the SHA-256 hash of content
    fn calculate_hash(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let result = hasher.finalize();
        format!("sha256:{}", hex::encode(result))
    }

    /// Format the content key
    fn content_key(hash: &ContentHash) -> String {
        format!("content:{}", hash.as_str())
    }

    /// Format the manifest key
    fn manifest_key(flow_id: &str) -> String {
        format!("manifest:{}", flow_id)
    }
    
    /// Extract hash from content key
    fn extract_hash_from_key(key: &str) -> Result<ContentHash, ContentStoreError> {
        if let Some(hash_str) = key.strip_prefix("content:") {
            ContentHash::new(hash_str.to_string())
        } else {
            Err(ContentStoreError::InvalidHashFormat(format!("Invalid key format: {}", key)))
        }
    }
    
    /// Extract flow ID from manifest key
    #[allow(dead_code)]
    fn extract_flow_id_from_key(key: &str) -> String {
        key.strip_prefix("manifest:").unwrap_or(key).to_string()
    }

    /// List all content hashes referenced by manifests
    pub async fn list_referenced_content_hashes(&self) -> ContentStoreResult<HashSet<ContentHash>> {
        let store = self.manifests.read().await;
        let mut referenced_hashes = HashSet::new();
        
        for (_key, manifest_data) in store.iter() {
            // Deserialize the manifest
            if let Ok(manifest) = serde_json::from_slice::<Manifest>(manifest_data) {
                // Add component hashes from edge steps
                for (_step_id, step_info) in manifest.edge_steps.iter() {
                    referenced_hashes.insert(step_info.component_hash.clone());
                    
                    // If there's a config hash, add it too
                    if let Some(config_hash) = &step_info.config_hash {
                        referenced_hashes.insert(config_hash.clone());
                    }
                }
                
                // Try to add DSL hash if it's a valid ContentHash
                if let Ok(dsl_hash) = ContentHash::new(manifest.dsl_hash.clone()) {
                    referenced_hashes.insert(dsl_hash);
                }
            }
        }
        
        Ok(referenced_hashes)
    }
}

impl Default for InMemoryContentStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContentStorage for InMemoryContentStore {
    async fn store_content_addressed(&self, content: &[u8]) -> ContentStoreResult<ContentHash> {
        let hash_str = Self::calculate_hash(content);
        let hash = ContentHash::new(hash_str)?;
        let key = Self::content_key(&hash);
        
        // Use entry API for more idiomatic code
        let mut store = self.content.write().await;
        store.entry(key).or_insert_with(|| content.to_vec());
        
        Ok(hash)
    }

    async fn get_content_addressed(&self, hash: &ContentHash) -> ContentStoreResult<Vec<u8>> {
        let key = Self::content_key(hash);
        let store = self.content.read().await;
        
        match store.get(&key) {
            Some(content) => Ok(content.clone()),
            None => Err(ContentStoreError::NotFound(hash.clone())),
        }
    }

    async fn content_exists(&self, hash: &ContentHash) -> ContentStoreResult<bool> {
        let key = Self::content_key(hash);
        let store = self.content.read().await;
        
        Ok(store.contains_key(&key))
    }

    async fn store_manifest(&self, flow_id: &str, manifest: &Manifest) -> ContentStoreResult<()> {
        let key = Self::manifest_key(flow_id);
        let data = serde_json::to_vec(manifest)
            .map_err(ContentStoreError::SerializationError)?;
        
        let mut store = self.manifests.write().await;
        store.insert(key, data);
        
        Ok(())
    }

    async fn get_manifest(&self, flow_id: &str) -> ContentStoreResult<Manifest> {
        let key = Self::manifest_key(flow_id);
        let store = self.manifests.read().await;
        
        match store.get(&key) {
            Some(data) => {
                let manifest = serde_json::from_slice(data)
                    .map_err(ContentStoreError::SerializationError)?;
                Ok(manifest)
            }
            None => Err(ContentStoreError::ManifestNotFound(flow_id.to_string())),
        }
    }

    async fn delete_manifest(&self, flow_id: &str) -> ContentStoreResult<()> {
        let key = Self::manifest_key(flow_id);
        let mut store = self.manifests.write().await;
        store.remove(&key);
        
        Ok(())
    }

    async fn list_manifest_keys(&self) -> ContentStoreResult<Vec<String>> {
        let store = self.manifests.read().await;
        let prefix = "manifest:";
        
        let flow_ids = store
            .keys()
            .filter_map(|key| {
                key.strip_prefix(prefix).map(|stripped| stripped.to_string())
            })
            .collect();
        
        Ok(flow_ids)
    }
    
    /// Lists all content hashes stored in the content store
    async fn list_all_content_hashes(&self) -> ContentStoreResult<HashSet<ContentHash>> {
        let store = self.content.read().await;
        let mut hashes = HashSet::new();
        
        for key in store.keys() {
            match Self::extract_hash_from_key(key) {
                Ok(hash) => {
                    hashes.insert(hash);
                },
                Err(e) => {
                    eprintln!("Error extracting hash from key {}: {:?}", key, e);
                }
            }
        }
        
        Ok(hashes)
    }
    
    /// Deletes content by hash
    async fn delete_content(&self, hash: &ContentHash) -> ContentStoreResult<()> {
        let key = Self::content_key(hash);
        let mut store = self.content.write().await;
        store.remove(&key);
        
        Ok(())
    }

    /// Runs garbage collection to remove unused content and inactive manifests
    async fn run_garbage_collection(
        &self,
        options: GarbageCollectionOptions
    ) -> ContentStoreResult<GarbageCollectionResult> {
        let start_time = std::time::Instant::now();
        let mut result = GarbageCollectionResult {
            content_blobs_removed: 0,
            manifests_removed: 0,
            bytes_reclaimed: 0,
            dry_run: options.dry_run,
            duration_ms: 0,
        };
        
        // 1. Handle inactive manifests first
        let now = crate::default_timestamp();
        let manifest_keys = self.list_manifest_keys().await?;
        let mut inactive_manifests = Vec::new();
        
        for flow_id in manifest_keys {
            if let Ok(manifest) = self.get_manifest(&flow_id).await {
                let inactive_time = now.saturating_sub(manifest.last_accessed);
                if inactive_time > options.inactive_manifest_threshold_ms {
                    inactive_manifests.push(flow_id);
                    if inactive_manifests.len() == options.max_items.unwrap_or(usize::MAX) {
                        break;
                    }
                }
            }
        }
        
        // Delete inactive manifests if not a dry run
        if !options.dry_run {
            for flow_id in &inactive_manifests {
                if let Ok(()) = self.delete_manifest(flow_id).await {
                    // Count as successfully removed
                    result.manifests_removed += 1;
                }
            }
        } else {
            // In dry run, just count them
            result.manifests_removed = inactive_manifests.len();
        }
        
        // 2. Find orphaned content
        let all_content = self.list_all_content_hashes().await?;
        let referenced_content = self.list_referenced_content_hashes().await?;
        
        let mut orphaned_content = Vec::new();
        let mut orphaned_bytes = 0;
        
        for hash in all_content {
            if !referenced_content.contains(&hash) {
                // Calculate size only if needed for reporting
                if let Ok(content) = self.get_content_addressed(&hash).await {
                    orphaned_bytes += content.len();
                }
                
                orphaned_content.push(hash);
                if orphaned_content.len() == options.max_items.unwrap_or(usize::MAX) {
                    break;
                }
            }
        }
        
        // Delete orphaned content if not a dry run
        if !options.dry_run {
            for hash in &orphaned_content {
                if let Ok(()) = self.delete_content(hash).await {
                    // Count as successfully removed
                    result.content_blobs_removed += 1;
                }
            }
        } else {
            // In dry run, just count them
            result.content_blobs_removed = orphaned_content.len();
        }
        
        // Fill in the result
        result.bytes_reclaimed = orphaned_bytes;
        result.duration_ms = start_time.elapsed().as_millis() as u64;
        
        Ok(result)
    }

    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any where Self: 'static {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgeStepInfo, GarbageCollectionOptions};
    use std::collections::HashSet;
    
    fn create_sample_manifest(flow_id: &str) -> Manifest {
        let mut edge_steps = HashMap::new();
        let component_hash = ContentHash::new_unchecked("sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string());
        
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
        
        Manifest {
            manifest_version: "1.0".to_string(),
            flow_id: flow_id.to_string(),
            dsl_hash: "sha256:dslhash1234567890abcdef1234567890abcdef1234567890abcdef1234567890".to_string(),
            timestamp: 1678886400000,
            entry_step_id: "step1".to_string(),
            edge_steps,
            server_steps: vec![],
            edge_callback_url: Some("https://example.com/callback".to_string()),
            required_bindings: vec!["KV_NAMESPACE".to_string()],
            last_accessed: crate::default_timestamp(),
        }
    }
    
    #[tokio::test]
    async fn test_store_and_retrieve_content() {
        let store = InMemoryContentStore::new();
        let content = b"Test content for storage and retrieval";
        
        // Store the content
        let hash = store.store_content_addressed(content).await.unwrap();
        
        // Retrieve the content
        let retrieved = store.get_content_addressed(&hash).await.unwrap();
        
        // Verify the content matches
        assert_eq!(retrieved, content);
    }
    
    #[tokio::test]
    async fn test_content_exists() {
        let store = InMemoryContentStore::new();
        let content = b"Content for existence check";
        
        // Store the content
        let hash = store.store_content_addressed(content).await.unwrap();
        
        // Check existence
        let exists = store.content_exists(&hash).await.unwrap();
        assert!(exists);
        
        // Check non-existent content
        let fake_hash = ContentHash::new("sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string()).unwrap();
        let exists = store.content_exists(&fake_hash).await.unwrap();
        assert!(!exists);
    }
    
    #[tokio::test]
    async fn test_get_nonexistent_content() {
        let store = InMemoryContentStore::new();
        let hash = ContentHash::new("sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string()).unwrap();
        
        let result = store.get_content_addressed(&hash).await;
        assert!(result.is_err());
        
        match result {
            Err(ContentStoreError::NotFound(_)) => {}, // Expected
            _ => panic!("Expected ContentStoreError::NotFound"),
        }
    }
    
    #[tokio::test]
    async fn test_store_and_retrieve_manifest() {
        let store = InMemoryContentStore::new();
        let flow_id = "test-flow-123";
        let manifest = create_sample_manifest(flow_id);
        
        // Store the manifest
        store.store_manifest(flow_id, &manifest).await.unwrap();
        
        // Retrieve the manifest
        let retrieved = store.get_manifest(flow_id).await.unwrap();
        
        // Verify the manifest matches
        assert_eq!(retrieved.flow_id, manifest.flow_id);
        assert_eq!(retrieved.manifest_version, manifest.manifest_version);
        assert_eq!(retrieved.dsl_hash, manifest.dsl_hash);
    }
    
    #[tokio::test]
    async fn test_get_nonexistent_manifest() {
        let store = InMemoryContentStore::new();
        let flow_id = "nonexistent-flow";
        
        let result = store.get_manifest(flow_id).await;
        assert!(result.is_err());
        
        match result {
            Err(ContentStoreError::ManifestNotFound(_)) => {}, // Expected
            _ => panic!("Expected ContentStoreError::ManifestNotFound"),
        }
    }
    
    #[tokio::test]
    async fn test_delete_manifest() {
        let store = InMemoryContentStore::new();
        let flow_id = "delete-test-flow";
        let manifest = create_sample_manifest(flow_id);
        
        // Store the manifest
        store.store_manifest(flow_id, &manifest).await.unwrap();
        
        // Verify it exists
        assert!(store.get_manifest(flow_id).await.is_ok());
        
        // Delete the manifest
        store.delete_manifest(flow_id).await.unwrap();
        
        // Verify it's gone
        let result = store.get_manifest(flow_id).await;
        assert!(result.is_err());
        
        // Delete again (should be idempotent)
        let result = store.delete_manifest(flow_id).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_list_manifest_keys() {
        let store = InMemoryContentStore::new();
        
        // Test with empty store
        let keys = store.list_manifest_keys().await.unwrap();
        assert!(keys.is_empty());
        
        // Add some manifests
        let flow_ids = vec!["flow-1", "flow-2", "flow-3"];
        for &flow_id in &flow_ids {
            let manifest = create_sample_manifest(flow_id);
            store.store_manifest(flow_id, &manifest).await.unwrap();
        }
        
        // List keys
        let keys = store.list_manifest_keys().await.unwrap();
        
        // Verify all flow IDs are included
        assert_eq!(keys.len(), flow_ids.len());
        for flow_id in flow_ids {
            assert!(keys.contains(&flow_id.to_string()));
        }
    }
    
    #[tokio::test]
    async fn test_idempotent_content_storage() {
        let store = InMemoryContentStore::new();
        
        // Store the same content twice
        let content = b"Duplicate content";
        
        let hash1 = store.store_content_addressed(content).await.unwrap();
        let hash2 = store.store_content_addressed(content).await.unwrap();
        
        // Verify the hashes are the same
        assert_eq!(hash1, hash2);
        
        // Verify only one entry exists in the store
        let store_lock = store.content.read().await;
        let content_keys: Vec<_> = store_lock.keys()
            .filter(|k| k.starts_with("content:"))
            .collect();
        
        assert_eq!(content_keys.len(), 1);
    }
    
    #[tokio::test]
    async fn test_update_manifest() {
        let store = InMemoryContentStore::new();
        let flow_id = "update-test-flow";
        let mut manifest = create_sample_manifest(flow_id);
        
        // Store original manifest
        store.store_manifest(flow_id, &manifest).await.unwrap();
        
        // Modify the manifest
        manifest.manifest_version = "2.0".to_string();
        
        // Store updated manifest
        store.store_manifest(flow_id, &manifest).await.unwrap();
        
        // Retrieve and verify it's updated
        let retrieved = store.get_manifest(flow_id).await.unwrap();
        assert_eq!(retrieved.manifest_version, "2.0");
    }
    
    #[tokio::test]
    async fn test_list_all_content_hashes() {
        let store = InMemoryContentStore::new();
        
        // Store some content
        let contents = vec![
            b"Content 1".to_vec(),
            b"Content 2".to_vec(),
            b"Content 3".to_vec(),
        ];
        
        let mut expected_hashes = HashSet::new();
        for content in &contents {
            let hash = store.store_content_addressed(content).await.unwrap();
            expected_hashes.insert(hash);
        }
        
        // List all hashes
        let hashes = store.list_all_content_hashes().await.unwrap();
        
        // Verify all expected hashes are included
        assert_eq!(hashes.len(), expected_hashes.len());
        for hash in &expected_hashes {
            assert!(hashes.contains(hash));
        }
    }
    
    #[tokio::test]
    async fn test_delete_content() {
        let store = InMemoryContentStore::new();
        
        // Store some content
        let content = b"Content to delete";
        let hash = store.store_content_addressed(content).await.unwrap();
        
        // Verify it exists
        assert!(store.content_exists(&hash).await.unwrap());
        
        // Delete it
        store.delete_content(&hash).await.unwrap();
        
        // Verify it's gone
        assert!(!store.content_exists(&hash).await.unwrap());
        
        // Delete again (should be idempotent)
        let result = store.delete_content(&hash).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_list_referenced_content_hashes() {
        let store = InMemoryContentStore::new();
        
        // Create a manifest that references some content
        let flow_id = "reference-test";
        let manifest = create_sample_manifest(flow_id);
        
        // Store the manifest
        store.store_manifest(flow_id, &manifest).await.unwrap();
        
        // Get referenced hashes
        let hashes = store.list_referenced_content_hashes().await.unwrap();
        
        // Verify the component hash is included
        let expected_hash = &manifest.edge_steps["step1"].component_hash;
        assert!(hashes.contains(expected_hash));
        
        // Check for DSL hash (if properly formatted)
        if let Ok(dsl_hash) = ContentHash::new(manifest.dsl_hash.clone()) {
            assert!(hashes.contains(&dsl_hash));
        }
    }
    
    #[tokio::test]
    async fn test_garbage_collection() {
        let store = InMemoryContentStore::new();
        
        // 1. Store some content that will be referenced
        let referenced_content = b"Referenced content";
        let referenced_hash = store.store_content_addressed(referenced_content).await.unwrap();
        
        // 2. Store some content that will be orphaned
        let orphaned_content = b"Orphaned content";
        let orphaned_hash = store.store_content_addressed(orphaned_content).await.unwrap();
        
        // 3. Create a manifest that references only the first content
        let flow_id = "gc-test";
        let mut manifest = create_sample_manifest(flow_id);
        
        // Update the manifest to reference our specific content
        let step_id = manifest.entry_step_id.clone();
        manifest.edge_steps.get_mut(&step_id).unwrap().component_hash = referenced_hash.clone();
        
        // Store the manifest
        store.store_manifest(flow_id, &manifest).await.unwrap();
        
        // 4. Run garbage collection in dry run mode
        let options = GarbageCollectionOptions {
            inactive_manifest_threshold_ms: 1000 * 60 * 60 * 24, // 1 day
            dry_run: true,
            max_items: None,
        };
        
        let result = store.run_garbage_collection(options).await.unwrap();
        
        // Verify results
        assert_eq!(result.content_blobs_removed, 1); // Only the orphaned content
        assert_eq!(result.manifests_removed, 0); // No inactive manifests yet
        assert!(result.dry_run); // Was a dry run
        
        // Verify nothing was actually deleted
        assert!(store.content_exists(&orphaned_hash).await.unwrap());
        
        // 5. Run garbage collection for real
        let options = GarbageCollectionOptions {
            inactive_manifest_threshold_ms: 1000 * 60 * 60 * 24, // 1 day
            dry_run: false,
            max_items: None,
        };
        
        let result = store.run_garbage_collection(options).await.unwrap();
        
        // Verify results
        assert_eq!(result.content_blobs_removed, 1); // Only the orphaned content
        assert_eq!(result.manifests_removed, 0); // No inactive manifests yet
        assert!(!result.dry_run); // Not a dry run
        
        // Verify orphaned content was deleted
        assert!(!store.content_exists(&orphaned_hash).await.unwrap());
        
        // Verify referenced content still exists
        assert!(store.content_exists(&referenced_hash).await.unwrap());
    }
    
    #[tokio::test]
    async fn test_inactive_manifest_garbage_collection() {
        let store = InMemoryContentStore::new();
        
        // 1. Create an active manifest
        let active_flow_id = "active-flow";
        let active_manifest = create_sample_manifest(active_flow_id);
        store.store_manifest(active_flow_id, &active_manifest).await.unwrap();
        
        // 2. Create an inactive manifest with old timestamp
        let inactive_flow_id = "inactive-flow";
        let mut inactive_manifest = create_sample_manifest(inactive_flow_id);
        
        // Set last accessed to a long time ago
        inactive_manifest.last_accessed = 0; // Beginning of epoch
        store.store_manifest(inactive_flow_id, &inactive_manifest).await.unwrap();
        
        // 3. Run garbage collection with threshold that should catch inactive manifest
        let options = GarbageCollectionOptions {
            inactive_manifest_threshold_ms: 60 * 60 * 1000, // 1 hour
            dry_run: false,
            max_items: None,
        };
        
        let result = store.run_garbage_collection(options).await.unwrap();
        
        // Verify results
        assert_eq!(result.manifests_removed, 1); // Only the inactive manifest
        
        // Verify inactive manifest was deleted
        let inactive_result = store.get_manifest(inactive_flow_id).await;
        assert!(inactive_result.is_err());
        
        // Verify active manifest still exists
        let active_result = store.get_manifest(active_flow_id).await;
        assert!(active_result.is_ok());
    }
} 
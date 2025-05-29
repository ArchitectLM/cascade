//! In-memory implementation of the ContentStorage trait.

use crate::mocks::content_storage::{ContentHash, ContentStorage, Manifest, ManifestKey, StorageError};
use async_trait::async_trait;
use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::fmt;

/// Thread-safe in-memory content store implementation for testing.
#[derive(Clone)]
pub struct InMemoryContentStore {
    content_store: Arc<RwLock<HashMap<ContentHash, Value>>>,
    manifest_store: Arc<RwLock<HashMap<ManifestKey, Manifest>>>,
}

impl fmt::Debug for InMemoryContentStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InMemoryContentStore")
            .field("content_count", &self.content_store.read().len())
            .field("manifest_count", &self.manifest_store.read().len())
            .finish()
    }
}

impl InMemoryContentStore {
    /// Creates a new in-memory content store.
    pub fn new() -> Self {
        Self {
            content_store: Arc::new(RwLock::new(HashMap::new())),
            manifest_store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Creates a new in-memory content store with initial data.
    pub fn with_initial_data(
        content: HashMap<ContentHash, Value>,
        manifests: HashMap<ManifestKey, Manifest>,
    ) -> Self {
        Self {
            content_store: Arc::new(RwLock::new(content)),
            manifest_store: Arc::new(RwLock::new(manifests)),
        }
    }

    /// Gets direct access to the stored content for inspection in tests.
    pub fn get_content_store(&self) -> Arc<RwLock<HashMap<ContentHash, Value>>> {
        self.content_store.clone()
    }

    /// Gets direct access to the stored manifests for inspection in tests.
    pub fn get_manifest_store(&self) -> Arc<RwLock<HashMap<ManifestKey, Manifest>>> {
        self.manifest_store.clone()
    }

    /// Computes a content hash for the given content.
    fn compute_hash(&self, content: &Value) -> ContentHash {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        content.to_string().hash(&mut hasher);
        let hash_value = hasher.finish();
        
        format!("sha256:{:x}", hash_value)
    }

    /// Clears all stored content and manifests.
    pub fn clear(&self) {
        self.content_store.write().clear();
        self.manifest_store.write().clear();
    }
}

impl Default for InMemoryContentStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContentStorage for InMemoryContentStore {
    async fn store_content_addressed(&self, content: Value) -> Result<ContentHash, StorageError> {
        let hash = self.compute_hash(&content);
        self.content_store.write().insert(hash.clone(), content);
        Ok(hash)
    }

    async fn get_content_addressed(&self, hash: &ContentHash) -> Result<Option<Value>, StorageError> {
        Ok(self.content_store.read().get(hash).cloned())
    }

    async fn content_exists(&self, hash: &ContentHash) -> Result<bool, StorageError> {
        Ok(self.content_store.read().contains_key(hash))
    }

    async fn store_manifest(&self, key: &ManifestKey, manifest: Manifest) -> Result<(), StorageError> {
        self.manifest_store.write().insert(key.to_string(), manifest);
        Ok(())
    }

    async fn get_manifest(&self, key: &ManifestKey) -> Result<Option<Manifest>, StorageError> {
        Ok(self.manifest_store.read().get(key).cloned())
    }

    async fn delete_manifest(&self, key: &ManifestKey) -> Result<bool, StorageError> {
        Ok(self.manifest_store.write().remove(key).is_some())
    }

    async fn list_manifest_keys(&self, prefix: Option<&str>) -> Result<Vec<ManifestKey>, StorageError> {
        let store = self.manifest_store.read();
        
        match prefix {
            Some(prefix) => Ok(store
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect()),
            None => Ok(store.keys().cloned().collect()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_store_and_retrieve_content() {
        let store = InMemoryContentStore::new();
        let content = json!({"test": "data"});
        
        let hash = store.store_content_addressed(content.clone()).await.unwrap();
        let retrieved = store.get_content_addressed(&hash).await.unwrap().unwrap();
        
        assert_eq!(retrieved, content);
    }

    #[tokio::test]
    async fn test_content_exists() {
        let store = InMemoryContentStore::new();
        let content = json!({"test": "data"});
        
        let hash = store.store_content_addressed(content).await.unwrap();
        let exists = store.content_exists(&hash).await.unwrap();
        
        assert!(exists);
        
        let not_exists = store.content_exists(&"nonexistent".to_string()).await.unwrap();
        assert!(!not_exists);
    }

    #[tokio::test]
    async fn test_store_and_retrieve_manifest() {
        let store = InMemoryContentStore::new();
        let manifest = json!({"flow_id": "test-flow", "components": []});
        
        store.store_manifest(&"test-flow".to_string(), manifest.clone()).await.unwrap();
        let retrieved = store.get_manifest(&"test-flow".to_string()).await.unwrap().unwrap();
        
        assert_eq!(retrieved, manifest);
    }

    #[tokio::test]
    async fn test_delete_manifest() {
        let store = InMemoryContentStore::new();
        let manifest = json!({"flow_id": "test-flow", "components": []});
        
        store.store_manifest(&"test-flow".to_string(), manifest).await.unwrap();
        let deleted = store.delete_manifest(&"test-flow".to_string()).await.unwrap();
        
        assert!(deleted);
        
        let retrieved = store.get_manifest(&"test-flow".to_string()).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_list_manifest_keys() {
        let store = InMemoryContentStore::new();
        
        store.store_manifest(&"flow1".to_string(), json!({})).await.unwrap();
        store.store_manifest(&"flow2".to_string(), json!({})).await.unwrap();
        store.store_manifest(&"test1".to_string(), json!({})).await.unwrap();
        
        let all_keys = store.list_manifest_keys(None).await.unwrap();
        assert_eq!(all_keys.len(), 3);
        
        let flow_keys = store.list_manifest_keys(Some("flow")).await.unwrap();
        assert_eq!(flow_keys.len(), 2);
        assert!(flow_keys.contains(&"flow1".to_string()));
        assert!(flow_keys.contains(&"flow2".to_string()));
    }

    #[tokio::test]
    async fn test_with_initial_data() {
        let mut content = HashMap::new();
        content.insert("hash1".to_string(), json!({"initial": "content"}));
        
        let mut manifests = HashMap::new();
        manifests.insert("initial-flow".to_string(), json!({"initial": "manifest"}));
        
        let store = InMemoryContentStore::with_initial_data(content, manifests);
        
        let retrieved = store.get_content_addressed(&"hash1".to_string()).await.unwrap().unwrap();
        assert_eq!(retrieved, json!({"initial": "content"}));
        
        let manifest = store.get_manifest(&"initial-flow".to_string()).await.unwrap().unwrap();
        assert_eq!(manifest, json!({"initial": "manifest"}));
    }

    #[tokio::test]
    async fn test_clear() {
        let store = InMemoryContentStore::new();
        
        store.store_content_addressed(json!({"test": "data"})).await.unwrap();
        store.store_manifest(&"test-flow".to_string(), json!({})).await.unwrap();
        
        store.clear();
        
        let content_count = store.get_content_store().read().len();
        let manifest_count = store.get_manifest_store().read().len();
        
        assert_eq!(content_count, 0);
        assert_eq!(manifest_count, 0);
    }

    #[tokio::test]
    async fn test_thread_safety() {
        use tokio::task;
        
        let store = Arc::new(InMemoryContentStore::new());
        let mut handles = Vec::new();
        
        // Spawn tasks to write content concurrently
        for i in 0..10 {
            let store_clone = store.clone();
            let handle = task::spawn(async move {
                let content = json!({ "task": i });
                store_clone.store_content_addressed(content).await.unwrap()
            });
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        let hashes = futures::future::join_all(handles).await;
        
        // Verify all content was stored
        let content_count = store.get_content_store().read().len();
        assert_eq!(content_count, 10);
        
        // Verify all hashes can be retrieved
        for hash in hashes {
            let content = store.get_content_addressed(&hash.unwrap()).await.unwrap();
            assert!(content.is_some());
        }
    }
} 
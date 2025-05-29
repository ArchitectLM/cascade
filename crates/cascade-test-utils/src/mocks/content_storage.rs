//! Mock implementation of the ContentStorage trait.

use async_trait::async_trait;
use mockall::predicate::*;
use serde_json::Value;
use std::collections::HashMap;
use md5;

// Define the types needed for ContentStorage trait
pub type ContentHash = String;
pub type ManifestKey = String;
pub type Manifest = Value;

/// Define the ContentStorage trait interface based on the actual trait
#[async_trait]
pub trait ContentStorage: Send + Sync {
    async fn store_content_addressed(&self, content: Value) -> Result<ContentHash, StorageError>;
    async fn get_content_addressed(&self, hash: &ContentHash) -> Result<Option<Value>, StorageError>;
    async fn content_exists(&self, hash: &ContentHash) -> Result<bool, StorageError>;
    
    async fn store_manifest(&self, key: &ManifestKey, manifest: Manifest) -> Result<(), StorageError>;
    async fn get_manifest(&self, key: &ManifestKey) -> Result<Option<Manifest>, StorageError>;
    async fn delete_manifest(&self, key: &ManifestKey) -> Result<bool, StorageError>;
    async fn list_manifest_keys(&self, prefix: Option<&str>) -> Result<Vec<ManifestKey>, StorageError>;
}

/// Error type for storage operations
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Storage error: {0}")]
    Other(String),
}

/// Content Storage mock implementation
#[derive(Default)]
pub struct MockContentStorage {
    content_store: std::sync::Mutex<HashMap<ContentHash, Value>>,
    manifest_store: std::sync::Mutex<HashMap<ManifestKey, Manifest>>,
}

#[async_trait]
impl ContentStorage for MockContentStorage {
    async fn store_content_addressed(&self, content: Value) -> Result<ContentHash, StorageError> {
        let hash = format!("sha256:{:x}", md5::compute(content.to_string()));
        self.content_store.lock().unwrap().insert(hash.clone(), content);
        Ok(hash)
    }
    
    async fn get_content_addressed(&self, hash: &ContentHash) -> Result<Option<Value>, StorageError> {
        Ok(self.content_store.lock().unwrap().get(hash).cloned())
    }
    
    async fn content_exists(&self, hash: &ContentHash) -> Result<bool, StorageError> {
        Ok(self.content_store.lock().unwrap().contains_key(hash))
    }
    
    async fn store_manifest(&self, key: &ManifestKey, manifest: Manifest) -> Result<(), StorageError> {
        self.manifest_store.lock().unwrap().insert(key.clone(), manifest);
        Ok(())
    }
    
    async fn get_manifest(&self, key: &ManifestKey) -> Result<Option<Manifest>, StorageError> {
        Ok(self.manifest_store.lock().unwrap().get(key).cloned())
    }
    
    async fn delete_manifest(&self, key: &ManifestKey) -> Result<bool, StorageError> {
        Ok(self.manifest_store.lock().unwrap().remove(key).is_some())
    }
    
    async fn list_manifest_keys(&self, prefix: Option<&str>) -> Result<Vec<ManifestKey>, StorageError> {
        let keys = self.manifest_store.lock().unwrap().keys().cloned().collect::<Vec<_>>();
        
        if let Some(prefix) = prefix {
            Ok(keys.into_iter().filter(|k| k.starts_with(prefix)).collect())
        } else {
            Ok(keys)
        }
    }
}

/// Creates a new mock ContentStorage instance.
pub fn create_mock_content_storage() -> MockContentStorage {
    MockContentStorage::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_mock_content_storage_default_behavior() {
        let mock = create_mock_content_storage();
        
        let exists = mock.content_exists(&"nonexistent".to_string()).await.unwrap();
        assert!(!exists);
        
        let keys = mock.list_manifest_keys(None).await.unwrap();
        assert!(keys.is_empty());
    }
    
    #[tokio::test]
    async fn test_mock_content_storage_custom_behavior() {
        let mock = create_mock_content_storage();
        
        // Store content and verify
        let hash = mock.store_content_addressed(json!({"test": "data"})).await.unwrap();
        assert!(hash.starts_with("sha256:"));
        
        // Retrieve the content
        let content = mock.get_content_addressed(&hash).await.unwrap().unwrap();
        assert_eq!(content, json!({"test": "data"}));
        
        // Verify content exists
        let exists = mock.content_exists(&hash).await.unwrap();
        assert!(exists);
    }
} 
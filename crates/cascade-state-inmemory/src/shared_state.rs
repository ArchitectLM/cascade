//! In-memory implementation of the SharedStateService interface
//!
//! This module provides an in-memory implementation of the shared state service
//! primarily used for development and testing purposes.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use async_trait::async_trait;
use serde_json::Value;
use tracing::{debug, info};

use cascade_core::{
    CoreError,
    SharedStateService,
};

/// A value with optional expiration time
struct ValueWithExpiry {
    /// The stored JSON value
    value: Value,
    /// Optional expiration time
    expires_at: Option<SystemTime>,
}

/// In-memory implementation of SharedStateService
pub struct InMemorySharedStateService {
    /// Map of scope -> (key -> value with expiry)
    state: Arc<RwLock<HashMap<String, HashMap<String, ValueWithExpiry>>>>,
}

impl InMemorySharedStateService {
    /// Create a new in-memory shared state service
    pub fn new() -> Self {
        info!("Creating new InMemorySharedStateService");
        let state = Arc::new(RwLock::new(HashMap::new()));
        
        // Start the background cleanup task
        Self::start_cleanup_task(state.clone());
        
        Self { state }
    }

    /// Clean up expired entries in the background
    fn start_cleanup_task(state: Arc<RwLock<HashMap<String, HashMap<String, ValueWithExpiry>>>>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // Run every minute
            loop {
                interval.tick().await;
                
                // Acquire write lock and clean up expired entries
                let now = SystemTime::now();
                let mut expired_keys = Vec::new();
                
                // First, collect all expired keys
                {
                    let state_read = state.read().await;
                    for (scope_key, scope_map) in state_read.iter() {
                        for (key, value_with_expiry) in scope_map.iter() {
                            if let Some(expires_at) = value_with_expiry.expires_at {
                                if now >= expires_at {
                                    expired_keys.push((scope_key.clone(), key.clone()));
                                }
                            }
                        }
                    }
                }
                
                // Then remove them (if any)
                if !expired_keys.is_empty() {
                    let mut state_write = state.write().await;
                    for (scope_key, key) in expired_keys {
                        if let Some(scope_map) = state_write.get_mut(&scope_key) {
                            scope_map.remove(&key);
                            debug!("Removed expired key from shared state: scope={}, key={}", scope_key, key);
                        }
                    }
                }
            }
        });
    }
}

impl Default for InMemorySharedStateService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SharedStateService for InMemorySharedStateService {
    async fn get_shared_state(&self, scope_key: &str, key: &str) -> Result<Option<Value>, CoreError> {
        let state = self.state.read().await;
        
        match state.get(scope_key) {
            Some(scope_map) => {
                match scope_map.get(key) {
                    Some(value_with_expiry) => {
                        // Check if value has expired
                        if let Some(expires_at) = value_with_expiry.expires_at {
                            if SystemTime::now() >= expires_at {
                                // Value has expired, pretend it doesn't exist
                                return Ok(None);
                            }
                        }
                        Ok(Some(value_with_expiry.value.clone()))
                    },
                    None => Ok(None),
                }
            },
            None => Ok(None),
        }
    }
    
    async fn set_shared_state(&self, scope_key: &str, key: &str, value: Value) -> Result<(), CoreError> {
        let mut state = self.state.write().await;
        
        // Get or create scope map
        let scope_map = state.entry(scope_key.to_string()).or_insert_with(HashMap::new);
        
        // Insert value with no expiry
        scope_map.insert(key.to_string(), ValueWithExpiry {
            value,
            expires_at: None,
        });
        
        debug!("Set shared state for scope={}, key={}", scope_key, key);
        Ok(())
    }
    
    async fn set_shared_state_with_ttl(&self, scope_key: &str, key: &str, value: Value, ttl_ms: u64) -> Result<(), CoreError> {
        let mut state = self.state.write().await;
        
        // Get or create scope map
        let scope_map = state.entry(scope_key.to_string()).or_insert_with(HashMap::new);
        
        // Calculate expiration time
        let expires_at = SystemTime::now() + Duration::from_millis(ttl_ms);
        
        // Insert value with expiry
        scope_map.insert(key.to_string(), ValueWithExpiry {
            value,
            expires_at: Some(expires_at),
        });
        
        debug!("Set shared state with TTL for scope={}, key={}, ttl={}ms", scope_key, key, ttl_ms);
        Ok(())
    }
    
    async fn delete_shared_state(&self, scope_key: &str, key: &str) -> Result<(), CoreError> {
        let mut state = self.state.write().await;
        
        if let Some(scope_map) = state.get_mut(scope_key) {
            scope_map.remove(key);
            debug!("Deleted shared state for scope={}, key={}", scope_key, key);
        }
        
        Ok(())
    }
    
    async fn list_shared_state_keys(&self, scope_key: &str) -> Result<Vec<String>, CoreError> {
        let state = self.state.read().await;
        let now = SystemTime::now();
        
        match state.get(scope_key) {
            Some(scope_map) => {
                // Filter out expired keys
                let mut keys: Vec<String> = Vec::new();
                for (key, value_with_expiry) in scope_map.iter() {
                    if let Some(expires_at) = value_with_expiry.expires_at {
                        if now < expires_at {
                            keys.push(key.clone());
                        }
                    } else {
                        keys.push(key.clone());
                    }
                }
                Ok(keys)
            },
            None => Ok(Vec::new()),
        }
    }
    
    async fn get_metrics(&self) -> Result<HashMap<String, i64>, CoreError> {
        let state = self.state.read().await;
        let now = SystemTime::now();
        
        let mut metrics = HashMap::new();
        let mut total_keys = 0;
        let mut expired_keys = 0;
        
        // Count scopes and keys
        metrics.insert("scopes".to_string(), state.len() as i64);
        
        for scope_map in state.values() {
            for value_with_expiry in scope_map.values() {
                total_keys += 1;
                if let Some(expires_at) = value_with_expiry.expires_at {
                    if now >= expires_at {
                        expired_keys += 1;
                    }
                }
            }
        }
        
        metrics.insert("total_keys".to_string(), total_keys as i64);
        metrics.insert("active_keys".to_string(), (total_keys - expired_keys) as i64);
        metrics.insert("expired_keys".to_string(), expired_keys as i64);
        
        Ok(metrics)
    }
    
    async fn health_check(&self) -> Result<bool, CoreError> {
        // In-memory service is always healthy
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_basic_operations() {
        let service = InMemorySharedStateService::new();
        
        // Test empty state
        let keys = service.list_shared_state_keys("test_scope").await.unwrap();
        assert!(keys.is_empty());
        
        // Test set and get
        let test_value = json!({"test": "value"});
        service.set_shared_state("test_scope", "test_key", test_value.clone()).await.unwrap();
        
        let result = service.get_shared_state("test_scope", "test_key").await.unwrap();
        assert_eq!(result, Some(test_value));
        
        // Test list keys
        let keys = service.list_shared_state_keys("test_scope").await.unwrap();
        assert_eq!(keys, vec!["test_key".to_string()]);
        
        // Test delete
        service.delete_shared_state("test_scope", "test_key").await.unwrap();
        let result = service.get_shared_state("test_scope", "test_key").await.unwrap();
        assert_eq!(result, None);
    }
    
    #[tokio::test]
    async fn test_ttl_functionality() {
        let service = InMemorySharedStateService::new();
        
        // Set a value with a short TTL (100ms)
        let test_value = json!({"test": "value"});
        service.set_shared_state_with_ttl("test_scope", "ttl_key", test_value.clone(), 100).await.unwrap();
        
        // Verify it exists immediately
        let result = service.get_shared_state("test_scope", "ttl_key").await.unwrap();
        assert_eq!(result, Some(test_value));
        
        // Wait for TTL to expire
        sleep(Duration::from_millis(200)).await;
        
        // Verify it's gone after expiry
        let result = service.get_shared_state("test_scope", "ttl_key").await.unwrap();
        assert_eq!(result, None);
        
        // Verify it's not listed in keys
        let keys = service.list_shared_state_keys("test_scope").await.unwrap();
        assert!(!keys.contains(&"ttl_key".to_string()));
        
        // Check metrics
        let metrics = service.get_metrics().await.unwrap();
        assert!(metrics.contains_key("expired_keys"));
    }
} 
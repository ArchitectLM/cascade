//! Shared state service for the Cascade Platform
//!
//! This module provides interfaces for shared state that can be used
//! across component instances and flows to coordinate resilience patterns
//! and other stateful operations.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;

#[allow(unused_imports)]
use std::sync::Mutex;

use crate::CoreError;

/// A service that provides access to state shared across component instances
#[async_trait]
pub trait SharedStateService: Send + Sync {
    /// Get a shared state value for a given scope and key
    async fn get_shared_state(
        &self,
        scope_key: &str,
        key: &str,
    ) -> Result<Option<Value>, CoreError>;

    /// Set a shared state value for a given scope and key
    async fn set_shared_state(
        &self,
        scope_key: &str,
        key: &str,
        value: Value,
    ) -> Result<(), CoreError>;

    /// Set a shared state value for a given scope and key with a time-to-live
    async fn set_shared_state_with_ttl(
        &self,
        scope_key: &str,
        key: &str,
        value: Value,
        _ttl_ms: u64,
    ) -> Result<(), CoreError> {
        // Default implementation just calls set_shared_state and ignores TTL
        tracing::debug!("Using default set_shared_state_with_ttl implementation (ignores TTL)");
        self.set_shared_state(scope_key, key, value).await
    }

    /// Delete a shared state value for a given scope and key
    async fn delete_shared_state(&self, scope_key: &str, key: &str) -> Result<(), CoreError>;

    /// List all keys in a scope
    async fn list_shared_state_keys(&self, scope_key: &str) -> Result<Vec<String>, CoreError>;

    /// Get metrics about the shared state service (counts, etc.)
    async fn get_metrics(&self) -> Result<HashMap<String, i64>, CoreError> {
        // Default implementation returns empty metrics
        tracing::debug!("Using default get_metrics implementation (returns empty metrics)");
        Ok(HashMap::new())
    }

    /// Health check for the service
    async fn health_check(&self) -> Result<bool, CoreError> {
        // Default implementation that always returns true
        Ok(true)
    }
}

/// Value object representing a shared state scope
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SharedStateScope {
    /// Name of the scope
    pub name: String,

    /// Optional namespace for additional scoping
    pub namespace: Option<String>,
}

impl SharedStateScope {
    /// Create a new shared state scope
    pub fn new(name: impl Into<String>, namespace: Option<impl Into<String>>) -> Self {
        Self {
            name: name.into(),
            namespace: namespace.map(|n| n.into()),
        }
    }

    /// Convert the scope to a string key for storage
    pub fn to_key(&self) -> String {
        match &self.namespace {
            Some(ns) => format!("{}:{}", ns, self.name),
            None => self.name.clone(),
        }
    }

    /// Parse a scope from a string key
    pub fn from_key(key: &str) -> Self {
        if let Some(pos) = key.find(':') {
            let (namespace, name) = key.split_at(pos);
            Self {
                name: name[1..].to_string(), // Skip the ':' character
                namespace: Some(namespace.to_string()),
            }
        } else {
            Self {
                name: key.to_string(),
                namespace: None,
            }
        }
    }
}

/// Factory function type for creating shared state services
pub type SharedStateServiceFactory =
    Box<dyn Fn(&str) -> Result<Box<dyn SharedStateService>, CoreError> + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_shared_state_scope_new() {
        // Test with name only
        let scope = SharedStateScope::new("test-scope", None::<String>);
        assert_eq!(scope.name, "test-scope");
        assert_eq!(scope.namespace, None);

        // Test with name and namespace
        let scope = SharedStateScope::new("test-scope", Some("test-namespace"));
        assert_eq!(scope.name, "test-scope");
        assert_eq!(scope.namespace, Some("test-namespace".to_string()));
    }

    #[test]
    fn test_shared_state_scope_to_key() {
        // Test with name only
        let scope = SharedStateScope::new("scope1", None::<String>);
        assert_eq!(scope.to_key(), "scope1");

        // Test with name and namespace
        let scope = SharedStateScope::new("scope2", Some("namespace1"));
        assert_eq!(scope.to_key(), "namespace1:scope2");
    }

    #[test]
    fn test_shared_state_scope_from_key() {
        // Test with name only
        let scope = SharedStateScope::from_key("simple-key");
        assert_eq!(scope.name, "simple-key");
        assert_eq!(scope.namespace, None);

        // Test with name and namespace
        let scope = SharedStateScope::from_key("ns:scoped-key");
        assert_eq!(scope.name, "scoped-key");
        assert_eq!(scope.namespace, Some("ns".to_string()));
    }

    // Mock implementation of SharedStateService for testing
    struct MockSharedStateService {
        store: Mutex<HashMap<String, HashMap<String, Value>>>,
    }

    impl MockSharedStateService {
        fn new() -> Self {
            Self {
                store: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl SharedStateService for MockSharedStateService {
        async fn get_shared_state(
            &self,
            scope_key: &str,
            key: &str,
        ) -> Result<Option<Value>, CoreError> {
            let store = self.store.lock().unwrap();
            Ok(store
                .get(scope_key)
                .and_then(|scope| scope.get(key).cloned()))
        }

        async fn set_shared_state(
            &self,
            scope_key: &str,
            key: &str,
            value: Value,
        ) -> Result<(), CoreError> {
            let mut store = self.store.lock().unwrap();
            let scope = store.entry(scope_key.to_string()).or_default();
            scope.insert(key.to_string(), value);
            Ok(())
        }

        async fn delete_shared_state(&self, scope_key: &str, key: &str) -> Result<(), CoreError> {
            let mut store = self.store.lock().unwrap();
            if let Some(scope) = store.get_mut(scope_key) {
                scope.remove(key);
            }
            Ok(())
        }

        async fn list_shared_state_keys(&self, scope_key: &str) -> Result<Vec<String>, CoreError> {
            let store = self.store.lock().unwrap();
            Ok(store
                .get(scope_key)
                .map(|scope| scope.keys().cloned().collect())
                .unwrap_or_default())
        }
    }

    #[tokio::test]
    async fn test_shared_state_service_default_methods() {
        let service = MockSharedStateService::new();
        
        // Test default implementation of set_shared_state_with_ttl
        service
            .set_shared_state_with_ttl("scope", "key", json!("value"), 10000)
            .await
            .unwrap();
            
        // Verify it called the regular set_shared_state
        let result = service.get_shared_state("scope", "key").await.unwrap();
        assert_eq!(result, Some(json!("value")));
        
        // Test default metrics implementation
        let metrics = service.get_metrics().await.unwrap();
        assert!(metrics.is_empty());
        
        // Test default health check implementation
        let health = service.health_check().await.unwrap();
        assert!(health);
    }
}

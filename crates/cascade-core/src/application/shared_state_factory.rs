//! Factory for creating shared state services
//!
//! This module provides a factory function for creating shared state services
//! based on configuration.

use std::sync::Arc;
use tracing::{error, info};

use crate::domain::shared_state::SharedStateService;
use crate::CoreError;

/// Create a shared state service factory function
///
/// This function returns a function that can create shared state services
/// for different providers based on the URL prefix.
///
/// # Arguments
///
/// * `in_memory_factory` - Function to create an in-memory shared state service
///
/// # Returns
///
/// A function that takes a URL and returns a shared state service
pub fn create_shared_state_factory<F, S>(
    in_memory_factory: F,
) -> impl Fn(&str) -> Result<Arc<dyn SharedStateService>, CoreError> + Send + Sync
where
    F: Fn() -> S + Send + Sync + 'static,
    S: SharedStateService + 'static,
{
    move |url: &str| -> Result<Arc<dyn SharedStateService>, CoreError> {
        if url.starts_with("memory://") {
            info!("Creating in-memory shared state service");
            let service = in_memory_factory();
            Ok(Arc::new(service))
        } else {
            error!("Unsupported shared state service URL: {}", url);
            Err(CoreError::ConfigurationError(format!(
                "Unsupported shared state service URL: {}",
                url
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::Value;

    // Simple mock impl that always returns success but doesn't do anything
    struct NoOpSharedStateService;

    #[async_trait]
    impl SharedStateService for NoOpSharedStateService {
        async fn get_shared_state(
            &self,
            _scope_key: &str,
            _key: &str,
        ) -> Result<Option<Value>, CoreError> {
            Ok(None)
        }

        async fn set_shared_state(
            &self,
            _scope_key: &str,
            _key: &str,
            _value: Value,
        ) -> Result<(), CoreError> {
            Ok(())
        }

        async fn delete_shared_state(&self, _scope_key: &str, _key: &str) -> Result<(), CoreError> {
            Ok(())
        }

        async fn list_shared_state_keys(&self, _scope_key: &str) -> Result<Vec<String>, CoreError> {
            Ok(vec![])
        }
    }

    #[test]
    fn test_create_shared_state_factory_memory_url() {
        // Create a factory function that creates our mock service
        let factory = create_shared_state_factory(|| NoOpSharedStateService);

        // Test with a memory URL
        let result = factory("memory://test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_shared_state_factory_unsupported_url() {
        // Create a factory function that creates our mock service
        let factory = create_shared_state_factory(|| NoOpSharedStateService);

        // Test with an unsupported URL
        let result = factory("redis://localhost:6379");
        assert!(result.is_err());

        // Verify the error type
        match result {
            Err(CoreError::ConfigurationError(msg)) => {
                assert!(msg.contains("redis://localhost:6379"));
            }
            _ => panic!("Expected ConfigurationError"),
        }
    }
}

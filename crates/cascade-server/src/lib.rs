//! 
//! Cascade Server - Main application server for the Cascade Platform
//!
//! This module exports all the components of the Cascade Server.

// External dependencies
use std::sync::Arc;

/// API module
pub mod api;

/// Server module
pub mod server;

/// Content store client module
pub mod content_store;

/// Edge platform client module
pub mod edge;

/// Edge manager module
pub mod edge_manager;

/// Configuration module
pub mod config;

/// Error module
pub mod error;

/// Server factory module
pub mod server_factory;

/// Shared state module
pub mod shared_state;

/// Resilience module
pub mod resilience;

// Re-export key types
pub use config::ServerConfig;
pub use server::CascadeServer;
pub use error::{ServerError, ServerResult};
pub use edge_manager::EdgeManager;
pub use shared_state::SharedStateService;
pub use resilience::{RateLimiter, CircuitBreaker};

/// Run function
pub async fn run(config: ServerConfig) -> ServerResult<()> {
    // Initialize logging
    init_logging(&config);
    
    // Create dependencies
    let content_store = create_content_store(&config)?;
    let edge_platform = create_edge_platform(&config)?;
    let shared_state = create_shared_state_service(&config)?;
    let core_runtime = create_core_runtime(Some(shared_state.clone()))?;
    
    // Create edge manager
    let edge_manager = EdgeManager::new(
        edge_platform.clone(),
        content_store.clone(),
        format!("http://{}:{}/api/v1/edge/callback", config.bind_address, config.port),
        Some(config.edge_callback_jwt_secret.clone().unwrap_or_default()),
        Some(config.edge_callback_jwt_issuer.clone()),
        Some(config.edge_callback_jwt_audience.clone()),
        config.edge_callback_jwt_expiry_seconds,
    );
    
    // Create server
    let server = CascadeServer::new(
        config,
        content_store,
        edge_platform,
        edge_manager,
        core_runtime,
        shared_state,
    );
    
    // Run server
    server.run().await
}

/// Initialize logging
fn init_logging(config: &ServerConfig) {
    use tracing_subscriber::{fmt, EnvFilter};
    
    // Create filter based on config
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_level));
    
    // Initialize subscriber
    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}

/// Create content store client
fn create_content_store(config: &ServerConfig) -> ServerResult<Arc<dyn cascade_content_store::ContentStorage>> {
    let base_store: Arc<dyn cascade_content_store::ContentStorage> = if config.content_store_url.starts_with("memory://") {
        // Use in-memory content store for development and testing
        tracing::info!("Using in-memory content store");
        let store = cascade_content_store::memory::InMemoryContentStore::new();
        Arc::new(store)
    } else if config.content_store_url.starts_with("cloudflare://") {
        // Parse the URL for Cloudflare KV configuration
        // Format: cloudflare://{account_id}:{namespace_id}?token={token}
        
        // URL format validation and extraction
        let url = &config.content_store_url["cloudflare://".len()..];
        let mut parts = url.splitn(2, '?');
        
        let ids_part = parts.next().ok_or_else(|| 
            ServerError::ConfigurationError("Invalid Cloudflare KV URL format".to_string())
        )?;
        
        let mut ids_parts = ids_part.splitn(2, ':');
        let account_id = ids_parts.next().ok_or_else(|| 
            ServerError::ConfigurationError("Missing Cloudflare account ID in KV URL".to_string())
        )?;
        
        let namespace_id = ids_parts.next().ok_or_else(|| 
            ServerError::ConfigurationError("Missing Cloudflare namespace ID in KV URL".to_string())
        )?;
        
        // Get API token from the query string or from the config
        let api_token = if let Some(query) = parts.next() {
            let token_prefix = "token=";
            if let Some(token) = query.strip_prefix(token_prefix) {
                token.to_string()
            } else {
                config.cloudflare_api_token.clone().ok_or_else(|| 
                    ServerError::ConfigurationError("Missing Cloudflare API token".to_string())
                )?
            }
        } else {
            config.cloudflare_api_token.clone().ok_or_else(|| 
                ServerError::ConfigurationError("Missing Cloudflare API token".to_string())
            )?
        };
        
        // Create the Cloudflare KV store
        tracing::info!("Using Cloudflare KV store with account {}", account_id);
        let store = cascade_content_store::cloudflare::CloudflareKvStore::new(
            account_id.to_string(),
            namespace_id.to_string(),
            api_token,
        );
        
        Arc::new(store)
    } else if config.content_store_url.starts_with("redis://") {
        // TODO: Implement Redis content store
        return Err(ServerError::ConfigurationError("Redis content store not yet implemented".to_string()));
    } else {
        return Err(ServerError::ConfigurationError(format!(
            "Unsupported content store URL: {}", config.content_store_url
        )));
    };
    
    // Wrap with cache if configured
    if let Some(cache_config) = &config.content_cache_config {
        tracing::info!("Enabling content cache with max_items={}, max_size={}MB", 
                      cache_config.max_items, cache_config.max_size_bytes / (1024 * 1024));
        let cached_store = content_store::CachedContentStore::with_config(base_store, cache_config.clone());
        Ok(Arc::new(cached_store))
    } else {
        tracing::info!("Content cache disabled");
        Ok(base_store)
    }
}

/// Create edge platform client
fn create_edge_platform(config: &ServerConfig) -> ServerResult<Arc<dyn edge::EdgePlatform>> {
    // Parse the edge_api_url to determine platform type
    if config.edge_api_url.starts_with("cloudflare://") {
        // Extract Cloudflare account ID from the URL
        // Format: cloudflare://{account_id}?token={token}
        
        // URL format validation and extraction
        let url = &config.edge_api_url["cloudflare://".len()..];
        let mut parts = url.splitn(2, '?');
        
        let account_id = parts.next().ok_or_else(|| 
            ServerError::ConfigurationError("Invalid Cloudflare API URL format".to_string())
        )?;
        
        // Get API token from the query string or from the config
        let api_token = if let Some(query) = parts.next() {
            let token_prefix = "token=";
            if let Some(token) = query.strip_prefix(token_prefix) {
                token.to_string()
            } else {
                config.cloudflare_api_token.clone().ok_or_else(|| 
                    ServerError::ConfigurationError("Missing Cloudflare API token".to_string())
                )?
            }
        } else {
            config.cloudflare_api_token.clone().ok_or_else(|| 
                ServerError::ConfigurationError("Missing Cloudflare API token".to_string())
            )?
        };
        
        tracing::info!("Using Cloudflare Edge Platform with account {}", account_id);
        let platform = edge::cloudflare::CloudflareEdgePlatform::new(account_id.to_string(), api_token);
        Ok(Arc::new(platform))
    } else {
        // For backward compatibility, use mock values
        tracing::warn!("Using default edge platform, the URL format is not recognized: {}", config.edge_api_url);
        let account_id = "placeholder_account_id".to_string();
        let api_token = "placeholder_api_token".to_string();
        
        let platform = edge::cloudflare::CloudflareEdgePlatform::new(account_id, api_token);
        Ok(Arc::new(platform))
    }
}

// Define MockRuntime at the module level
struct MockRuntime {
    shared_state: Option<Arc<dyn shared_state::SharedStateService>>,
}

impl cascade_core::ComponentRuntimeApiBase for MockRuntime {
    fn log(&self, level: tracing::Level, message: &str) {
        match level {
            tracing::Level::ERROR => tracing::error!("[COMPONENT] {}", message),
            tracing::Level::WARN => tracing::warn!("[COMPONENT] {}", message),
            tracing::Level::INFO => tracing::info!("[COMPONENT] {}", message),
            tracing::Level::DEBUG => tracing::debug!("[COMPONENT] {}", message),
            tracing::Level::TRACE => tracing::trace!("[COMPONENT] {}", message),
        }
    }
}

#[async_trait::async_trait]
impl cascade_core::ComponentRuntimeApi for MockRuntime {
    async fn get_input(&self, name: &str) -> Result<cascade_core::DataPacket, cascade_core::CoreError> {
        tracing::debug!("Mock runtime: get_input({})", name);
        Err(cascade_core::CoreError::ComponentError("Mock runtime does not implement get_input".to_string()))
    }
    
    async fn get_config(&self, name: &str) -> Result<serde_json::Value, cascade_core::CoreError> {
        tracing::debug!("Mock runtime: get_config({})", name);
        Err(cascade_core::CoreError::ComponentError("Mock runtime does not implement get_config".to_string()))
    }
    
    async fn set_output(&self, name: &str, value: cascade_core::DataPacket) -> Result<(), cascade_core::CoreError> {
        tracing::debug!("Mock runtime: set_output({}, {:?})", name, value);
        Ok(())
    }
    
    async fn get_state(&self) -> Result<Option<serde_json::Value>, cascade_core::CoreError> {
        tracing::debug!("Mock runtime: get_state()");
        Ok(None)
    }
    
    async fn save_state(&self, state: serde_json::Value) -> Result<(), cascade_core::CoreError> {
        tracing::debug!("Mock runtime: save_state({:?})", state);
        Ok(())
    }
    
    async fn schedule_timer(&self, duration: std::time::Duration) -> Result<(), cascade_core::CoreError> {
        tracing::debug!("Mock runtime: schedule_timer({:?})", duration);
        Ok(())
    }
    
    async fn correlation_id(&self) -> Result<cascade_core::CorrelationId, cascade_core::CoreError> {
        tracing::debug!("Mock runtime: correlation_id()");
        Ok(cascade_core::CorrelationId("mock-correlation".to_string()))
    }
    
    async fn log(&self, level: cascade_core::LogLevel, message: &str) -> Result<(), cascade_core::CoreError> {
        match level {
            cascade_core::LogLevel::Error => tracing::error!("[COMPONENT] {}", message),
            cascade_core::LogLevel::Warn => tracing::warn!("[COMPONENT] {}", message),
            cascade_core::LogLevel::Info => tracing::info!("[COMPONENT] {}", message),
            cascade_core::LogLevel::Debug => tracing::debug!("[COMPONENT] {}", message),
            cascade_core::LogLevel::Trace => tracing::trace!("[COMPONENT] {}", message),
        }
        Ok(())
    }
    
    async fn emit_metric(&self, name: &str, value: f64, labels: std::collections::HashMap<String, String>) -> Result<(), cascade_core::CoreError> {
        tracing::debug!("Mock runtime: emit_metric({}, {}, {:?})", name, value, labels);
        Ok(())
    }
    
    async fn get_shared_state(&self, scope_key: &str, key: &str) -> Result<Option<serde_json::Value>, cascade_core::CoreError> {
        tracing::debug!("Mock runtime: get_shared_state({}, {})", scope_key, key);
        
        if let Some(service) = &self.shared_state {
            service.get_state(scope_key, key).await.map_err(|e| {
                cascade_core::CoreError::StateStoreError(format!("Failed to get shared state: {}", e))
            })
        } else {
            tracing::debug!("No shared state service available");
            Ok(None)
        }
    }
    
    async fn set_shared_state(&self, scope_key: &str, key: &str, value: serde_json::Value) -> Result<(), cascade_core::CoreError> {
        tracing::debug!("Mock runtime: set_shared_state({}, {}, {:?})", scope_key, key, value);
        
        if let Some(service) = &self.shared_state {
            service.set_state(scope_key, key, value).await.map_err(|e| {
                cascade_core::CoreError::StateStoreError(format!("Failed to set shared state: {}", e))
            })
        } else {
            tracing::debug!("No shared state service available");
            Ok(())
        }
    }
    
    async fn delete_shared_state(&self, scope_key: &str, key: &str) -> Result<(), cascade_core::CoreError> {
        tracing::debug!("Mock runtime: delete_shared_state({}, {})", scope_key, key);
        
        if let Some(service) = &self.shared_state {
            service.delete_state(scope_key, key).await.map_err(|e| {
                cascade_core::CoreError::StateStoreError(format!("Failed to delete shared state: {}", e))
            })
        } else {
            tracing::debug!("No shared state service available");
            Ok(())
        }
    }
    
    async fn list_shared_state_keys(&self, scope_key: &str) -> Result<Vec<String>, cascade_core::CoreError> {
        tracing::debug!("Mock runtime: list_shared_state_keys({})", scope_key);
        
        if let Some(service) = &self.shared_state {
            service.list_keys(scope_key).await.map_err(|e| {
                cascade_core::CoreError::StateStoreError(format!("Failed to list shared state keys: {}", e))
            })
        } else {
            tracing::debug!("No shared state service available");
            Ok(Vec::new())
        }
    }
}

/// Create core runtime client
pub fn create_core_runtime(shared_state: Option<Arc<dyn shared_state::SharedStateService>>) -> ServerResult<Arc<dyn cascade_core::ComponentRuntimeApi>> {
    // Try to create an embedded runtime first, and if that fails, use the mock runtime
    let runtime = match create_embedded_runtime() {
        Ok(runtime) => runtime,
        Err(_) => create_mock_runtime(shared_state)?,
    };
    
    Ok(Arc::new(runtime))
}

// Create the real embedded runtime from cascade-core
fn create_embedded_runtime() -> anyhow::Result<MockRuntime> {
    // This is a placeholder - replace with actual cascade-core runtime creation
    // when the crate provides that functionality
    
    // For now, fail deliberately to fall back to mock
    Err(anyhow::anyhow!("Embedded runtime not yet implemented"))
}

/// Create shared state service
pub fn create_shared_state_service(config: &ServerConfig) -> ServerResult<Arc<dyn shared_state::SharedStateService>> {
    if config.shared_state_url.starts_with("memory://") {
        // Use in-memory shared state for development and testing
        tracing::info!("Using in-memory shared state service");
        let store = shared_state::InMemorySharedStateService::new();
        return Ok(Arc::new(store));
    } 
    #[cfg(feature = "redis")]
    if config.shared_state_url.starts_with("redis://") {
        // Use Redis shared state
        tracing::info!("Using Redis shared state service at {}", config.shared_state_url);
        let store = shared_state::redis::RedisSharedStateService::new(&config.shared_state_url)
            .map_err(|e| ServerError::StateServiceError(e.to_string()))?;
        return Ok(Arc::new(store));
    } 
    
    Err(ServerError::ConfigurationError(format!(
        "Unsupported shared state URL: {}", config.shared_state_url
    )))
}

// Create a mock runtime for testing
fn create_mock_runtime(shared_state: Option<Arc<dyn shared_state::SharedStateService>>) -> ServerResult<MockRuntime> {
    // Create a mock runtime for initial deployment
    tracing::info!("Creating mock runtime");
    
    Ok(MockRuntime { shared_state })
} 
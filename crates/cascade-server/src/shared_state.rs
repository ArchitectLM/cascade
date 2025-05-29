//! Shared state service for the Cascade Platform
//!
//! This module provides a pluggable shared state service that can be used
//! to store and retrieve state that is shared across flow instances.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time;
use tracing::{debug, error, info};
use serde_json::{json, Value};

use cascade_core::CoreError;
use crate::error::{ServerError, ServerResult};

/// A service that provides access to state shared across flow instances
#[async_trait]
pub trait SharedStateService: Send + Sync {
    /// Get a shared state value for a given scope and key
    async fn get_state(&self, scope_key: &str, key: &str) -> Result<Option<Value>, CoreError>;
    
    /// Set a shared state value for a given scope and key
    async fn set_state(&self, scope_key: &str, key: &str, value: Value) -> Result<(), CoreError>;
    
    /// Set a shared state value with a TTL (Time-To-Live) in milliseconds
    async fn set_state_with_ttl(&self, scope_key: &str, key: &str, value: Value, ttl_ms: u64) -> Result<(), CoreError>;
    
    /// Delete a shared state value for a given scope and key
    async fn delete_state(&self, scope_key: &str, key: &str) -> Result<(), CoreError>;
    
    /// List all keys in a scope
    async fn list_keys(&self, scope_key: &str) -> Result<Vec<String>, CoreError>;
    
    /// Get metrics about the shared state service usage
    async fn get_metrics(&self) -> Result<Value, CoreError> {
        // Default implementation that returns basic metrics
        Ok(json!({
            "type": "unknown",
            "scopes": 0,
            "keys": 0
        }))
    }
    
    /// Health check
    async fn health_check(&self) -> Result<bool, CoreError> {
        // Default implementation that always returns true
        Ok(true)
    }
}

/// Entry in the in-memory shared state store
struct StateEntry {
    /// The stored value
    value: Value,
    
    /// Optional expiration time (if TTL was set)
    expires_at: Option<Instant>,
}

impl StateEntry {
    /// Create a new state entry without TTL
    fn new(value: Value) -> Self {
        Self {
            value,
            expires_at: None,
        }
    }
    
    /// Create a new state entry with TTL
    fn with_ttl(value: Value, ttl_ms: u64) -> Self {
        Self {
            value,
            expires_at: Some(Instant::now() + Duration::from_millis(ttl_ms)),
        }
    }
    
    /// Check if this entry has expired
    fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expiry) => Instant::now() >= expiry,
            None => false,
        }
    }
}

/// In-memory implementation of SharedStateService
pub struct InMemorySharedStateService {
    /// Map of scope -> (key -> entry)
    state: Arc<RwLock<HashMap<String, HashMap<String, StateEntry>>>>,
    
    /// Flag indicating if cleanup task is running
    is_cleanup_running: Arc<tokio::sync::Mutex<bool>>,
}

impl InMemorySharedStateService {
    /// Create a new in-memory shared state service
    pub fn new() -> Self {
        info!("Creating new InMemorySharedStateService");
        let state = Arc::new(RwLock::new(HashMap::new()));
        let is_cleanup_running = Arc::new(tokio::sync::Mutex::new(false));
        
        let instance = Self {
            state: state.clone(),
            is_cleanup_running: is_cleanup_running.clone(),
        };
        
        // Launch cleanup task
        tokio::spawn({
            let state = state.clone();
            let is_running = is_cleanup_running.clone();
            async move {
                Self::cleanup_task(state, is_running).await;
            }
        });
        
        instance
    }
    
    /// Background task that periodically cleans up expired entries
    async fn cleanup_task(
        state: Arc<RwLock<HashMap<String, HashMap<String, StateEntry>>>>,
        is_running: Arc<tokio::sync::Mutex<bool>>
    ) {
        let mut interval = time::interval(Duration::from_secs(30));
        
        // Mark as running
        {
            let mut running = is_running.lock().await;
            *running = true;
        }
        
        loop {
            interval.tick().await;
            
            // Perform cleanup
            let mut total_removed = 0;
            
            // Need to acquire write lock here
            let mut state_lock = state.write().await;
            
            // Iterate through all scopes
            for (scope_key, scope_map) in state_lock.iter_mut() {
                // Find keys to remove
                let expired_keys: Vec<String> = scope_map
                    .iter()
                    .filter(|(_, entry)| entry.is_expired())
                    .map(|(key, _)| key.clone())
                    .collect();
                
                // Remove expired keys
                for key in &expired_keys {
                    scope_map.remove(key);
                    total_removed += 1;
                }
                
                if !expired_keys.is_empty() {
                    debug!(
                        "Cleaned up {} expired keys in scope {}",
                        expired_keys.len(),
                        scope_key
                    );
                }
            }
            
            // Remove empty scopes
            let empty_scopes: Vec<String> = state_lock
                .iter()
                .filter(|(_, scope_map)| scope_map.is_empty())
                .map(|(scope_key, _)| scope_key.clone())
                .collect();
            
            for scope_key in empty_scopes {
                state_lock.remove(&scope_key);
            }
            
            if total_removed > 0 {
                info!("Shared state cleanup removed {} expired keys", total_removed);
            }
        }
    }
}

impl Default for InMemorySharedStateService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SharedStateService for InMemorySharedStateService {
    async fn get_state(&self, scope_key: &str, key: &str) -> Result<Option<Value>, CoreError> {
        let state = self.state.read().await;
        
        match state.get(scope_key) {
            Some(scope_map) => {
                match scope_map.get(key) {
                    Some(entry) => {
                        if entry.is_expired() {
                            // Entry is expired, return None (clean happens in background)
                            Ok(None)
                        } else {
                            Ok(Some(entry.value.clone()))
                        }
                    },
                    None => Ok(None),
                }
            },
            None => Ok(None),
        }
    }
    
    async fn set_state(&self, scope_key: &str, key: &str, value: Value) -> Result<(), CoreError> {
        let mut state = self.state.write().await;
        
        // Get or create scope map
        let scope_map = state.entry(scope_key.to_string()).or_insert_with(HashMap::new);
        
        // Insert value with no TTL
        scope_map.insert(key.to_string(), StateEntry::new(value));
        
        debug!("Set shared state for scope={}, key={}", scope_key, key);
        Ok(())
    }
    
    async fn set_state_with_ttl(&self, scope_key: &str, key: &str, value: Value, ttl_ms: u64) -> Result<(), CoreError> {
        let mut state = self.state.write().await;
        
        // Get or create scope map
        let scope_map = state.entry(scope_key.to_string()).or_insert_with(HashMap::new);
        
        // Insert value with TTL
        scope_map.insert(key.to_string(), StateEntry::with_ttl(value, ttl_ms));
        
        debug!("Set shared state with ttl={}ms for scope={}, key={}", ttl_ms, scope_key, key);
        Ok(())
    }
    
    async fn delete_state(&self, scope_key: &str, key: &str) -> Result<(), CoreError> {
        let mut state = self.state.write().await;
        
        if let Some(scope_map) = state.get_mut(scope_key) {
            scope_map.remove(key);
            debug!("Deleted shared state for scope={}, key={}", scope_key, key);
        }
        
        Ok(())
    }
    
    async fn list_keys(&self, scope_key: &str) -> Result<Vec<String>, CoreError> {
        let state = self.state.read().await;
        
        match state.get(scope_key) {
            Some(scope_map) => {
                // Filter out expired keys
                let keys: Vec<String> = scope_map
                    .iter()
                    .filter(|(_, entry)| !entry.is_expired())
                    .map(|(key, _)| key.clone())
                    .collect();
                
                Ok(keys)
            },
            None => Ok(Vec::new()),
        }
    }
    
    async fn get_metrics(&self) -> Result<Value, CoreError> {
        let state = self.state.read().await;
        
        let total_scopes = state.len();
        let mut total_keys = 0;
        let mut active_keys = 0;
        let mut expired_keys = 0;
        
        // Count keys across all scopes
        for (_, scope_map) in state.iter() {
            total_keys += scope_map.len();
            
            // Count active vs expired keys
            for (_, entry) in scope_map.iter() {
                if entry.is_expired() {
                    expired_keys += 1;
                } else {
                    active_keys += 1;
                }
            }
        }
        
        Ok(json!({
            "type": "in_memory",
            "scopes": total_scopes,
            "total_keys": total_keys,
            "active_keys": active_keys,
            "expired_keys": expired_keys,
            "is_cleanup_running": *self.is_cleanup_running.lock().await
        }))
    }
}

// Redis implementation if the redis feature is enabled
#[cfg(feature = "redis")]
pub mod redis {
    use super::*;
    use redis::{Client, AsyncCommands, ConnectionLike};
    use redis::RedisError;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    use std::time::Duration;

    const DEFAULT_POOL_SIZE: usize = 20;
    const DEFAULT_CONNECTION_TIMEOUT_MS: u64 = 3000;
    const DEFAULT_POOL_TIMEOUT_MS: u64 = 5000;

    /// Connection pool for Redis
    struct RedisConnectionPool {
        /// Redis client
        client: Client,
        /// Semaphore to limit concurrent connections
        semaphore: Semaphore,
        /// Connection timeout in milliseconds
        connection_timeout_ms: u64,
        /// Pool timeout in milliseconds
        pool_timeout_ms: u64,
    }

    impl RedisConnectionPool {
        /// Create a new Redis connection pool
        fn new(client: Client, pool_size: usize, connection_timeout_ms: u64, pool_timeout_ms: u64) -> Self {
            Self {
                client,
                semaphore: Semaphore::new(pool_size),
                connection_timeout_ms,
                pool_timeout_ms,
            }
        }

        /// Get a connection from the pool
        async fn get_connection(&self) -> Result<redis::aio::Connection, CoreError> {
            // Acquire permit with timeout
            let permit = match time::timeout(
                Duration::from_millis(self.pool_timeout_ms),
                self.semaphore.acquire()
            ).await {
                Ok(Ok(permit)) => permit,
                Ok(Err(e)) => return Err(CoreError::StateStoreError(format!("Redis semaphore error: {}", e))),
                Err(_) => return Err(CoreError::StateStoreError(format!("Timed out waiting for Redis connection after {}ms", self.pool_timeout_ms))),
            };

            // Get a connection with timeout
            match time::timeout(
                Duration::from_millis(self.connection_timeout_ms),
                self.client.get_async_connection()
            ).await {
                Ok(Ok(conn)) => {
                    // Create wrapper that releases permit when dropped
                    Ok(conn)
                },
                Ok(Err(e)) => {
                    // Release permit explicitly on error
                    drop(permit);
                    Err(CoreError::StateStoreError(format!("Redis connection error: {}", e)))
                },
                Err(_) => {
                    // Release permit explicitly on timeout
                    drop(permit);
                    Err(CoreError::StateStoreError(format!("Timed out establishing Redis connection after {}ms", self.connection_timeout_ms)))
                }
            }
        }
    }

    /// Connection pool configuration
    #[derive(Debug, Clone)]
    pub struct RedisPoolConfig {
        /// Maximum number of concurrent connections
        pub max_connections: usize,
        /// Connection timeout in milliseconds
        pub connection_timeout_ms: u64,
        /// Pool timeout in milliseconds (waiting for available connection)
        pub pool_timeout_ms: u64,
    }

    impl Default for RedisPoolConfig {
        fn default() -> Self {
            Self {
                max_connections: DEFAULT_POOL_SIZE,
                connection_timeout_ms: DEFAULT_CONNECTION_TIMEOUT_MS,
                pool_timeout_ms: DEFAULT_POOL_TIMEOUT_MS,
            }
        }
    }

    /// Redis implementation of SharedStateService
    pub struct RedisSharedStateService {
        /// Connection pool
        pool: Arc<RedisConnectionPool>,
        /// Configuration
        config: RedisPoolConfig,
        /// Pool metrics
        metrics: Arc<tokio::sync::RwLock<PoolMetrics>>,
    }

    /// Pool metrics
    #[derive(Debug, Default)]
    struct PoolMetrics {
        /// Number of successful connection acquisitions
        success_count: u64,
        /// Number of failed connection acquisitions
        error_count: u64,
        /// Number of timeouts waiting for a connection
        timeout_count: u64,
        /// Total wait time in milliseconds
        total_wait_ms: u64,
        /// Maximum wait time in milliseconds
        max_wait_ms: u64,
    }

    impl RedisSharedStateService {
        /// Create a new Redis shared state service
        pub fn new(redis_url: &str) -> Result<Self, RedisError> {
            Self::with_config(redis_url, RedisPoolConfig::default())
        }

        /// Create a new Redis shared state service with custom pool configuration
        pub fn with_config(redis_url: &str, config: RedisPoolConfig) -> Result<Self, RedisError> {
            info!("Creating new RedisSharedStateService with URL: {}, pool_size: {}", 
                  redis_url, config.max_connections);
            
            let client = Client::open(redis_url)?;
            let pool = RedisConnectionPool::new(
                client,
                config.max_connections,
                config.connection_timeout_ms,
                config.pool_timeout_ms,
            );

            Ok(Self { 
                pool: Arc::new(pool),
                config,
                metrics: Arc::new(tokio::sync::RwLock::new(PoolMetrics::default())),
            })
        }
        
        /// Format a Redis key from scope and key
        fn make_key(scope_key: &str, key: &str) -> String {
            format!("cascade:shared:{}:{}", scope_key, key)
        }
        
        /// Get the key prefix for a scope
        fn make_scope_prefix(scope_key: &str) -> String {
            format!("cascade:shared:{}:", scope_key)
        }

        /// Get a connection from the pool with metrics tracking
        async fn get_pooled_connection(&self) -> Result<redis::aio::Connection, CoreError> {
            let start = std::time::Instant::now();
            let result = self.pool.get_connection().await;
            let elapsed = start.elapsed().as_millis() as u64;

            // Update metrics
            let mut metrics = self.metrics.write().await;
            match &result {
                Ok(_) => {
                    metrics.success_count += 1;
                    metrics.total_wait_ms += elapsed;
                    metrics.max_wait_ms = std::cmp::max(metrics.max_wait_ms, elapsed);
                },
                Err(e) => {
                    if e.to_string().contains("timed out") {
                        metrics.timeout_count += 1;
                    } else {
                        metrics.error_count += 1;
                    }
                }
            }

            result
        }
    }

    #[async_trait]
    impl SharedStateService for RedisSharedStateService {
        async fn get_state(&self, scope_key: &str, key: &str) -> Result<Option<Value>, CoreError> {
            let redis_key = Self::make_key(scope_key, key);
            
            let mut conn = self.get_pooled_connection().await?;
            
            let result: Option<String> = conn.get(&redis_key).await
                .map_err(|e| CoreError::StateStoreError(format!("Redis get error: {}", e)))?;
            
            match result {
                Some(json_str) => {
                    serde_json::from_str(&json_str)
                        .map(Some)
                        .map_err(|e| CoreError::StateStoreError(format!("JSON parse error: {}", e)))
                },
                None => Ok(None),
            }
        }
        
        async fn set_state(&self, scope_key: &str, key: &str, value: Value) -> Result<(), CoreError> {
            let redis_key = Self::make_key(scope_key, key);
            let json_str = serde_json::to_string(&value)
                .map_err(|e| CoreError::StateStoreError(format!("JSON serialize error: {}", e)))?;
            
            let mut conn = self.get_pooled_connection().await?;
            
            conn.set(&redis_key, json_str).await
                .map_err(|e| CoreError::StateStoreError(format!("Redis set error: {}", e)))?;
            
            debug!("Set Redis shared state for scope={}, key={}", scope_key, key);
            Ok(())
        }
        
        async fn set_state_with_ttl(&self, scope_key: &str, key: &str, value: Value, ttl_ms: u64) -> Result<(), CoreError> {
            let redis_key = Self::make_key(scope_key, key);
            let json_str = serde_json::to_string(&value)
                .map_err(|e| CoreError::StateStoreError(format!("JSON serialize error: {}", e)))?;
            
            let mut conn = self.get_pooled_connection().await?;
            
            // Convert milliseconds to seconds for Redis TTL
            let ttl_seconds = (ttl_ms / 1000) as usize;
            if ttl_seconds == 0 {
                // For very short TTLs, make it at least 1 second
                conn.set_ex(&redis_key, json_str, 1).await
                    .map_err(|e| CoreError::StateStoreError(format!("Redis setex error: {}", e)))?;
            } else {
                conn.set_ex(&redis_key, json_str, ttl_seconds).await
                    .map_err(|e| CoreError::StateStoreError(format!("Redis setex error: {}", e)))?;
            }
            
            debug!("Set Redis shared state with ttl={}ms for scope={}, key={}", ttl_ms, scope_key, key);
            Ok(())
        }
        
        async fn delete_state(&self, scope_key: &str, key: &str) -> Result<(), CoreError> {
            let redis_key = Self::make_key(scope_key, key);
            
            let mut conn = self.get_pooled_connection().await?;
            
            conn.del(&redis_key).await
                .map_err(|e| CoreError::StateStoreError(format!("Redis delete error: {}", e)))?;
            
            debug!("Deleted Redis shared state for scope={}, key={}", scope_key, key);
            Ok(())
        }
        
        async fn list_keys(&self, scope_key: &str) -> Result<Vec<String>, CoreError> {
            let pattern = format!("{}*", Self::make_scope_prefix(scope_key));
            
            let mut conn = self.get_pooled_connection().await?;
            
            let keys: Vec<String> = redis::cmd("KEYS")
                .arg(&pattern)
                .query_async(&mut conn)
                .await
                .map_err(|e| CoreError::StateStoreError(format!("Redis keys error: {}", e)))?;
            
            // Extract the key part from the redis key
            let prefix = Self::make_scope_prefix(scope_key);
            let result = keys.into_iter()
                .filter_map(|full_key| {
                    if full_key.starts_with(&prefix) {
                        Some(full_key[prefix.len()..].to_string())
                    } else {
                        None
                    }
                })
                .collect();
            
            Ok(result)
        }
        
        async fn get_metrics(&self) -> Result<Value, CoreError> {
            let mut conn = self.get_pooled_connection().await?;
            
            // Get all keys and count by scope
            let keys: Vec<String> = redis::cmd("KEYS")
                .arg("cascade:shared:*")
                .query_async(&mut conn)
                .await
                .map_err(|e| CoreError::StateStoreError(format!("Redis keys error: {}", e)))?;
            
            // Count scopes and keys
            let mut scopes = HashMap::new();
            let mut total_keys = 0;
            
            for key in &keys {
                total_keys += 1;
                
                // Extract scope from key
                if let Some(pos) = key.find(':', "cascade:shared:".len()) {
                    let scope = &key["cascade:shared:".len()..pos];
                    *scopes.entry(scope.to_string()).or_insert(0) += 1;
                }
            }

            // Get pool metrics
            let pool_metrics = self.metrics.read().await;
            let avg_wait_ms = if pool_metrics.success_count > 0 {
                pool_metrics.total_wait_ms / pool_metrics.success_count
            } else {
                0
            };
            
            Ok(json!({
                "type": "redis",
                "scopes": scopes.len(),
                "total_keys": total_keys,
                "keys_by_scope": scopes,
                "connection_status": "connected",
                "pool": {
                    "size": self.config.max_connections,
                    "available": self.pool.semaphore.available_permits(),
                    "success_count": pool_metrics.success_count,
                    "error_count": pool_metrics.error_count,
                    "timeout_count": pool_metrics.timeout_count,
                    "avg_wait_ms": avg_wait_ms,
                    "max_wait_ms": pool_metrics.max_wait_ms
                }
            }))
        }
        
        async fn health_check(&self) -> Result<bool, CoreError> {
            let mut conn = match time::timeout(
                Duration::from_millis(self.config.connection_timeout_ms),
                self.get_pooled_connection()
            ).await {
                Ok(Ok(conn)) => conn,
                Ok(Err(e)) => {
                    error!("Redis health check failed: {}", e);
                    return Err(e);
                },
                Err(_) => {
                    error!("Redis health check timed out after {}ms", self.config.connection_timeout_ms);
                    return Err(CoreError::StateStoreError(
                        format!("Redis health check timed out after {}ms", self.config.connection_timeout_ms)
                    ));
                }
            };
            
            let ping: String = redis::cmd("PING")
                .query_async(&mut conn)
                .await
                .map_err(|e| {
                    error!("Redis PING failed: {}", e);
                    CoreError::StateStoreError(format!("Redis ping error: {}", e))
                })?;
            
            Ok(ping == "PONG")
        }
    }
}

/// Factory function to create a SharedStateService based on URL
pub fn create_shared_state_service(url: &str) -> ServerResult<Arc<dyn SharedStateService>> {
    if url.starts_with("memory://") {
        info!("Creating in-memory shared state service");
        Ok(Arc::new(InMemorySharedStateService::new()))
    } else if url.starts_with("redis://") {
        #[cfg(feature = "redis")]
        {
            info!("Creating Redis shared state service");
            let service = redis::RedisSharedStateService::new(url)
                .map_err(|e| ServerError::StateServiceError(format!("Redis init error: {}", e)))?;
            Ok(Arc::new(service))
        }
        
        #[cfg(not(feature = "redis"))]
        {
            error!("Redis shared state service requested but 'redis' feature not enabled");
            Err(ServerError::StateServiceError(
                "Redis shared state service requested but 'redis' feature not enabled".to_string()
            ))
        }
    } else {
        error!("Unsupported shared state service URL: {}", url);
        Err(ServerError::StateServiceError(format!(
            "Unsupported shared state service URL: {}", url
        )))
    }
} 
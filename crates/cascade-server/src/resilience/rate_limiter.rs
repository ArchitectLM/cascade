//! 
//! Rate limiter for limiting the rate of API calls
//! Supports both local and distributed rate limiting through shared state
//!

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::warn;
use std::fmt;

use serde_json::json;

use crate::error::{ServerError, ServerResult};
use crate::shared_state::SharedStateService;
use super::current_time_ms;

/// Rate limiter key for identifying different rate limit buckets
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RateLimiterKey {
    /// Resource being limited (API endpoint, component name, etc.)
    pub resource: String,
    
    /// Optional identifier (user ID, API key, IP address, etc.)
    pub identifier: Option<String>,
    
    /// Optional scope for distributed rate limiting
    pub scope: Option<String>,
}

impl RateLimiterKey {
    /// Create a new rate limiter key with a specific resource and optional ID
    pub fn new(resource: String, identifier: Option<String>, scope: Option<String>) -> Self {
        Self {
            resource,
            identifier,
            scope,
        }
    }
}

impl fmt::Display for RateLimiterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.identifier {
            Some(id) => write!(f, "{}:{}", self.resource, id),
            None => write!(f, "{}", self.resource),
        }
    }
}

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    /// Maximum number of requests allowed in the time window
    pub max_requests: u32,
    
    /// Time window in milliseconds
    pub window_ms: u64,
    
    /// Optional shared state scope for distributed rate limiting
    pub shared_state_scope: Option<String>,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window_ms: 60_000, // 1 minute
            shared_state_scope: None,
        }
    }
}

/// Local rate limiter bucket
struct LocalBucket {
    /// Timestamp when the bucket was created or reset
    start_time: Instant,
    
    /// Remaining requests in the current window
    remaining: u32,
}

/// Rate limiter for controlling request rates
pub struct RateLimiter {
    /// Configuration for the rate limiter
    config: RateLimiterConfig,
    
    /// Local buckets for rate limiting
    local_buckets: Mutex<HashMap<RateLimiterKey, LocalBucket>>,
    
    /// Shared state service for distributed rate limiting
    shared_state: Option<Arc<dyn SharedStateService>>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration
    pub fn new(config: RateLimiterConfig, shared_state: Option<Arc<dyn SharedStateService>>) -> Self {
        Self {
            config,
            local_buckets: Mutex::new(HashMap::new()),
            shared_state,
        }
    }
    
    /// Check if a request is allowed by the rate limiter
    /// Returns the number of remaining requests in the window if allowed
    /// Returns an error if the rate limit is exceeded
    pub async fn allow(&self, key: &RateLimiterKey) -> ServerResult<u32> {
        // If we have shared state and a scope, use distributed rate limiting
        if let (Some(service), Some(scope)) = (&self.shared_state, &self.config.shared_state_scope) {
            self.allow_distributed(service, scope, key).await
        } else {
            // Otherwise use local rate limiting
            self.allow_local(key)
        }
    }
    
    /// Check if allowed using local rate limiting
    fn allow_local(&self, key: &RateLimiterKey) -> ServerResult<u32> {
        let mut buckets = self.local_buckets.lock().unwrap();
        
        let now = Instant::now();
        let window_duration = Duration::from_millis(self.config.window_ms);
        
        // Get or create bucket
        let bucket = buckets.entry(key.clone()).or_insert_with(|| LocalBucket {
            start_time: now,
            remaining: self.config.max_requests,
        });
        
        // Reset bucket if the window has expired
        if now.duration_since(bucket.start_time) >= window_duration {
            bucket.start_time = now;
            bucket.remaining = self.config.max_requests;
        }
        
        // Check if we have remaining requests
        if bucket.remaining > 0 {
            bucket.remaining -= 1;
            Ok(bucket.remaining)
        } else {
            // Rate limit exceeded
            Err(ServerError::RateLimitExceeded {
                resource: key.resource.clone(),
                identifier: key.identifier.clone().unwrap_or_default(),
                max_requests: self.config.max_requests,
                window_ms: self.config.window_ms,
            })
        }
    }
    
    /// Check if allowed using distributed rate limiting with shared state
    async fn allow_distributed(
        &self,
        service: &Arc<dyn SharedStateService>,
        scope: &str,
        key: &RateLimiterKey
    ) -> ServerResult<u32> {
        let state_key = key.to_string();
        let now = current_time_ms();
        
        // Get current bucket data from shared state
        let bucket_data = match service.get_state(scope, &state_key).await {
            Ok(Some(data)) => data,
            Ok(None) => {
                // Initialize a new bucket
                json!({
                    "start_time": now,
                    "remaining": self.config.max_requests,
                })
            },
            Err(e) => {
                warn!("Error fetching rate limit data: {}", e);
                // Fall back to local rate limiting on error
                return self.allow_local(key);
            }
        };
        
        let start_time = bucket_data["start_time"].as_u64().unwrap_or(now);
        let remaining = bucket_data["remaining"].as_u64().unwrap_or(self.config.max_requests as u64) as u32;
        
        // Reset bucket if the window has expired
        if now - start_time >= self.config.window_ms {
            // Initialize a new bucket
            let new_bucket = json!({
                "start_time": now,
                "remaining": self.config.max_requests - 1, // subtract the current request
            });
            
            if let Err(e) = service.set_state(scope, &state_key, new_bucket.clone()).await {
                warn!("Error saving rate limit data: {}", e);
                // Fall back to local rate limiting on error
                return self.allow_local(key);
            }
            
            return Ok(self.config.max_requests - 1);
        }
        
        // Check if we have remaining requests
        if remaining > 0 {
            // Decrement remaining requests
            let new_bucket = json!({
                "start_time": start_time,
                "remaining": remaining - 1,
            });
            
            if let Err(e) = service.set_state(scope, &state_key, new_bucket.clone()).await {
                warn!("Error saving rate limit data: {}", e);
                // Fall back to local rate limiting on error
                return self.allow_local(key);
            }
            
            Ok(remaining - 1)
        } else {
            // Rate limit exceeded
            Err(ServerError::RateLimitExceeded {
                resource: key.resource.clone(),
                identifier: key.identifier.clone().unwrap_or_default(),
                max_requests: self.config.max_requests,
                window_ms: self.config.window_ms,
            })
        }
    }
    
    /// Execute a function with rate limiting
    /// Returns the result of the function if allowed
    /// Returns an error if the rate limit is exceeded
    pub async fn execute<F, Fut, T>(
        &self,
        key: &RateLimiterKey,
        operation: F
    ) -> ServerResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ServerResult<T>>,
    {
        // Check if allowed by rate limiter
        self.allow(key).await?;
        
        // Execute the operation
        operation().await
    }
    
    /// Create a rate limiter that uses the given component runtime API
    /// for shared state operations
    pub fn for_component(
        config: RateLimiterConfig,
        shared_state: Option<Arc<dyn SharedStateService>>
    ) -> Self {
        Self::new(config, shared_state)
    }
} 
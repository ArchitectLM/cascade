use cascade_core::{
    ComponentExecutor, 
    ComponentRuntimeApi, 
    CoreError, 
    ExecutionResult,
    LogLevel,
    ComponentExecutorBase,
};
use cascade_core::types::DataPacket;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Rate limiter state
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RateLimiterState {
    /// Start time of the current window (in milliseconds since epoch)
    start_time: u64,
    
    /// Remaining requests in the current window
    remaining: u32,
    
    /// Total capacity for the window
    capacity: u32,
    
    /// Window size in milliseconds
    window_ms: u64,
    
    /// Unique ID for this rate limiter for shared state
    #[serde(skip_serializing_if = "Option::is_none")]
    limiter_id: Option<String>,
}

/// RateLimiter component
/// 
/// Implements the rate limiting pattern to control the rate at which
/// operations are allowed to proceed.
#[derive(Debug, Default)]
pub struct RateLimiter {
    // Default values
    default_max_requests: u32,
    default_window_ms: u64,
}

impl RateLimiter {
    /// Create a new RateLimiter component
    pub fn new() -> Self {
        Self {
            default_max_requests: 100,
            default_window_ms: 60000, // 1 minute
        }
    }

    /// Get the current timestamp in milliseconds
    fn current_time_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Get the rate limiter state from either local component state or shared state
    async fn get_limiter_state(
        &self, 
        api: &Arc<dyn ComponentRuntimeApi>,
        shared_state_scope: Option<&str>,
        limiter_id: Option<&str>,
    ) -> Result<RateLimiterState, CoreError> {
        // If shared state is configured and we have a limiter_id, try to load from there first
        if let (Some(scope), Some(id)) = (shared_state_scope, limiter_id) {
            let key = format!("ratelimit:{}", id);
            match api.get_shared_state(scope, &key).await {
                Ok(Some(state_value)) => {
                    match serde_json::from_value::<RateLimiterState>(state_value) {
                        Ok(state) => {
                            let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                                &format!("Loaded rate limiter state from shared state: {}/{} remaining", 
                                    state.remaining, state.capacity)).await;
                            return Ok(state);
                        }
                        Err(e) => {
                            let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                                &format!("Failed to deserialize shared rate limiter state: {}", e)).await;
                            // Fall through to component state
                        }
                    }
                }
                Ok(None) => {
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                        "No rate limiter state found in shared state, checking component state").await;
                    // Fall through to component state
                }
                Err(e) => {
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                        &format!("Error loading shared rate limiter state: {}", e)).await;
                    // Fall through to component state
                }
            }
        }
        
        // Try to load from component state (either as fallback or primary option)
        match api.get_state().await {
            Ok(Some(state_packet)) => {
                match serde_json::from_value::<RateLimiterState>(state_packet) {
                    Ok(state) => {
                        let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                            &format!("Loaded rate limiter state from component state: {}/{} remaining", 
                                state.remaining, state.capacity)).await;
                        Ok(state)
                    }
                    Err(e) => {
                        let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                            &format!("Invalid rate limiter state: {}", e)).await;
                        
                        // Initialize with closed state as fallback
                        let max_requests = match api.get_config("maxRequests").await {
                            Ok(value) => value.as_u64().unwrap_or(self.default_max_requests as u64) as u32,
                            Err(_) => self.default_max_requests,
                        };
                        
                        let window_ms = match api.get_config("windowMs").await {
                            Ok(value) => value.as_u64().unwrap_or(self.default_window_ms),
                            Err(_) => self.default_window_ms,
                        };
                        
                        Ok(RateLimiterState {
                            start_time: Self::current_time_ms(),
                            remaining: max_requests,
                            capacity: max_requests,
                            window_ms,
                            limiter_id: limiter_id.map(|s| s.to_string()),
                        })
                    }
                }
            }
            // No state found - initialize new state
            _ => {
                let max_requests = match api.get_config("maxRequests").await {
                    Ok(value) => value.as_u64().unwrap_or(self.default_max_requests as u64) as u32,
                    Err(_) => self.default_max_requests,
                };
                
                let window_ms = match api.get_config("windowMs").await {
                    Ok(value) => value.as_u64().unwrap_or(self.default_window_ms),
                    Err(_) => self.default_window_ms,
                };
                
                Ok(RateLimiterState {
                    start_time: Self::current_time_ms(),
                    remaining: max_requests,
                    capacity: max_requests,
                    window_ms,
                    limiter_id: limiter_id.map(|s| s.to_string()),
                })
            },
        }
    }
    
    /// Save the rate limiter state to either local component state or shared state
    async fn save_limiter_state(
        &self, 
        api: &Arc<dyn ComponentRuntimeApi>,
        state: &RateLimiterState,
        shared_state_scope: Option<&str>,
    ) -> Result<(), CoreError> {
        // Always save to component state
        if let Err(e) = api.save_state(json!(state)).await {
            let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                &format!("Failed to save component state: {}", e)).await;
            return Err(e);
        }
        
        // If shared state is configured and we have a limiter_id, save there too
        if let (Some(scope), Some(id)) = (shared_state_scope, &state.limiter_id) {
            let key = format!("ratelimit:{}", id);
            
            // Set TTL based on the window size to automatically clean up expired buckets
            // Add a buffer of 10% to ensure we don't expire too early
            let ttl_ms = state.window_ms + (state.window_ms / 10);
            
            // Try to use TTL
            if let Err(e) = api.set_shared_state_with_ttl(scope, &key, json!(state), ttl_ms).await {
                let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                    &format!("Failed to save to shared state with TTL: {}", e)).await;
                
                // Fall back to regular set
                if let Err(e) = api.set_shared_state(scope, &key, json!(state)).await {
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                        &format!("Failed to save to shared state: {}", e)).await;
                    // Continue even if shared state fails - component state was saved
                } else {
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                        &format!("Saved rate limiter state to shared state: {}/{}", 
                            state.remaining, state.capacity)).await;
                }
            } else {
                let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                    &format!("Saved rate limiter state to shared state with TTL: {}/{}", 
                        state.remaining, state.capacity)).await;
            }
        }
        
        Ok(())
    }
}

impl ComponentExecutorBase for RateLimiter {
    fn component_type(&self) -> &str {
        "RateLimiter"
    }
}

#[async_trait]
impl ComponentExecutor for RateLimiter {
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
        // Step 1: Get configuration parameters
        let max_requests = match api.get_config("maxRequests").await {
            Ok(value) => value.as_u64().unwrap_or(self.default_max_requests as u64) as u32,
            Err(_) => self.default_max_requests,
        };
        
        let window_ms = match api.get_config("windowMs").await {
            Ok(value) => value.as_u64().unwrap_or(self.default_window_ms),
            Err(_) => self.default_window_ms,
        };
        
        // Get shared state configuration
        let shared_state_scope = match api.get_config("sharedStateScope").await {
            Ok(value) => value.as_str().map(|s| s.to_string()),
            Err(_) => None,
        };
        
        // Get optional limiter ID (for shared state tracking)
        let limiter_id = match api.get_config("limiterId").await {
            Ok(value) => value.as_str().map(|s| s.to_string()),
            Err(_) => None,
        };

        // Generate a limiter ID if we're using shared state but no ID was provided
        let limiter_id = if shared_state_scope.is_some() && limiter_id.is_none() {
            // Use correlation ID or generate UUID for scoping
            let correlation = api.correlation_id().await.map_or_else(
                |_| Uuid::new_v4().to_string(),
                |id| id.0
            );
            Some(format!("{}-limiter", correlation))
        } else {
            limiter_id
        };
        
        // Get the description field for logging and metrics
        let limiter_description = match api.get_config("description").await {
            Ok(value) => value.as_str().map(|s| s.to_string()).unwrap_or_else(|| 
                limiter_id.clone().unwrap_or_else(|| "unnamed-limiter".to_string())
            ),
            Err(_) => limiter_id.clone().unwrap_or_else(|| "unnamed-limiter".to_string()),
        };
        
        // Step 2: Check current rate limit state
        let now = Self::current_time_ms();
        
        // Try to get existing state or initialize a new one
        let mut state = match self.get_limiter_state(&api, shared_state_scope.as_deref(), limiter_id.as_deref()).await {
            Ok(mut state) => {
                // Check if window has expired
                if now - state.start_time >= state.window_ms {
                    // Reset the window
                    state.start_time = now;
                    state.remaining = state.capacity;
                    
                    let _ = ComponentRuntimeApi::log(&*api, LogLevel::Debug, 
                        &format!("Rate limiter '{}' window reset, new quota {}/{}", 
                            limiter_description, state.remaining, state.capacity)).await;
                }
                state
            },
            // No state or invalid state, initialize fresh
            Err(_) => {
                let state = RateLimiterState {
                    start_time: now,
                    remaining: max_requests,
                    capacity: max_requests,
                    window_ms,
                    limiter_id,
                };
                
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Debug, 
                    &format!("Created new rate limiter state for '{}', quota {}/{}", 
                        limiter_description, state.remaining, state.capacity)).await;
                
                state
            }
        };
        
        // Step 3: Check if we have remaining capacity
        if state.remaining > 0 {
            // We have capacity, decrement and allow the operation
            state.remaining -= 1;
            
            // Log and emit metric
            let _ = ComponentRuntimeApi::log(&*api, LogLevel::Debug, 
                &format!("Rate limiter '{}' allowed request, {}/{} remaining", 
                    limiter_description, state.remaining, state.capacity)).await;
            
            let mut labels = HashMap::new();
            labels.insert("component".to_string(), "RateLimiter".to_string());
            labels.insert("limiter".to_string(), limiter_description.clone());
            let _ = api.emit_metric("rate_limiter_remaining", state.remaining as f64, labels.clone()).await;
            let _ = api.emit_metric("rate_limiter_request_allowed", 1.0, labels).await;
            
            // Save updated state
            if let Err(e) = self.save_limiter_state(&api, &state, shared_state_scope.as_deref()).await {
                return ExecutionResult::Failure(e);
            }
            
            // Set output to indicate allowed status
            let output = DataPacket::new(json!({
                "allowed": true,
                "remaining": state.remaining,
                "capacity": state.capacity,
                "reset": state.start_time + state.window_ms,
                "limiter": limiter_description
            }));
            
            if let Err(e) = api.set_output("status", output).await {
                return ExecutionResult::Failure(e);
            }
            
            // Check if request is in the context, if so, pass it through
            if let Ok(request) = api.get_input("request").await {
                if let Err(e) = api.set_output("result", request).await {
                    return ExecutionResult::Failure(e);
                }
            }
            
            return ExecutionResult::Success;
        } else {
            // Rate limit exceeded, block the request
            let _ = ComponentRuntimeApi::log(&*api, LogLevel::Info, 
                &format!("Rate limiter '{}' blocked request, limit exceeded (0/{} remaining)", 
                    limiter_description, state.capacity)).await;
            
            let mut labels = HashMap::new();
            labels.insert("component".to_string(), "RateLimiter".to_string());
            labels.insert("limiter".to_string(), limiter_description.clone());
            let _ = api.emit_metric("rate_limiter_request_blocked", 1.0, labels).await;
            
            // Save state (even though it didn't change, to refresh TTL in shared state)
            if let Err(e) = self.save_limiter_state(&api, &state, shared_state_scope.as_deref()).await {
                return ExecutionResult::Failure(e);
            }
            
            // Set output to indicate blocked status
            let status_output = DataPacket::new(json!({
                "allowed": false,
                "remaining": 0,
                "capacity": state.capacity,
                "reset": state.start_time + state.window_ms,
                "resetIn": (state.start_time + state.window_ms).saturating_sub(now),
                "limiter": limiter_description
            }));
            
            if let Err(e) = api.set_output("status", status_output).await {
                return ExecutionResult::Failure(e);
            }
            
            // Set error to indicate rate limit exceeded
            let error_output = DataPacket::new(json!({
                "errorCode": "ERR_RATE_LIMIT_EXCEEDED",
                "errorMessage": format!("Rate limit exceeded for '{}'", limiter_description),
                "component": {
                    "type": "RateLimiter"
                },
                "details": {
                    "limiter": limiter_description,
                    "capacity": state.capacity,
                    "windowMs": state.window_ms,
                    "reset": state.start_time + state.window_ms,
                    "currentTime": now,
                    "resetIn": (state.start_time + state.window_ms).saturating_sub(now)
                }
            }));
            
            if let Err(e) = api.set_output("error", error_output).await {
                return ExecutionResult::Failure(e);
            }
            
            return ExecutionResult::Success;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Test implementations will be added here
} 
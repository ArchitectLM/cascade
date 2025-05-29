use cascade_core::{
    ComponentExecutor,
    ComponentExecutorBase,
    ComponentRuntimeApi,
    CoreError,
    ExecutionResult,
    LogLevel,
};
use cascade_core::types::DataPacket;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use std::sync::Arc;
use std::collections::HashMap;
use uuid;

/// A component that implements retry logic for arbitrary operation components
#[derive(Debug)]
pub struct RetryWrapper {
    // Default values
    default_max_attempts: u32,
    default_initial_delay_ms: u64,
    default_backoff_multiplier: f64,
    default_max_delay_ms: u64,
    default_jitter_factor: f64,
}

/// The retry state, stored as component state
#[derive(Debug, Serialize, Deserialize, Clone)]
struct RetryState {
    // Required fields
    attempt: u32,
    max_attempts: u32,
    
    // Timing configuration
    initial_delay_ms: u64,
    backoff_multiplier: f64,
    max_delay_ms: u64,
    
    // State tracking
    #[serde(default)]
    completed: bool,
    
    // Result storage (if completed)
    #[serde(skip_serializing_if = "Option::is_none")]
    final_result: Option<bool>,
    
    // Unique ID for this retry sequence if using shared state
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_id: Option<String>,
}

impl RetryWrapper {
    /// Create a new RetryWrapper component
    pub fn new() -> Self {
        Self {
            default_max_attempts: 3,
            default_initial_delay_ms: 1000,
            default_backoff_multiplier: 2.0,
            default_max_delay_ms: 30000,
            default_jitter_factor: 0.2,
        }
    }
    
    /// Calculate the delay for the next retry
    fn calculate_delay(&self, state: &RetryState) -> Duration {
        let base_delay_ms = (state.initial_delay_ms as f64 * state.backoff_multiplier.powf(state.attempt as f64 - 1.0))
            .min(state.max_delay_ms as f64) as u64;
        
        // Apply jitter factor to avoid thundering herd issues
        let jitter_range = (base_delay_ms as f64 * self.default_jitter_factor) as u64;
        let jitter = if jitter_range > 0 {
            // Create a random jitter between -jitter_range/2 and +jitter_range/2
            let rand_value = (uuid::Uuid::new_v4().as_u128() % jitter_range as u128) as i64;
            (rand_value - (jitter_range as i64 / 2)).max(-(base_delay_ms as i64 / 2)) // Ensure we don't get negative delay
        } else {
            0
        };
        
        // Apply jitter (making sure we don't end up with negative values)
        let final_delay_ms = (base_delay_ms as i64 + jitter).max(1) as u64;
        
        Duration::from_millis(final_delay_ms)
    }
    
    /// Get the retry state from either local component state or shared state
    async fn get_retry_state(
        &self, 
        api: &Arc<dyn ComponentRuntimeApi>,
        shared_state_scope: Option<&str>,
        retry_id: Option<&str>,
    ) -> Result<RetryState, CoreError> {
        // If shared state is configured and we have a retry_id, try to load from there first
        if let (Some(scope), Some(id)) = (shared_state_scope, retry_id) {
            let key = format!("retry:{}", id);
            match api.get_shared_state(scope, &key).await {
                Ok(Some(state_value)) => {
                    match serde_json::from_value::<RetryState>(state_value) {
                        Ok(state) => {
                            let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                                &format!("Loaded retry state from shared state: attempt {}/{}", 
                                    state.attempt, state.max_attempts)).await;
                            return Ok(state);
                        }
                        Err(e) => {
                            let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                                &format!("Failed to deserialize shared retry state: {}", e)).await;
                            // Fall through to component state
                        }
                    }
                }
                Ok(None) => {
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                        "No retry state found in shared state, checking component state").await;
                    // Fall through to component state
                }
                Err(e) => {
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                        &format!("Error loading shared retry state: {}", e)).await;
                    // Fall through to component state
                }
            }
        }
        
        // Try to load from component state (either as fallback or primary option)
        match api.get_state().await {
            Ok(Some(state_packet)) => {
                match serde_json::from_value::<RetryState>(state_packet) {
                    Ok(state) => {
                        let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                            &format!("Loaded retry state from component state: attempt {}/{}", 
                                state.attempt, state.max_attempts)).await;
                        Ok(state)
                    }
                    Err(e) => {
                        let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                            &format!("Invalid retry state: {}", e)).await;
                        Err(CoreError::StateStoreError(format!("Invalid retry state: {}", e)))
                    }
                }
            }
            // No state found
            _ => Err(CoreError::ComponentError("No retry state found".to_string())),
        }
    }
    
    /// Save the retry state to either local component state or shared state
    async fn save_retry_state(
        &self, 
        api: &Arc<dyn ComponentRuntimeApi>,
        state: &RetryState,
        shared_state_scope: Option<&str>,
    ) -> Result<(), CoreError> {
        // Always save to component state
        if let Err(e) = api.save_state(json!(state)).await {
            let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                &format!("Failed to save component state: {}", e)).await;
            return Err(e);
        }
        
        // If shared state is configured and we have a retry_id, save there too
        if let (Some(scope), Some(id)) = (shared_state_scope, &state.retry_id) {
            let key = format!("retry:{}", id);
            
            // Get TTL configuration
            let ttl_ms = match api.get_config("sharedStateTtlMs").await {
                Ok(value) => value.as_u64().unwrap_or(24 * 60 * 60 * 1000), // Default 24 hours
                Err(_) => 24 * 60 * 60 * 1000, // 24 hours default TTL
            };
            
            // Use TTL if available
            if let Err(e) = api.set_shared_state_with_ttl(scope, &key, json!(state), ttl_ms).await {
                let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                    &format!("Failed to save to shared state with TTL: {}", e)).await;
                
                // Fall back to regular set_shared_state
                if let Err(e) = api.set_shared_state(scope, &key, json!(state)).await {
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                        &format!("Failed to save to shared state: {}", e)).await;
                    // Continue even if shared state fails - component state was saved
                } else {
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                        &format!("Saved retry state to shared state: attempt {}/{}", 
                            state.attempt, state.max_attempts)).await;
                }
            } else {
                let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                    &format!("Saved retry state to shared state with TTL: attempt {}/{}", 
                        state.attempt, state.max_attempts)).await;
            }
        }
        
        Ok(())
    }
}

impl ComponentExecutorBase for RetryWrapper {
    fn component_type(&self) -> &str {
        "RetryWrapper"
    }
}

impl Default for RetryWrapper {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ComponentExecutor for RetryWrapper {
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
        // Step 1: Get the configuration parameters
        let max_attempts = match api.get_config("maxAttempts").await {
            Ok(value) => value.as_u64().unwrap_or(self.default_max_attempts as u64) as u32,
            Err(_) => self.default_max_attempts,
        };
        
        let initial_delay_ms = match api.get_config("initialDelayMs").await {
            Ok(value) => value.as_u64().unwrap_or(self.default_initial_delay_ms),
            Err(_) => self.default_initial_delay_ms,
        };
        
        let backoff_multiplier = match api.get_config("backoffMultiplier").await {
            Ok(value) => value.as_f64().unwrap_or(self.default_backoff_multiplier),
            Err(_) => self.default_backoff_multiplier,
        };
        
        let max_delay_ms = match api.get_config("maxDelayMs").await {
            Ok(value) => value.as_u64().unwrap_or(self.default_max_delay_ms),
            Err(_) => self.default_max_delay_ms,
        };
        
        // Get shared state configuration
        let shared_state_scope = match api.get_config("sharedStateScope").await {
            Ok(value) => value.as_str().map(|s| s.to_string()),
            Err(_) => None,
        };
        
        // Get optional retry ID (for shared state tracking)
        let retry_id = match api.get_config("retryId").await {
            Ok(value) => value.as_str().map(|s| s.to_string()),
            Err(_) => None,
        };
        
        // Generate a retry ID if we're using shared state but no ID was provided
        let retry_id = if shared_state_scope.is_some() && retry_id.is_none() {
            let correlation = api.correlation_id().await.map_or_else(
                |_| uuid::Uuid::new_v4().to_string(),
                |id| id.0
            );
            Some(format!("{}-{}", correlation, uuid::Uuid::new_v4()))
        } else {
            retry_id
        };
        
        // Step 2: Check if we're in the middle of a retry sequence by loading state
        let state = match self.get_retry_state(&api, shared_state_scope.as_deref(), retry_id.as_deref()).await {
            Ok(state) => state,
            // No state or invalid state, initialize fresh
            Err(_) => RetryState {
                attempt: 1,
                max_attempts,
                initial_delay_ms,
                backoff_multiplier,
                max_delay_ms,
                completed: false,
                final_result: None,
                retry_id,
            },
        };
        
        // Step 3: Handle already completed retries (passthrough cached result)
        if state.completed {
            let _ = ComponentRuntimeApi::log(&*api, LogLevel::Debug, "Retry already completed, passing through result").await;
            
            if let Some(true) = state.final_result {
                // Operation succeeded
                if let Ok(operation_result) = api.get_input("operationResult").await {
                    if let Err(e) = api.set_output("result", operation_result).await {
                        return ExecutionResult::Failure(e);
                    }
                    return ExecutionResult::Success;
                }
            } else {
                // Operation failed
                if let Ok(operation_error) = api.get_input("operationError").await {
                    if let Err(e) = api.set_output("error", operation_error).await {
                        return ExecutionResult::Failure(e);
                    }
                    return ExecutionResult::Success;
                }
                
                // No error input provided, return generic error
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Info, &format!("Retry failed after {} attempts", state.attempt)).await;
                
                let error_output = DataPacket::new(json!({
                    "errorCode": "ERR_RETRY_EXHAUSTED",
                    "errorMessage": format!("Operation failed after {} retry attempts", state.attempt),
                    "component": {
                        "type": "RetryWrapper"
                    },
                    "details": {
                        "attempt": state.attempt,
                        "maxAttempts": state.max_attempts
                    }
                }));
                
                if let Err(e) = api.set_output("error", error_output).await {
                    return ExecutionResult::Failure(e);
                }
            }
            
            return ExecutionResult::Success;
        }
        
        // Step 4: Handle inputs for current attempt
        
        // Check for operation result (success case)
        if let Ok(operation_result) = api.get_input("operationResult").await {
            // Operation succeeded! Update state as completed
            let mut updated_state = state.clone();
            updated_state.completed = true;
            updated_state.final_result = Some(true);
            
            // Save successful state
            let _ = ComponentRuntimeApi::log(&*api, LogLevel::Info, &format!("Operation succeeded on attempt {}", updated_state.attempt)).await;
            
            // Emit metric for successful retry
            let mut labels = HashMap::new();
            labels.insert("component".to_string(), "RetryWrapper".to_string());
            labels.insert("outcome".to_string(), "success".to_string());
            labels.insert("attempts".to_string(), updated_state.attempt.to_string());
            let _ = api.emit_metric("retry_success", 1.0, labels).await;
            
            // Save updated state
            if let Err(e) = self.save_retry_state(&api, &updated_state, shared_state_scope.as_deref()).await {
                return ExecutionResult::Failure(e);
            }
            
            // Output the successful result
            if let Err(e) = api.set_output("result", operation_result).await {
                return ExecutionResult::Failure(e);
            }
            
            return ExecutionResult::Success;
        }
        
        // Step 5: We're in a retry sequence - process current state
        let mut updated_state = state.clone();
        
        // Log current attempt
        let _ = ComponentRuntimeApi::log(&*api, LogLevel::Debug, &format!("Executing retry attempt {}/{}", updated_state.attempt, updated_state.max_attempts)).await;
        
        // Check for operation error (failure case)
        if let Ok(operation_error) = api.get_input("operationError").await {
            // Operation failed - decide if we retry or give up
            
            // If we've reached max attempts, mark as complete failure
            if updated_state.attempt >= updated_state.max_attempts {
                updated_state.completed = true;
                updated_state.final_result = Some(false);
                
                // Emit metric for exhausted retries
                let mut labels = HashMap::new();
                labels.insert("component".to_string(), "RetryWrapper".to_string());
                labels.insert("outcome".to_string(), "exhausted".to_string());
                labels.insert("attempts".to_string(), updated_state.attempt.to_string());
                let _ = api.emit_metric("retry_exhausted", 1.0, labels).await;
                
                // Save the failed state
                if let Err(e) = self.save_retry_state(&api, &updated_state, shared_state_scope.as_deref()).await {
                    return ExecutionResult::Failure(e);
                }
                
                // Log the failure
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Info, &format!("Retry failed after {} attempts", updated_state.attempt)).await;
                
                // Pass through the error output
                if let Err(e) = api.set_output("error", operation_error).await {
                    return ExecutionResult::Failure(e);
                }
                
                return ExecutionResult::Success;
            }
            
            // Still have attempts left - schedule the next retry
            updated_state.attempt += 1;
            
            // Calculate delay for next attempt
            let delay = self.calculate_delay(&updated_state);
            
            // Emit metric for retry attempt
            let mut labels = HashMap::new();
            labels.insert("component".to_string(), "RetryWrapper".to_string());
            labels.insert("outcome".to_string(), "retrying".to_string());
            labels.insert("attempts".to_string(), updated_state.attempt.to_string());
            let _ = api.emit_metric("retry_attempt", 1.0, labels).await;
            
            // Save updated state for next attempt
            if let Err(e) = self.save_retry_state(&api, &updated_state, shared_state_scope.as_deref()).await {
                return ExecutionResult::Failure(CoreError::StateStoreError(
                    format!("Failed to save retry state: {}", e)
                ));
            }
            
            // Log retry schedule
            let _ = ComponentRuntimeApi::log(&*api, LogLevel::Debug, &format!("Operation failed, will retry (attempt {}) after {:?}",
                updated_state.attempt, delay)).await;
            
            // Schedule the timer for the next attempt
            if let Err(e) = api.schedule_timer(delay).await {
                return ExecutionResult::Failure(e);
            }
            
            return ExecutionResult::Pending(format!("Scheduled retry attempt {} after {:?}", updated_state.attempt, delay));
        }
        
        // If we reach here, it means the component is not providing either a result or an error
        // This is not expected - emit a validation error
        let validation_error = DataPacket::new(json!({
            "errorCode": "ERR_INVALID_RETRY_INPUTS",
            "errorMessage": "RetryWrapper requires either 'operationResult' or 'operationError' inputs",
            "component": {
                "type": "RetryWrapper"
            }
        }));
        
        if let Err(e) = api.set_output("error", validation_error).await {
            return ExecutionResult::Failure(e);
        }
        
        ExecutionResult::Failure(CoreError::ValidationError(
            "RetryWrapper requires either 'operationResult' or 'operationError' inputs".to_string()
        ))
    }
} 
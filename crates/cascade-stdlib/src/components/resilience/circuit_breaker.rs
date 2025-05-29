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
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use uuid::Uuid;

/// Represents the possible states of a circuit breaker
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CircuitState {
    /// The circuit is closed, operations are allowed to proceed
    Closed,
    /// The circuit is open, operations will be blocked
    Open,
    /// The circuit is in a probationary state, allowing a test operation
    HalfOpen,
}

impl std::str::FromStr for CircuitState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(CircuitState::Open),
            "half-open" => Ok(CircuitState::HalfOpen),
            "closed" => Ok(CircuitState::Closed),
            _ => Err(format!("Invalid circuit state: {}", s)),
        }
    }
}

impl CircuitState {
    /// Get string representation of the circuit state
    pub fn as_str(&self) -> &'static str {
        match self {
            CircuitState::Closed => "closed",
            CircuitState::Open => "open",
            CircuitState::HalfOpen => "half-open",
        }
    }

    /// Convert a string to circuit state, with a default of Closed
    /// Prefer using the `std::str::FromStr` implementation if you need error handling
    pub fn from_str_fallback(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "open" => CircuitState::Open,
            "half-open" => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }
}

/// Represents the state of a circuit breaker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerState {
    /// Current state of the circuit
    pub state: CircuitState,
    /// Number of consecutive failures
    pub failure_count: u32,
    /// Last time the circuit state changed (in milliseconds since epoch)
    pub last_state_change: u64,
    /// Unique ID for this circuit in shared state
    pub circuit_id: Option<String>,
}

/// Circuit breaker component to prevent repeated calls to failing services
#[derive(Debug, Default)]
pub struct CircuitBreaker {
    // Default values
    default_failure_threshold: u32,
    default_reset_timeout_ms: u64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new() -> Self {
        Self {
            default_failure_threshold: 5,
            default_reset_timeout_ms: 30000, // 30 seconds
        }
    }

    /// Get the current timestamp in milliseconds
    fn current_time_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Get the circuit breaker state from either local component state or shared state
    async fn get_circuit_state(
        &self, 
        api: &Arc<dyn ComponentRuntimeApi>,
        shared_state_scope: Option<&str>,
        circuit_id: Option<&str>,
    ) -> Result<CircuitBreakerState, CoreError> {
        // If shared state is configured and we have a circuit_id, try to load from there first
        if let (Some(scope), Some(id)) = (shared_state_scope, circuit_id) {
            let key = format!("circuit:{}", id);
            match api.get_shared_state(scope, &key).await {
                Ok(Some(state_value)) => {
                    match serde_json::from_value::<CircuitBreakerState>(state_value) {
                        Ok(state) => {
                            let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                                &format!("Loaded circuit state from shared state: {}", 
                                    state.state.as_str())).await;
                            return Ok(state);
                        }
                        Err(e) => {
                            let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                                &format!("Failed to deserialize shared circuit state: {}", e)).await;
                            // Fall through to component state
                        }
                    }
                }
                Ok(None) => {
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                        "No circuit state found in shared state, checking component state").await;
                    // Fall through to component state
                }
                Err(e) => {
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                        &format!("Error loading shared circuit state: {}", e)).await;
                    // Fall through to component state
                }
            }
        }
        
        // Try to load from component state (either as fallback or primary option)
        match api.get_state().await {
            Ok(Some(state_packet)) => {
                match serde_json::from_value::<CircuitBreakerState>(state_packet) {
                    Ok(state) => {
                        let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                            &format!("Loaded circuit state from component state: {}", 
                                state.state.as_str())).await;
                        Ok(state)
                    }
                    Err(e) => {
                        let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                            &format!("Invalid circuit state: {}", e)).await;
                        
                        // Initialize with closed state as fallback
                        Ok(CircuitBreakerState {
                            state: CircuitState::Closed,
                            failure_count: 0,
                            last_state_change: Self::current_time_ms(),
                            circuit_id: circuit_id.map(|s| s.to_string()),
                        })
                    }
                }
            }
            // No state found - initialize new state
            _ => Ok(CircuitBreakerState {
                state: CircuitState::Closed,
                failure_count: 0,
                last_state_change: Self::current_time_ms(),
                circuit_id: circuit_id.map(|s| s.to_string()),
            }),
        }
    }
    
    /// Save the circuit state to either local component state or shared state
    async fn save_circuit_state(
        &self, 
        api: &Arc<dyn ComponentRuntimeApi>,
        state: &CircuitBreakerState,
        shared_state_scope: Option<&str>,
    ) -> Result<(), CoreError> {
        // Always save to component state
        if let Err(e) = api.save_state(json!(state)).await {
            let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                &format!("Failed to save component state: {}", e)).await;
            return Err(e);
        }
        
        // If shared state is configured and we have a circuit_id, save there too
        if let (Some(scope), Some(id)) = (shared_state_scope, &state.circuit_id) {
            let key = format!("circuit:{}", id);
            
            // Get TTL configuration with a default of 24 hours (86400000ms)
            let ttl_ms = match api.get_config("sharedStateTtlMs").await {
                Ok(value) => value.as_u64().unwrap_or(86400000), // 24 hours default
                Err(_) => 86400000, // 24 hours default
            };
            
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
                        &format!("Saved circuit state to shared state: {}", 
                            state.state.as_str())).await;
                }
            } else {
                let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                    &format!("Saved circuit state to shared state with TTL: {}", 
                        state.state.as_str())).await;
            }
        }
        
        Ok(())
    }
}

impl ComponentExecutorBase for CircuitBreaker {
    fn component_type(&self) -> &str {
        "CircuitBreaker"
    }
}

#[async_trait]
impl ComponentExecutor for CircuitBreaker {
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
        // Step 1: Get the configuration parameters
        let failure_threshold = match api.get_config("failureThreshold").await {
            Ok(value) => value.as_u64().unwrap_or(self.default_failure_threshold as u64) as u32,
            Err(_) => self.default_failure_threshold,
        };
        
        let reset_timeout_ms = match api.get_config("resetTimeoutMs").await {
            Ok(value) => value.as_u64().unwrap_or(self.default_reset_timeout_ms),
            Err(_) => self.default_reset_timeout_ms,
        };
        
        // Get shared state configuration
        let shared_state_scope = match api.get_config("sharedStateScope").await {
            Ok(value) => value.as_str().map(|s| s.to_string()),
            Err(_) => None,
        };

        // Get optional circuit ID (for shared state tracking)
        let circuit_id = match api.get_config("circuitId").await {
            Ok(value) => value.as_str().map(|s| s.to_string()),
            Err(_) => None,
        };
        
        // Generate a circuit ID if we're using shared state but no ID was provided
        let circuit_id = if shared_state_scope.is_some() && circuit_id.is_none() {
            // Use correlation ID or generate UUID for scoping
            let correlation = api.correlation_id().await.map_or_else(
                |_| Uuid::new_v4().to_string(),
                |id| id.0
            );
            Some(format!("{}-{}", correlation, "circuit"))
        } else {
            circuit_id
        };
        
        // Get the description field for logging and metrics
        let circuit_description = match api.get_config("description").await {
            Ok(value) => value.as_str().map(|s| s.to_string()).unwrap_or_else(|| 
                circuit_id.clone().unwrap_or_else(|| "unnamed-circuit".to_string())
            ),
            Err(_) => circuit_id.clone().unwrap_or_else(|| "unnamed-circuit".to_string()),
        };
        
        // Step 2: Check if we're in the middle of a circuit sequence
        let state = match self.get_circuit_state(&api, shared_state_scope.as_deref(), circuit_id.as_deref()).await {
            Ok(state) => state,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Step 3: Check if operation result is already present (short-circuit path for "after" handling)
        if let Ok(operation_result) = api.get_input("operationResult").await {
            // Success - reset failure count if needed
            let mut updated_state = state.clone();
            let now = Self::current_time_ms();
            
            // When successful in half-open state, close the circuit
            if state.state == CircuitState::HalfOpen {
                updated_state.state = CircuitState::Closed;
                updated_state.failure_count = 0;
                updated_state.last_state_change = now;
                
                // Log and emit metric for circuit closed after recovery
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Info, 
                    &format!("Circuit '{}' closed after successful test request", circuit_description)).await;
                
                let mut labels = HashMap::new();
                labels.insert("component".to_string(), "CircuitBreaker".to_string());
                labels.insert("circuit".to_string(), circuit_description.clone());
                labels.insert("state".to_string(), "CLOSED".to_string());
                let _ = api.emit_metric("circuit_state_change", 1.0, labels).await;
            } 
            // In closed state, reset the failure counter
            else if state.state == CircuitState::Closed && state.failure_count > 0 {
                updated_state.failure_count = 0;
                
                // Log success after previous failures
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Debug, 
                    &format!("Circuit '{}' reset failure count after success", circuit_description)).await;
            }
            
            // Save updated state if it changed
            if updated_state.state != state.state || updated_state.failure_count != state.failure_count {
                if let Err(e) = self.save_circuit_state(&api, &updated_state, shared_state_scope.as_deref()).await {
                    return ExecutionResult::Failure(e);
                }
            }

            // Pass through the result
            if let Err(e) = api.set_output("result", operation_result).await {
                return ExecutionResult::Failure(e);
            }
            
            return ExecutionResult::Success;
        }
        
        // Step 4: Check if operation error is present (circuit tracking)
        if let Ok(operation_error) = api.get_input("operationError").await {
            // Failure - increment failure count and potentially open circuit
            let mut updated_state = state.clone();
            let now = Self::current_time_ms();
            
            // Increment failure counter
            updated_state.failure_count += 1;
            
            // Check if we should open the circuit
            if (state.state == CircuitState::Closed || state.state == CircuitState::HalfOpen) 
                && updated_state.failure_count >= failure_threshold {
                
                updated_state.state = CircuitState::Open;
                updated_state.last_state_change = now;
                
                // Log and emit metric for circuit opened
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Warn, 
                    &format!("Circuit '{}' opened after {} consecutive failures", 
                        circuit_description, updated_state.failure_count)).await;
                
                let mut labels = HashMap::new();
                labels.insert("component".to_string(), "CircuitBreaker".to_string());
                labels.insert("circuit".to_string(), circuit_description.clone());
                labels.insert("state".to_string(), "OPEN".to_string());
                let _ = api.emit_metric("circuit_state_change", 1.0, labels).await;
            }

            // Save updated state
            if let Err(e) = self.save_circuit_state(&api, &updated_state, shared_state_scope.as_deref()).await {
                return ExecutionResult::Failure(e);
            }
            
            // Pass through the error
            if let Err(e) = api.set_output("error", operation_error).await {
                return ExecutionResult::Failure(e);
            }
            
            return ExecutionResult::Success;
        }
        
        // Step 5: No result or error means we're checking if circuit allows operation
        // Check if circuit is open
        if state.state == CircuitState::Open {
            let now = Self::current_time_ms();

            // Check if we should try half-open state (timeout elapsed)
            if now - state.last_state_change >= reset_timeout_ms {
                // Transition to half-open state
                let mut updated_state = state.clone();
                updated_state.state = CircuitState::HalfOpen;
                updated_state.last_state_change = now;
                
                // Log and emit metric for circuit half-open
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Info, 
                    &format!("Circuit '{}' half-open, allowing test request after timeout", 
                        circuit_description)).await;
                
                let mut labels = HashMap::new();
                labels.insert("component".to_string(), "CircuitBreaker".to_string());
                labels.insert("circuit".to_string(), circuit_description.clone());
                labels.insert("state".to_string(), "HALF_OPEN".to_string());
                let _ = api.emit_metric("circuit_state_change", 1.0, labels).await;
                
                // Save updated state
                if let Err(e) = self.save_circuit_state(&api, &updated_state, shared_state_scope.as_deref()).await {
                    return ExecutionResult::Failure(e);
                }
                
                // Allow operation to proceed
                let output = DataPacket::new(json!({
                    "circuitState": "HALF_OPEN",
                    "circuit": circuit_description,
                    "allowOperation": true
                }));
                
                if let Err(e) = api.set_output("status", output).await {
                    return ExecutionResult::Failure(e);
                }
                
                return ExecutionResult::Success;
            } else {
                // Circuit is still open, block operation
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Debug, 
                    &format!("Circuit '{}' is open, blocking operation", circuit_description)).await;
                
                let error_output = DataPacket::new(json!({
                    "errorCode": "ERR_CIRCUIT_OPEN",
                    "errorMessage": format!("Circuit '{}' is open", circuit_description),
                    "component": {
                        "type": "CircuitBreaker"
                    },
                    "details": {
                        "circuit": circuit_description,
                        "failures": state.failure_count,
                        "resetTimeoutMs": reset_timeout_ms,
                        "lastStateChange": state.last_state_change,
                        "currentTime": Self::current_time_ms(),
                        "remainingMs": reset_timeout_ms.saturating_sub(Self::current_time_ms() - state.last_state_change)
                    }
                }));
                
                if let Err(e) = api.set_output("error", error_output).await {
                    return ExecutionResult::Failure(e);
                }

                // Also set status output
                let status_output = DataPacket::new(json!({
                    "circuitState": "OPEN",
                    "circuit": circuit_description,
                    "allowOperation": false
                }));
                
                if let Err(e) = api.set_output("status", status_output).await {
                    return ExecutionResult::Failure(e);
                }
                
                return ExecutionResult::Success;
            }
        } else {
            // Circuit is closed or half-open, allow operation
            let state_str = if state.state == CircuitState::Closed { "CLOSED" } else { "HALF_OPEN" };
            
            let _ = ComponentRuntimeApi::log(&*api, LogLevel::Debug, 
                &format!("Circuit '{}' is {}, allowing operation", circuit_description, state_str)).await;
            
            let output = DataPacket::new(json!({
                "circuitState": state_str,
                "circuit": circuit_description,
                "allowOperation": true
            }));
            
            if let Err(e) = api.set_output("status", output).await {
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
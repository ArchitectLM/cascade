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
use tokio::sync::Semaphore;
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};

/// Bulkhead component state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkheadState {
    /// Maximum concurrent executions allowed
    pub max_concurrent: u32,
    
    /// Current number of active executions
    pub active_count: u32,
    
    /// Unique ID for this bulkhead in shared state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bulkhead_id: Option<String>,
    
    /// Last updated timestamp (in milliseconds since epoch)
    pub last_updated: u64,
}

/// Bulkhead component for limiting concurrent execution
#[derive(Debug, Default)]
pub struct Bulkhead {
    // Default values
    default_max_concurrent: u32,
    
    // Local concurrency control
    semaphores: tokio::sync::Mutex<HashMap<String, Arc<Semaphore>>>,
}

impl Bulkhead {
    /// Create a new bulkhead
    pub fn new() -> Self {
        Self {
            default_max_concurrent: 10,
            semaphores: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
    
    /// Get the current timestamp in milliseconds since epoch
    fn current_time_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
    
    /// Get or create a semaphore for the given bulkhead ID
    async fn get_semaphore(&self, id: &str, max_concurrent: u32) -> Arc<Semaphore> {
        let mut semaphores = self.semaphores.lock().await;
        
        if let Some(semaphore) = semaphores.get(id) {
            return semaphore.clone();
        }
        
        // Create new semaphore
        let semaphore = Arc::new(Semaphore::new(max_concurrent as usize));
        semaphores.insert(id.to_string(), semaphore.clone());
        semaphore
    }
    
    /// Get the bulkhead state from either local component state or shared state
    async fn get_bulkhead_state(
        &self, 
        api: &Arc<dyn ComponentRuntimeApi>,
        shared_state_scope: Option<&str>,
        bulkhead_id: Option<&str>,
    ) -> Result<BulkheadState, CoreError> {
        // If shared state is configured and we have a bulkhead_id, try to load from there first
        if let (Some(scope), Some(id)) = (shared_state_scope, bulkhead_id) {
            let key = format!("bulkhead:{}", id);
            match api.get_shared_state(scope, &key).await {
                Ok(Some(state_value)) => {
                    match serde_json::from_value::<BulkheadState>(state_value) {
                        Ok(state) => {
                            let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                                &format!("Loaded bulkhead state from shared state: active={}/{}", 
                                    state.active_count, state.max_concurrent)).await;
                            return Ok(state);
                        }
                        Err(e) => {
                            let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                                &format!("Failed to deserialize shared bulkhead state: {}", e)).await;
                            // Fall through to component state
                        }
                    }
                }
                Ok(None) => {
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                        "No bulkhead state found in shared state, checking component state").await;
                    // Fall through to component state
                }
                Err(e) => {
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                        &format!("Error loading shared bulkhead state: {}", e)).await;
                    // Fall through to component state
                }
            }
        }
        
        // Try to load from component state (either as fallback or primary option)
        match api.get_state().await {
            Ok(Some(state_packet)) => {
                match serde_json::from_value::<BulkheadState>(state_packet) {
                    Ok(state) => {
                        let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                            &format!("Loaded bulkhead state from component state: active={}/{}", 
                                state.active_count, state.max_concurrent)).await;
                        Ok(state)
                    }
                    Err(e) => {
                        let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                            &format!("Invalid bulkhead state: {}", e)).await;
                        
                        // Initialize with default state as fallback
                        Ok(BulkheadState {
                            max_concurrent: self.default_max_concurrent,
                            active_count: 0,
                            bulkhead_id: bulkhead_id.map(|s| s.to_string()),
                            last_updated: Self::current_time_ms(),
                        })
                    }
                }
            }
            // No state found - initialize new state
            _ => Ok(BulkheadState {
                max_concurrent: self.default_max_concurrent,
                active_count: 0,
                bulkhead_id: bulkhead_id.map(|s| s.to_string()),
                last_updated: Self::current_time_ms(),
            }),
        }
    }
    
    /// Save the bulkhead state to either local component state or shared state
    async fn save_bulkhead_state(
        &self, 
        api: &Arc<dyn ComponentRuntimeApi>,
        state: &BulkheadState,
        shared_state_scope: Option<&str>,
    ) -> Result<(), CoreError> {
        // Always save to component state
        if let Err(e) = api.save_state(json!(state)).await {
            let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                &format!("Failed to save component state: {}", e)).await;
            return Err(e);
        }
        
        // If shared state is configured and we have a bulkhead_id, save there too
        if let (Some(scope), Some(id)) = (shared_state_scope, &state.bulkhead_id) {
            let key = format!("bulkhead:{}", id);
            if let Err(e) = api.set_shared_state(scope, &key, json!(state)).await {
                let _ = ComponentRuntimeApi::log(&**api, LogLevel::Warn, 
                    &format!("Failed to save to shared state: {}", e)).await;
                // Continue even if shared state fails - component state was saved
            } else {
                let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                    &format!("Saved bulkhead state to shared state: active={}/{}", 
                        state.active_count, state.max_concurrent)).await;
            }
        }
        
        Ok(())
    }
    
    /// Acquire a permit from the bulkhead, or return false if the bulkhead is full
    async fn try_acquire(
        &self, 
        api: &Arc<dyn ComponentRuntimeApi>,
        state: &mut BulkheadState,
        shared_state_scope: Option<&str>,
        local_semaphore: &Arc<Semaphore>,
    ) -> Result<bool, CoreError> {
        // First try to acquire a local semaphore permit
        let permit = match local_semaphore.try_acquire() {
            Ok(_) => {
                // Acquired local permit, now update the shared state
                state.active_count += 1;
                state.last_updated = Self::current_time_ms();
                
                let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                    &format!("Acquired bulkhead permit, active={}/{}", 
                        state.active_count, state.max_concurrent)).await;
                
                // Save updated state
                if let Err(e) = self.save_bulkhead_state(api, state, shared_state_scope).await {
                    // Failed to save state, release the permit and return error
                    let _ = ComponentRuntimeApi::log(&**api, LogLevel::Error, 
                        &format!("Failed to save bulkhead state: {}", e)).await;
                    return Err(e);
                }
                
                true
            },
            Err(_) => {
                // Could not acquire local permit
                let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
                    &format!("Bulkhead is full, active={}/{}", 
                        state.active_count, state.max_concurrent)).await;
                false
            }
        };
        
        Ok(permit)
    }
    
    /// Release a permit from the bulkhead
    async fn release_permit(
        &self, 
        api: &Arc<dyn ComponentRuntimeApi>,
        state: &mut BulkheadState,
        shared_state_scope: Option<&str>,
    ) -> Result<(), CoreError> {
        // Update the state
        if state.active_count > 0 {
            state.active_count -= 1;
        }
        state.last_updated = Self::current_time_ms();
        
        let _ = ComponentRuntimeApi::log(&**api, LogLevel::Debug, 
            &format!("Released bulkhead permit, active={}/{}", 
                state.active_count, state.max_concurrent)).await;
        
        // Save updated state
        self.save_bulkhead_state(api, state, shared_state_scope).await
    }
}

impl ComponentExecutorBase for Bulkhead {
    fn component_type(&self) -> &str {
        "Bulkhead"
    }
}

#[async_trait]
impl ComponentExecutor for Bulkhead {
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
        // Step 1: Get configuration parameters
        let max_concurrent = match api.get_config("maxConcurrent").await {
            Ok(value) => value.as_u64().unwrap_or(self.default_max_concurrent as u64) as u32,
            Err(_) => self.default_max_concurrent,
        };
        
        // Get shared state configuration
        let shared_state_scope = match api.get_config("sharedStateScope").await {
            Ok(value) => value.as_str().map(|s| s.to_string()),
            Err(_) => None,
        };
        
        // Get optional bulkhead ID (for shared state tracking)
        let bulkhead_id = match api.get_config("bulkheadId").await {
            Ok(value) => value.as_str().map(|s| s.to_string()),
            Err(_) => None,
        };
        
        // Generate a bulkhead ID if we're using shared state but no ID was provided
        let bulkhead_id = if shared_state_scope.is_some() && bulkhead_id.is_none() {
            // Use correlation ID or generate UUID for scoping
            let correlation = api.correlation_id().await.map_or_else(
                |_| Uuid::new_v4().to_string(),
                |id| id.0
            );
            Some(format!("{}-bulkhead", correlation))
        } else {
            bulkhead_id
        };
        
        // Get the description field for logging and metrics
        let bulkhead_description = match api.get_config("description").await {
            Ok(value) => value.as_str().map(|s| s.to_string()).unwrap_or_else(|| 
                bulkhead_id.clone().unwrap_or_else(|| "unnamed-bulkhead".to_string())
            ),
            Err(_) => bulkhead_id.clone().unwrap_or_else(|| "unnamed-bulkhead".to_string()),
        };
        
        // Step 2: Get or initialize bulkhead state
        let mut state = match self.get_bulkhead_state(&api, shared_state_scope.as_deref(), bulkhead_id.as_deref()).await {
            Ok(state) => state,
            Err(e) => {
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Error, 
                    &format!("Failed to get bulkhead state: {}", e)).await;
                return ExecutionResult::Failure(e);
            }
        };
        
        // Make sure max_concurrent is updated from config if changed
        if state.max_concurrent != max_concurrent {
            state.max_concurrent = max_concurrent;
        }
        
        // Step 3: Get or create local semaphore
        let semaphore_id = bulkhead_id.clone().unwrap_or_else(|| bulkhead_description.clone());
        let semaphore = self.get_semaphore(&semaphore_id, max_concurrent).await;
        
        // Step 4: Try to acquire a permit
        let acquired = match self.try_acquire(&api, &mut state, shared_state_scope.as_deref(), &semaphore).await {
            Ok(acquired) => acquired,
            Err(e) => {
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Error, 
                    &format!("Failed to acquire bulkhead permit: {}", e)).await;
                return ExecutionResult::Failure(e);
            }
        };
        
        // Step 5: Handle result based on whether we acquired a permit
        if acquired {
            // We acquired a permit, emit metric
            let mut labels = HashMap::new();
            labels.insert("component".to_string(), "Bulkhead".to_string());
            labels.insert("bulkhead".to_string(), bulkhead_description.clone());
            let _ = api.emit_metric("bulkhead_active", state.active_count as f64, labels.clone()).await;
            let _ = api.emit_metric("bulkhead_request_allowed", 1.0, labels).await;
            
            // Set output to indicate allowed status
            let output = DataPacket::new(json!({
                "allowed": true,
                "active": state.active_count,
                "capacity": state.max_concurrent,
                "bulkhead": bulkhead_description
            }));
            
            if let Err(e) = api.set_output("status", output).await {
                // Make sure to release the permit on error
                let _ = self.release_permit(&api, &mut state, shared_state_scope.as_deref()).await;
                return ExecutionResult::Failure(e);
            }
            
            // Check if request is in the context, if so, pass it through
            if let Ok(request) = api.get_input("request").await {
                if let Err(e) = api.set_output("result", request).await {
                    // Make sure to release the permit on error
                    let _ = self.release_permit(&api, &mut state, shared_state_scope.as_deref()).await;
                    return ExecutionResult::Failure(e);
                }
            }
            
            // Get input data to pass through
            let data = match api.get_input("data").await {
                Ok(data) => Some(data),
                Err(_) => None, // Input is optional
            };
            
            // Since we acquired a permit, we need to set up a callback to release it
            // when the operation is complete
            
            // Check for result or error input, indicating the operation completed
            if let Ok(result) = api.get_input("operationResult").await {
                // Operation succeeded, release permit and pass through result
                if let Err(e) = self.release_permit(&api, &mut state, shared_state_scope.as_deref()).await {
                    let _ = ComponentRuntimeApi::log(&*api, LogLevel::Warn, 
                        &format!("Failed to release bulkhead permit: {}", e)).await;
                    // Continue even if we fail to release the permit
                }
                
                if let Err(e) = api.set_output("result", result).await {
                    return ExecutionResult::Failure(e);
                }
                
                return ExecutionResult::Success;
            } else if let Ok(error) = api.get_input("operationError").await {
                // Operation failed, release permit and pass through error
                if let Err(e) = self.release_permit(&api, &mut state, shared_state_scope.as_deref()).await {
                    let _ = ComponentRuntimeApi::log(&*api, LogLevel::Warn, 
                        &format!("Failed to release bulkhead permit: {}", e)).await;
                    // Continue even if we fail to release the permit
                }
                
                if let Err(e) = api.set_output("error", error).await {
                    return ExecutionResult::Failure(e);
                }
                
                return ExecutionResult::Success;
            } else if data.is_some() {
                // We have data to pass through, but no operation result yet
                // This is expected when we're in the initial execution phase
                // Pass through the data input
                if let Err(e) = api.set_output("data", data.unwrap()).await {
                    // Make sure to release the permit on error
                    let _ = self.release_permit(&api, &mut state, shared_state_scope.as_deref()).await;
                    return ExecutionResult::Failure(e);
                }
                
                return ExecutionResult::Success;
            } else {
                // No operation result or data input, just return success
                return ExecutionResult::Success;
            }
        } else {
            // We could not acquire a permit, emit metric and return error
            let mut labels = HashMap::new();
            labels.insert("component".to_string(), "Bulkhead".to_string());
            labels.insert("bulkhead".to_string(), bulkhead_description.clone());
            let _ = api.emit_metric("bulkhead_request_rejected", 1.0, labels).await;
            
            // Set output to indicate rejected status
            let status_output = DataPacket::new(json!({
                "allowed": false,
                "active": state.active_count,
                "capacity": state.max_concurrent,
                "bulkhead": bulkhead_description
            }));
            
            if let Err(e) = api.set_output("status", status_output).await {
                return ExecutionResult::Failure(e);
            }
            
            // Create error output
            let error_output = DataPacket::new(json!({
                "errorCode": "ERR_BULKHEAD_FULL",
                "errorMessage": format!("Bulkhead '{}' is at capacity ({}/{})",
                    bulkhead_description, state.active_count, state.max_concurrent),
                "component": {
                    "type": "Bulkhead"
                },
                "details": {
                    "active": state.active_count,
                    "capacity": state.max_concurrent
                }
            }));
            
            if let Err(e) = api.set_output("error", error_output).await {
                return ExecutionResult::Failure(e);
            }
            
            // Return success even though the bulkhead is full
            // This allows the flow to handle the error appropriately
            return ExecutionResult::Success;
        }
    }
} 
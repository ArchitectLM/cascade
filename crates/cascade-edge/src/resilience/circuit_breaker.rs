use std::sync::Arc;
use std::time::{Instant, Duration};
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};
use serde_json::json;
use cascade_core::CoreError;
use tracing::{debug, warn, info, error, trace};

use super::StateStore;

/// Circuit breaker states
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are rejected
    Open,
    /// Circuit is half-open, limited number of requests allowed
    HalfOpen,
}

impl ToString for CircuitBreakerState {
    fn to_string(&self) -> String {
        match self {
            CircuitBreakerState::Closed => "CLOSED".to_string(),
            CircuitBreakerState::Open => "OPEN".to_string(),
            CircuitBreakerState::HalfOpen => "HALF_OPEN".to_string(),
        }
    }
}

impl From<String> for CircuitBreakerState {
    fn from(s: String) -> Self {
        match s.as_str() {
            "CLOSED" => CircuitBreakerState::Closed,
            "OPEN" => CircuitBreakerState::Open,
            "HALF_OPEN" => CircuitBreakerState::HalfOpen,
            _ => CircuitBreakerState::Closed, // Default to closed
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of failures needed to trip the circuit
    pub failure_threshold: u32,
    /// Timeout in milliseconds to wait before trying again
    pub reset_timeout_ms: u64,
    /// Maximum number of calls allowed in half-open state
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_timeout_ms: 5000,
            half_open_max_calls: 1,
        }
    }
}

/// Circuit breaker implementation
#[derive(Clone)]
pub struct CircuitBreaker {
    id: String,
    config: CircuitBreakerConfig,
    state_store: Arc<dyn StateStore>,
    state: Arc<Mutex<CircuitBreakerInternalState>>,
}

/// Internal state for the circuit breaker
struct CircuitBreakerInternalState {
    state: CircuitBreakerState,
    failure_count: u32,
    last_failure_time: Option<Instant>,
    half_open_calls: u32,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(
        id: String,
        config: CircuitBreakerConfig,
        state_store: Arc<dyn StateStore>,
    ) -> Self {
        debug!("Creating new circuit breaker with id: {}, config: {:?}", id, config);
        let state = Arc::new(Mutex::new(CircuitBreakerInternalState {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            last_failure_time: None,
            half_open_calls: 0,
        }));

        let circuit = Self {
            id,
            config,
            state_store,
            state,
        };

        // Start async task to load state
        let circuit_clone = circuit.clone();
        tokio::spawn(async move {
            debug!("Loading circuit breaker state for id: {}", circuit_clone.id);
            if let Err(e) = circuit_clone.load_state().await {
                error!("Failed to load circuit breaker state for {}: {}", circuit_clone.id, e);
            } else {
                debug!("Successfully loaded circuit breaker state for id: {}", circuit_clone.id);
            }
        });

        circuit
    }

    /// Execute an operation with circuit breaker protection
    pub async fn execute<F, T, E>(&self, operation: F) -> Result<T, CoreError>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: Into<CoreError> + std::fmt::Debug,
    {
        debug!("Circuit breaker {} executing operation", self.id);
        
        // Check if circuit is open - capture decision and release lock before operation
        let can_execute = {
            let mut internal_state = self.state.lock().await;
            trace!("Current state for circuit {}: {:?}, failure count: {}, half_open_calls: {}", 
                  self.id, internal_state.state, internal_state.failure_count, internal_state.half_open_calls);
            
            let decision = match internal_state.state {
                CircuitBreakerState::Closed => {
                    debug!("Circuit {} is closed, allowing execution", self.id);
                    true
                },
                CircuitBreakerState::Open => {
                    // Check if it's time to try again
                    if let Some(last_time) = internal_state.last_failure_time {
                        let elapsed = last_time.elapsed();
                        debug!("Circuit {} is open, last failure was {:?} ago (reset timeout: {}ms)", 
                              self.id, elapsed, self.config.reset_timeout_ms);
                        
                        if elapsed > Duration::from_millis(self.config.reset_timeout_ms) {
                            // Transition to half-open
                            internal_state.state = CircuitBreakerState::HalfOpen;
                            internal_state.half_open_calls = 0;
                            info!("Circuit breaker {} transitioned to half-open after timeout period", self.id);
                            // Save state after changing it
                            drop(internal_state); // Release lock before async operation
                            if let Err(e) = self.save_state().await {
                                error!("Failed to save circuit breaker state after transition to half-open: {}", e);
                                return Err(e);
                            }
                            true
                        } else {
                            debug!("Circuit {} remains open, reset timeout not reached", self.id);
                            false
                        }
                    } else {
                        error!("Circuit {} is open but has no last_failure_time", self.id);
                        false
                    }
                }
                CircuitBreakerState::HalfOpen => {
                    // Only allow a limited number of calls
                    debug!("Circuit {} is half-open with {}/{} calls", 
                          self.id, internal_state.half_open_calls, self.config.half_open_max_calls);
                    
                    if internal_state.half_open_calls < self.config.half_open_max_calls {
                        internal_state.half_open_calls += 1;
                        debug!("Allowing execution in half-open state, calls now {}/{}", 
                              internal_state.half_open_calls, self.config.half_open_max_calls);
                        
                        // Save state after incrementing half-open calls
                        drop(internal_state); // Release lock before async operation
                        if let Err(e) = self.save_state().await {
                            error!("Failed to save circuit breaker state after incrementing half-open calls: {}", e);
                            return Err(e);
                        }
                        true
                    } else {
                        debug!("Rejecting execution in half-open state, max calls reached");
                        false
                    }
                }
            };
            
            // Return decision without holding lock during await points
            decision
        };

        if !can_execute {
            info!("Circuit breaker {} rejected execution", self.id);
            return Err(CoreError::ComponentError(format!(
                "Circuit breaker {} is open",
                self.id
            )));
        }

        // Execute the operation without holding the lock
        debug!("Circuit breaker {} executing protected operation", self.id);
        let result = operation.await;

        // Update circuit state based on result
        // We'll get a fresh lock after the operation completes
        {
            let current_state = self.get_state().await?;
            let mut internal_state = self.state.lock().await;
            
            match &result {
                Ok(_) => {
                    debug!("Circuit breaker {} operation succeeded", self.id);
                    // Reset on success
                    if current_state == CircuitBreakerState::HalfOpen {
                        // Successful call in half-open state closes the circuit
                        internal_state.state = CircuitBreakerState::Closed;
                        internal_state.failure_count = 0;
                        internal_state.half_open_calls = 0;
                        info!("Circuit breaker {} closed after successful half-open call", self.id);
                    }
                }
                Err(e) => {
                    debug!("Circuit breaker {} operation failed: {:?}", self.id, e);
                    // Record failure
                    internal_state.last_failure_time = Some(Instant::now());

                    match current_state {
                        CircuitBreakerState::Closed => {
                            internal_state.failure_count += 1;
                            debug!("Circuit {} failure count increased to {}/{}", 
                                  self.id, internal_state.failure_count, self.config.failure_threshold);
                            
                            if internal_state.failure_count >= self.config.failure_threshold {
                                // Trip the circuit
                                internal_state.state = CircuitBreakerState::Open;
                                warn!(
                                    "Circuit breaker {} opened after {} consecutive failures",
                                    self.id, internal_state.failure_count
                                );
                            }
                        }
                        CircuitBreakerState::HalfOpen => {
                            // Failure in half-open state opens the circuit again
                            internal_state.state = CircuitBreakerState::Open;
                            warn!("Circuit breaker {} reopened after half-open failure", self.id);
                        }
                        CircuitBreakerState::Open => {
                            // Should not happen, just increment failure count
                            internal_state.failure_count += 1;
                            error!("Unexpected failure in OPEN state for circuit {}", self.id);
                        }
                    }
                }
            }
        }
        
        // Save state after updating it (without holding the lock)
        if let Err(e) = self.save_state().await {
            error!("Failed to save circuit breaker state after operation: {}", e);
        }

        match result {
            Ok(value) => Ok(value),
            Err(e) => Err(e.into()),
        }
    }

    /// Get the current state of the circuit breaker
    pub async fn get_state(&self) -> Result<CircuitBreakerState, CoreError> {
        let mut internal_state = self.state.lock().await;
        trace!("Getting circuit breaker {} state: {:?}", self.id, internal_state.state);
        
        // If in open state, check if it's time to transition to half-open
        if internal_state.state == CircuitBreakerState::Open {
            if let Some(last_time) = internal_state.last_failure_time {
                let elapsed = last_time.elapsed();
                trace!("Circuit {} is open, last failure was {:?} ago (reset_timeout: {}ms)", 
                      self.id, elapsed, self.config.reset_timeout_ms);
                
                if elapsed > Duration::from_millis(self.config.reset_timeout_ms) {
                    // It's time to transition to half-open state
                    info!("Transitioning circuit {} from Open to HalfOpen during get_state", self.id);
                    internal_state.state = CircuitBreakerState::HalfOpen;
                    internal_state.half_open_calls = 0;
                    
                    // Drop lock before saving state
                    let current_state = internal_state.state;
                    drop(internal_state);
                    
                    // Asynchronously save state without blocking get_state
                    let circuit_id = self.id.clone();
                    let state_store = self.state_store.clone();
                    tokio::spawn(async move {
                        debug!("Saving half-open transition state for circuit {}", circuit_id);
                        let state = json!({
                            "state": current_state.to_string(),
                            "failure_count": 0,
                            "half_open_calls": 0
                        });
                        
                        if let Err(e) = state_store.set_component_state(&circuit_id, state).await {
                            error!("Failed to save circuit breaker {} state after transition to half-open: {}", circuit_id, e);
                        }
                    });
                    
                    return Ok(CircuitBreakerState::HalfOpen);
                }
            }
        }
        
        Ok(internal_state.state)
    }

    /// Save the circuit breaker state
    async fn save_state(&self) -> Result<(), CoreError> {
        // Get a snapshot of the current state without holding the lock during the save operation
        let state_snapshot = {
            let internal_state = self.state.lock().await;
            json!({
                "state": internal_state.state.to_string(),
                "failure_count": internal_state.failure_count,
                "half_open_calls": internal_state.half_open_calls
            })
        };
        
        debug!("Saving circuit breaker {} state: {:?}", self.id, state_snapshot);
        match self.state_store.set_component_state(&self.id, state_snapshot).await {
            Ok(_) => {
                debug!("Successfully saved circuit breaker {} state", self.id);
                Ok(())
            },
            Err(e) => {
                error!("Failed to save circuit breaker {} state: {}", self.id, e);
                Err(e)
            }
        }
    }

    /// Load the circuit breaker state
    async fn load_state(&self) -> Result<(), CoreError> {
        debug!("Loading circuit breaker state for {}", self.id);
        let mut internal_state = self.state.lock().await;
        
        // Load from the state store
        match self.state_store.get_component_state(&self.id).await {
            Ok(maybe_state) => {
                if let Some(state) = maybe_state {
                    debug!("Found persisted state for circuit breaker {}: {:?}", self.id, state);
                    
                    if let Some(state_str) = state.get("state").and_then(|s| s.as_str()) {
                        internal_state.state = CircuitBreakerState::from(state_str.to_string());
                        debug!("Loaded circuit state: {:?}", internal_state.state);
                    } else {
                        debug!("No 'state' field found in persisted state");
                    }
                    
                    if let Some(failure_count) = state.get("failure_count").and_then(|c| c.as_u64()) {
                        internal_state.failure_count = failure_count as u32;
                        debug!("Loaded failure count: {}", internal_state.failure_count);
                    } else {
                        debug!("No 'failure_count' field found in persisted state");
                    }
                    
                    if let Some(half_open_calls) = state.get("half_open_calls").and_then(|c| c.as_u64()) {
                        internal_state.half_open_calls = half_open_calls as u32;
                        debug!("Loaded half_open_calls: {}", internal_state.half_open_calls);
                    } else {
                        debug!("No 'half_open_calls' field found in persisted state");
                    }
                    
                    info!("Loaded circuit breaker state for {}: {:?}, failures: {}, half_open_calls: {}", 
                          self.id, internal_state.state, internal_state.failure_count, internal_state.half_open_calls);
                } else {
                    debug!("No persisted state found for circuit breaker {}, using defaults", self.id);
                }
                Ok(())
            },
            Err(e) => {
                error!("Error loading circuit breaker state for {}: {}", self.id, e);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::sync::Mutex as TokioMutex;
    use serde_json::Value;
    use std::time::Duration as StdDuration;

    // Simple state store for testing
    struct MockStateStore {
        state: Arc<TokioMutex<HashMap<String, Value>>>,
    }

    impl MockStateStore {
        fn new() -> Self {
            Self {
                state: Arc::new(TokioMutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl StateStore for MockStateStore {
        async fn get_flow_instance_state(&self, _flow_id: &str, _instance_id: &str) -> Result<Option<Value>, CoreError> {
            Ok(None)
        }

        async fn set_flow_instance_state(&self, _flow_id: &str, _instance_id: &str, _state: Value) -> Result<(), CoreError> {
            Ok(())
        }

        async fn get_component_state(&self, component_id: &str) -> Result<Option<Value>, CoreError> {
            let state = self.state.lock().await;
            Ok(state.get(component_id).cloned())
        }

        async fn set_component_state(&self, component_id: &str, state: Value) -> Result<(), CoreError> {
            let mut lock = self.state.lock().await;
            lock.insert(component_id.to_string(), state);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_circuit_breaker_basic_operation() {
        // Create a simple test with immediate state for testing
        struct DirectStateStore;

        #[async_trait::async_trait]
        impl StateStore for DirectStateStore {
            async fn get_flow_instance_state(&self, _flow_id: &str, _instance_id: &str) -> Result<Option<Value>, CoreError> {
                Ok(None)
            }

            async fn set_flow_instance_state(&self, _flow_id: &str, _instance_id: &str, _state: Value) -> Result<(), CoreError> {
                Ok(())
            }

            async fn get_component_state(&self, _component_id: &str) -> Result<Option<Value>, CoreError> {
                // Always return empty state for testing
                Ok(None)
            }

            async fn set_component_state(&self, _component_id: &str, _state: Value) -> Result<(), CoreError> {
                // Just succeed for testing
                Ok(())
            }
        }

        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            reset_timeout_ms: 100,
            half_open_max_calls: 1,
        };

        let state_store = Arc::new(DirectStateStore);
        let circuit_breaker = CircuitBreaker::new("test".to_string(), config, state_store);

        // Give time for async operations
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Should be closed initially
        assert_eq!(circuit_breaker.get_state().await.unwrap(), CircuitBreakerState::Closed);

        // First failure shouldn't trip the circuit
        let result = circuit_breaker
            .execute(async { Err::<(), _>(CoreError::ComponentError("test".to_string())) })
            .await;
        assert!(result.is_err());
        
        // Wait a bit to allow state to update
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(circuit_breaker.get_state().await.unwrap(), CircuitBreakerState::Closed);

        // Second failure should trip the circuit
        let result = circuit_breaker
            .execute(async { Err::<(), _>(CoreError::ComponentError("test".to_string())) })
            .await;
        assert!(result.is_err());
        
        // Wait a bit to allow state to update
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(circuit_breaker.get_state().await.unwrap(), CircuitBreakerState::Open);
    }
} 
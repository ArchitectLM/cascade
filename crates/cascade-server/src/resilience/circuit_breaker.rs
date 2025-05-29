//!
//! Circuit breaker pattern implementation
//! Prevents calling a failing service repeatedly, allowing it time to recover
//!

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::sync::Mutex;
use std::future::Future;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::warn;
use std::fmt;

use crate::error::{ServerError, ServerResult};
use crate::shared_state::SharedStateService;
use super::current_time_ms;

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum CircuitStatus {
    /// Circuit is closed (normal operation)
    #[default]
    Closed,
    /// Circuit is open (failing)
    Open,
    /// Circuit is half-open (testing if it can return to normal)
    HalfOpen,
}

impl std::fmt::Display for CircuitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitStatus::Closed => write!(f, "CLOSED"),
            CircuitStatus::Open => write!(f, "OPEN"),
            CircuitStatus::HalfOpen => write!(f, "HALF_OPEN"),
        }
    }
}

/// Circuit breaker key for identifying different circuits
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CircuitBreakerKey {
    /// Service or component being protected
    pub service: String,
    
    /// Optional operation within the service
    pub operation: Option<String>,
}

impl fmt::Display for CircuitBreakerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.operation {
            Some(op) => write!(f, "{}:{}", self.service, op),
            None => write!(f, "{}", self.service),
        }
    }
}

impl CircuitBreakerKey {
    /// Create a new circuit breaker key for service/operation
    pub fn new(service: impl Into<String>, operation: Option<impl Into<String>>) -> Self {
        Self {
            service: service.into(),
            operation: operation.map(|op| op.into()),
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    
    /// Time to wait before allowing a test request (in milliseconds)
    pub reset_timeout_ms: u64,
    
    /// Optional shared state scope for distributed circuit breaking
    pub shared_state_scope: Option<String>,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_timeout_ms: 30_000, // 30 seconds
            shared_state_scope: None,
        }
    }
}

/// Local circuit state
struct LocalCircuit {
    /// Current state of the circuit
    state: CircuitStatus,
    
    /// Number of consecutive failures
    failures: u32,
    
    /// Timestamp of the last failure
    last_failure_time: Instant,
}

/// Circuit breaker for protecting against cascading failures
pub struct CircuitBreaker {
    /// Configuration for the circuit breaker
    config: CircuitBreakerConfig,
    
    /// Local circuits for circuit breaking
    local_circuits: Mutex<HashMap<CircuitBreakerKey, LocalCircuit>>,
    
    /// Shared state service for distributed circuit breaking
    shared_state: Option<Arc<dyn SharedStateService>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: CircuitBreakerConfig, shared_state: Option<Arc<dyn SharedStateService>>) -> Self {
        Self {
            config,
            local_circuits: Mutex::new(HashMap::new()),
            shared_state,
        }
    }
    
    /// Check if a request is allowed by the circuit breaker
    /// Returns the current state if the request is allowed
    /// Returns an error if the circuit is open
    pub async fn allow(&self, key: &CircuitBreakerKey) -> ServerResult<CircuitStatus> {
        // If we have shared state and a scope, use distributed circuit breaking
        if let (Some(service), Some(scope)) = (&self.shared_state, &self.config.shared_state_scope) {
            self.allow_distributed(service, scope, key).await
        } else {
            // Otherwise use local circuit breaking
            self.allow_local(key).await
        }
    }
    
    /// Check if allowed using local circuit breaking
    async fn allow_local(&self, key: &CircuitBreakerKey) -> ServerResult<CircuitStatus> {
        let mut circuits = self.local_circuits.lock().await;
        
        let now = Instant::now();
        
        // Get or create circuit
        let circuit = circuits.entry(key.clone()).or_insert_with(|| LocalCircuit {
            state: CircuitStatus::Closed,
            failures: 0,
            last_failure_time: now,
        });
        
        match circuit.state {
            CircuitStatus::Closed => {
                // Circuit is closed, allow the request
                Ok(CircuitStatus::Closed)
            },
            CircuitStatus::Open => {
                // Check if the reset timeout has elapsed
                let reset_duration = Duration::from_millis(self.config.reset_timeout_ms);
                
                if now.duration_since(circuit.last_failure_time) >= reset_duration {
                    // Try half-open state
                    circuit.state = CircuitStatus::HalfOpen;
                    Ok(CircuitStatus::HalfOpen)
                } else {
                    // Circuit is still open, block the request
                    Err(ServerError::CircuitBreakerOpen {
                        service: key.service.clone(),
                        operation: key.operation.clone().unwrap_or_default(),
                        failures: circuit.failures,
                        timeout_ms: self.config.reset_timeout_ms,
                    })
                }
            },
            CircuitStatus::HalfOpen => {
                // Allow the test request
                Ok(CircuitStatus::HalfOpen)
            },
        }
    }
    
    /// Check if allowed using distributed circuit breaking
    async fn allow_distributed(
        &self,
        service: &Arc<dyn SharedStateService>,
        scope: &str,
        key: &CircuitBreakerKey
    ) -> ServerResult<CircuitStatus> {
        let state_key = key.to_string();
        let now = current_time_ms();
        
        // Get current circuit data from shared state
        let circuit_data = match service.get_state(scope, &state_key).await {
            Ok(Some(data)) => data,
            Ok(None) => {
                // Initialize a new circuit
                let new_circuit = json!({
                    "state": "CLOSED",
                    "failures": 0,
                    "last_failure_time": now,
                });
                
                // Store the initial state in shared state
                if let Err(e) = service.set_state(scope, &state_key, new_circuit.clone()).await {
                    warn!("Error saving initial circuit breaker data: {}", e);
                    // Fall back to local circuit breaking on error
                    return self.allow_local(key).await;
                }
                
                new_circuit
            },
            Err(e) => {
                warn!("Error fetching circuit breaker data: {}", e);
                // Fall back to local circuit breaking on error
                return self.allow_local(key).await;
            }
        };
        
        let state_str = circuit_data["state"].as_str().unwrap_or("CLOSED");
        let state = match state_str {
            "CLOSED" => CircuitStatus::Closed,
            "OPEN" => CircuitStatus::Open,
            "HALF_OPEN" => CircuitStatus::HalfOpen,
            _ => CircuitStatus::Closed, // Default to closed for invalid state
        };
        let failures = circuit_data["failures"].as_u64().unwrap_or(0) as u32;
        let last_failure_time = circuit_data["last_failure_time"].as_u64().unwrap_or(now);
        
        match state {
            CircuitStatus::Closed => {
                // Circuit is closed, allow the request
                Ok(CircuitStatus::Closed)
            },
            CircuitStatus::Open => {
                // Check if the reset timeout has elapsed
                if now - last_failure_time >= self.config.reset_timeout_ms {
                    // Try half-open state
                    let new_circuit = json!({
                        "state": "HALF_OPEN",
                        "failures": failures,
                        "last_failure_time": last_failure_time,
                    });
                    
                    if let Err(e) = service.set_state(scope, &state_key, new_circuit.clone()).await {
                        warn!("Error saving circuit breaker data: {}", e);
                        // Fall back to local circuit breaking on error
                        return self.allow_local(key).await;
                    }
                    
                    Ok(CircuitStatus::HalfOpen)
                } else {
                    // Circuit is still open, block the request
                    Err(ServerError::CircuitBreakerOpen {
                        service: key.service.clone(),
                        operation: key.operation.clone().unwrap_or_default(),
                        failures,
                        timeout_ms: self.config.reset_timeout_ms,
                    })
                }
            },
            CircuitStatus::HalfOpen => {
                // Allow the test request
                Ok(CircuitStatus::HalfOpen)
            },
        }
    }
    
    /// Report a success to the circuit breaker
    /// Resets the failure count and closes the circuit if it was half-open
    pub async fn report_success(&self, key: &CircuitBreakerKey) -> ServerResult<()> {
        // If we have shared state and a scope, use distributed circuit breaking
        if let (Some(service), Some(scope)) = (&self.shared_state, &self.config.shared_state_scope) {
            self.report_success_distributed(service, scope, key).await
        } else {
            // Otherwise use local circuit breaking
            self.report_success_local(key).await
        }
    }
    
    /// Report a success using local circuit breaking
    async fn report_success_local(&self, key: &CircuitBreakerKey) -> ServerResult<()> {
        let mut circuits = self.local_circuits.lock().await;
        
        if let Some(circuit) = circuits.get_mut(key) {
            // Reset failures on success
            circuit.failures = 0;
            
            // If the circuit was half-open, close it
            if circuit.state == CircuitStatus::HalfOpen {
                circuit.state = CircuitStatus::Closed;
            }
        }
        
        Ok(())
    }
    
    /// Report a success using distributed circuit breaking
    async fn report_success_distributed(
        &self,
        service: &Arc<dyn SharedStateService>,
        scope: &str,
        key: &CircuitBreakerKey
    ) -> ServerResult<()> {
        let state_key = key.to_string();
        
        // Get current circuit data from shared state
        let circuit_data = match service.get_state(scope, &state_key).await {
            Ok(Some(data)) => data,
            Ok(None) => {
                // If not found, nothing to do
                return Ok(());
            },
            Err(e) => {
                warn!("Error fetching circuit breaker data: {}", e);
                // Fall back to local circuit breaking on error
                return self.report_success_local(key).await;
            }
        };
        
        let state_str = circuit_data["state"].as_str().unwrap_or("CLOSED");
        let state = match state_str {
            "CLOSED" => CircuitStatus::Closed,
            "OPEN" => CircuitStatus::Open,
            "HALF_OPEN" => CircuitStatus::HalfOpen,
            _ => CircuitStatus::Closed, // Default to closed for invalid state
        };
        
        // Only update state if it was half-open
        if state == CircuitStatus::HalfOpen {
            let new_circuit = json!({
                "state": "CLOSED",
                "failures": 0,
                "last_failure_time": current_time_ms(),
            });
            
            if let Err(e) = service.set_state(scope, &state_key, new_circuit).await {
                warn!("Error saving circuit breaker data: {}", e);
                // Fall back to local circuit breaking on error
                return self.report_success_local(key).await;
            }
        }
        
        Ok(())
    }
    
    /// Report a failure to the circuit breaker
    /// Increments the failure count and potentially opens the circuit
    pub async fn report_failure(&self, key: &CircuitBreakerKey) -> ServerResult<()> {
        // If we have shared state and a scope, use distributed circuit breaking
        if let (Some(service), Some(scope)) = (&self.shared_state, &self.config.shared_state_scope) {
            self.report_failure_distributed(service, scope, key).await
        } else {
            // Otherwise use local circuit breaking
            self.report_failure_local(key).await
        }
    }
    
    /// Report a failure using local circuit breaking
    async fn report_failure_local(&self, key: &CircuitBreakerKey) -> ServerResult<()> {
        let mut circuits = self.local_circuits.lock().await;
        
        let now = Instant::now();
        
        // Get or create circuit
        let circuit = circuits.entry(key.clone()).or_insert_with(|| LocalCircuit {
            state: CircuitStatus::Closed,
            failures: 0,
            last_failure_time: now,
        });
        
        // Update failure count and time
        circuit.failures += 1;
        circuit.last_failure_time = now;
        
        // Handle half-open or closed circuit exceeding threshold
        if circuit.state == CircuitStatus::HalfOpen || 
           (circuit.state == CircuitStatus::Closed && circuit.failures >= self.config.failure_threshold) {
            circuit.state = CircuitStatus::Open;
        }
        
        Ok(())
    }
    
    /// Report a failure using distributed circuit breaking
    async fn report_failure_distributed(
        &self,
        service: &Arc<dyn SharedStateService>,
        scope: &str,
        key: &CircuitBreakerKey
    ) -> ServerResult<()> {
        let state_key = key.to_string();
        let now = current_time_ms();
        
        // Get current circuit data from shared state
        let circuit_data = match service.get_state(scope, &state_key).await {
            Ok(Some(data)) => data,
            Ok(None) => {
                // Initialize a new circuit
                json!({
                    "state": "CLOSED",
                    "failures": 1,
                    "last_failure_time": now,
                })
            },
            Err(e) => {
                warn!("Error fetching circuit breaker data: {}", e);
                // Fall back to local circuit breaking on error
                return self.report_failure_local(key).await;
            }
        };
        
        let state_str = circuit_data["state"].as_str().unwrap_or("CLOSED");
        let state = match state_str {
            "CLOSED" => CircuitStatus::Closed,
            "OPEN" => CircuitStatus::Open,
            "HALF_OPEN" => CircuitStatus::HalfOpen,
            _ => CircuitStatus::Closed, // Default to closed for invalid state
        };
        let failures = circuit_data["failures"].as_u64().unwrap_or(0) as u32 + 1;
        
        // Create updated circuit data
        let new_circuit = match state {
            // If half-open, open the circuit immediately
            CircuitStatus::HalfOpen => {
                json!({
                    "state": "OPEN",
                    "failures": failures,
                    "last_failure_time": now,
                })
            },
            // If closed and threshold reached, open the circuit
            CircuitStatus::Closed if failures >= self.config.failure_threshold => {
                json!({
                    "state": "OPEN",
                    "failures": failures,
                    "last_failure_time": now,
                })
            },
            // Otherwise, just update the failure count
            _ => {
                json!({
                    "state": state.to_string(),
                    "failures": failures,
                    "last_failure_time": now,
                })
            }
        };
        
        if let Err(e) = service.set_state(scope, &state_key, new_circuit).await {
            warn!("Error saving circuit breaker data: {}", e);
            // Fall back to local circuit breaking on error
            return self.report_failure_local(key).await;
        }
        
        Ok(())
    }
    
    /// Execute a function with circuit breaking
    /// Returns the result of the function if allowed
    /// Returns an error if the circuit is open
    pub async fn execute<F, Fut, T>(
        &self,
        key: &CircuitBreakerKey,
        operation: F
    ) -> ServerResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ServerResult<T>>,
    {
        // Check if allowed by circuit breaker
        let state = self.allow(key).await?;
        
        // Execute the operation
        match operation().await {
            Ok(result) => {
                // Report success to potentially close the circuit
                if state == CircuitStatus::HalfOpen {
                    self.report_success(key).await?;
                }
                
                Ok(result)
            },
            Err(e) => {
                // Report failure to potentially open the circuit
                self.report_failure(key).await?;
                
                Err(e)
            },
        }
    }
    
    /// Create a circuit breaker that uses the given component runtime API
    /// for shared state operations
    pub fn for_component(
        config: CircuitBreakerConfig,
        shared_state: Option<Arc<dyn SharedStateService>>
    ) -> Self {
        Self::new(config, shared_state)
    }
    
    /// Check if a circuit is in a tripped state
    pub async fn is_circuit_open(&self, circuit_name: &str) -> Result<bool, ServerError> {
        let key = CircuitBreakerKey::new(circuit_name, None::<String>);
        
        // Check if allowed - if error, circuit is open
        match self.allow(&key).await {
            Ok(_) => Ok(false),
            Err(ServerError::CircuitBreakerOpen { .. }) => Ok(true),
            Err(e) => Err(e),
        }
    }

    /// Try to call a downstream service through the circuit breaker
    pub async fn call<F, Fut, T, E>(&self, circuit_name: &str, f: F) -> Result<T, ServerError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let key = CircuitBreakerKey::new(circuit_name, None::<String>);
        
        // Execute with circuit breaking
        self.execute(&key, || async {
            match f().await {
                Ok(value) => Ok(value),
                Err(e) => Err(ServerError::InternalError(format!(
                    "Downstream error in circuit {}: {}", circuit_name, e
                ))),
            }
        }).await
    }
    
    /// Reset a circuit to closed state
    pub async fn reset_circuit(&self, circuit_name: &str) -> Result<(), ServerError> {
        let key = CircuitBreakerKey::new(circuit_name, None::<String>);
        
        if let (Some(service), Some(scope)) = (&self.shared_state, &self.config.shared_state_scope) {
            // Reset in distributed state
            let state_key = key.to_string();
            let new_circuit = json!({
                "state": "CLOSED",
                "failures": 0,
                "last_failure_time": current_time_ms(),
            });
            
            if let Err(e) = service.set_state(scope, &state_key, new_circuit).await {
                warn!("Error resetting circuit breaker: {}", e);
            }
        }
        
        // Also reset locally in case we fall back to local
        let mut circuits = self.local_circuits.lock().await;
        if let Some(circuit) = circuits.get_mut(&key) {
            circuit.state = CircuitStatus::Closed;
            circuit.failures = 0;
        }
        
        Ok(())
    }
} 
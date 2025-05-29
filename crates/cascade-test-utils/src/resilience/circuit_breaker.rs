//! Circuit breaker pattern implementation for testing
//!
//! This is a simple implementation of the circuit breaker pattern
//! that can be used in tests.

use std::sync::{Arc, Mutex};
use tokio::time::sleep;
use std::time::Duration;
use serde::{Serialize, Deserialize};
use std::fmt;

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Circuit is closed, all calls proceed normally
    Closed,
    /// Circuit is open, all calls fail fast
    Open,
    /// Circuit is in half-open state, allowing limited calls through
    HalfOpen,
}

impl fmt::Display for CircuitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "CLOSED"),
            CircuitState::Open => write!(f, "OPEN"),
            CircuitState::HalfOpen => write!(f, "HALF_OPEN"),
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures to trigger opening the circuit
    pub failure_threshold: usize,
    /// Time to keep the circuit open before transitioning to half-open (in ms)
    pub reset_timeout_ms: u64,
    /// Maximum number of calls allowed in half-open state
    pub half_open_max_calls: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            reset_timeout_ms: 1000,
            half_open_max_calls: 1,
        }
    }
}

/// Circuit breaker metrics for monitoring
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CircuitBreakerMetrics {
    /// Total number of failures recorded
    pub failure_count: usize,
    /// Total number of successful calls
    pub success_count: usize,
    /// Number of times circuit transitioned to open
    pub open_transitions: usize,
    /// Number of times circuit transitioned to half-open
    pub half_open_transitions: usize,
    /// Number of times circuit transitioned to closed after being open/half-open
    pub closed_transitions: usize,
}

/// Circuit breaker for resilience testing
#[derive(Clone)]
pub struct CircuitBreaker {
    /// Current state of the circuit
    state: Arc<Mutex<CircuitState>>,
    /// Configuration for this circuit breaker
    config: CircuitBreakerConfig,
    /// Metrics collected during operation
    metrics: Arc<Mutex<CircuitBreakerMetrics>>,
    /// Consecutive failures in the current closed state
    consecutive_failures: Arc<Mutex<usize>>,
    /// Calls allowed in half-open state
    half_open_calls_remaining: Arc<Mutex<usize>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the specified configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitState::Closed)),
            config,
            metrics: Arc::new(Mutex::new(CircuitBreakerMetrics::default())),
            consecutive_failures: Arc::new(Mutex::new(0)),
            half_open_calls_remaining: Arc::new(Mutex::new(0)),
        }
    }

    /// Create a new circuit breaker with default configuration
    pub fn new_default() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }

    /// Get the current state of the circuit
    pub fn get_state(&self) -> CircuitState {
        self.state.lock().unwrap().clone()
    }

    /// Get the current state as a string
    pub fn get_state_string(&self) -> String {
        self.get_state().to_string()
    }

    /// Get a copy of the current metrics
    pub fn get_metrics(&self) -> CircuitBreakerMetrics {
        self.metrics.lock().unwrap().clone()
    }

    /// Execute a function through the circuit breaker
    pub async fn execute<F, Fut, T, E>(&self, operation: F) -> Result<T, E> 
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: From<String>,
    {
        let state = self.get_state();

        match state {
            CircuitState::Open => {
                // Circuit is open, fail fast
                Err("CircuitBreakerOpen: Circuit breaker is open".to_string().into())
            },
            CircuitState::HalfOpen => {
                // In half-open state, allow limited calls through
                let allow_call = {
                    let mut calls = self.half_open_calls_remaining.lock().unwrap();
                    if *calls > 0 {
                        *calls -= 1;
                        true
                    } else {
                        false
                    }
                };
                
                if allow_call {
                    match operation().await {
                        Ok(result) => {
                            self.record_success();
                            Ok(result)
                        },
                        Err(e) => {
                            self.record_failure();
                            Err(e)
                        }
                    }
                } else {
                    Err("CircuitBreakerOpen: No more calls allowed in half-open state".to_string().into())
                }
            },
            CircuitState::Closed => {
                // Circuit is closed, execute normally
                match operation().await {
                    Ok(result) => {
                        self.record_success();
                        Ok(result)
                    },
                    Err(e) => {
                        self.record_failure();
                        Err(e)
                    }
                }
            }
        }
    }

    /// Reset the circuit breaker to closed state
    pub fn reset(&self) {
        *self.state.lock().unwrap() = CircuitState::Closed;
        *self.consecutive_failures.lock().unwrap() = 0;
    }

    /// Manually transition the circuit to the open state
    pub fn trip_open(&self) {
        *self.state.lock().unwrap() = CircuitState::Open;
        *self.consecutive_failures.lock().unwrap() = 0;
        self.metrics.lock().unwrap().open_transitions += 1;
    }

    // Internal method to record a successful operation
    fn record_success(&self) {
        // Update metrics
        let mut metrics = self.metrics.lock().unwrap();
        metrics.success_count += 1;
        
        // Reset consecutive failures
        *self.consecutive_failures.lock().unwrap() = 0;
        
        // If in half-open state, transition to closed
        let mut state = self.state.lock().unwrap();
        if *state == CircuitState::HalfOpen {
            *state = CircuitState::Closed;
            metrics.closed_transitions += 1;
            println!("Circuit breaker transitioning from HALF_OPEN to CLOSED after successful call");
        }
    }

    // Internal method to record a failed operation
    fn record_failure(&self) {
        // Update metrics
        let mut metrics = self.metrics.lock().unwrap();
        metrics.failure_count += 1;
        
        // Get current state
        let current_state = self.get_state();
        
        match current_state {
            CircuitState::Closed => {
                // Increment consecutive failures
                let mut failures = self.consecutive_failures.lock().unwrap();
                *failures += 1;
                
                // Check if we should transition to open
                if *failures >= self.config.failure_threshold {
                    // Transition to open
                    let mut state = self.state.lock().unwrap();
                    *state = CircuitState::Open;
                    metrics.open_transitions += 1;
                    println!("Circuit breaker transitioning from CLOSED to OPEN after {} consecutive failures", *failures);
                    
                    // Schedule transition to half-open after timeout
                    let state_clone = self.state.clone();
                    let metrics_clone = self.metrics.clone();
                    let half_open_calls = self.half_open_calls_remaining.clone();
                    let max_calls = self.config.half_open_max_calls;
                    let timeout_ms = self.config.reset_timeout_ms;
                    
                    tokio::spawn(async move {
                        // Wait for the reset timeout
                        sleep(Duration::from_millis(timeout_ms)).await;
                        
                        // Transition to half-open if still open
                        let mut state = state_clone.lock().unwrap();
                        if *state == CircuitState::Open {
                            *state = CircuitState::HalfOpen;
                            let mut metrics = metrics_clone.lock().unwrap();
                            metrics.half_open_transitions += 1;
                            
                            // Reset the half-open calls counter
                            *half_open_calls.lock().unwrap() = max_calls;
                            
                            println!("Circuit breaker transitioning from OPEN to HALF_OPEN after timeout");
                        }
                    });
                }
            },
            CircuitState::HalfOpen => {
                // If failure in half-open, transition back to open
                let mut state = self.state.lock().unwrap();
                *state = CircuitState::Open;
                metrics.open_transitions += 1;
                println!("Circuit breaker transitioning from HALF_OPEN to OPEN after failed call");
                
                // Schedule transition to half-open after timeout
                let state_clone = self.state.clone();
                let metrics_clone = self.metrics.clone();
                let half_open_calls = self.half_open_calls_remaining.clone();
                let max_calls = self.config.half_open_max_calls;
                let timeout_ms = self.config.reset_timeout_ms;
                
                tokio::spawn(async move {
                    // Wait for the reset timeout
                    sleep(Duration::from_millis(timeout_ms)).await;
                    
                    // Transition to half-open if still open
                    let mut state = state_clone.lock().unwrap();
                    if *state == CircuitState::Open {
                        *state = CircuitState::HalfOpen;
                        let mut metrics = metrics_clone.lock().unwrap();
                        metrics.half_open_transitions += 1;
                        
                        // Reset the half-open calls counter
                        *half_open_calls.lock().unwrap() = max_calls;
                        
                        println!("Circuit breaker transitioning from OPEN to HALF_OPEN after timeout");
                    }
                });
            },
            CircuitState::Open => {
                // Already open, nothing to do
            }
        }
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }
} 
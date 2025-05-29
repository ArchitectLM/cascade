use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use cascade_server::error::ServerError;
use cascade_server::shared_state::InMemorySharedStateService;
use cascade_server::resilience::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerKey};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::future::Future;
use anyhow::Result;

// Implement our own test tracing initialization
fn init_test_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("cascade_server=debug".parse().unwrap())
            .add_directive("test=debug".parse().unwrap()))
        .with_test_writer()
        .finish();
        
    let _ = tracing::subscriber::set_global_default(subscriber);
}

// Define our own test circuit breaker config
#[derive(Debug, Clone)]
pub struct TestCircuitBreakerConfig {
    pub failure_threshold: usize,
    pub reset_timeout: Duration,
    pub success_threshold: usize,
}

// Our own implementation of test circuit breaker
#[derive(Debug, Clone)]
pub struct TestCircuitBreaker {
    name: String,
    config: TestCircuitBreakerConfig,
    failures: Arc<AtomicUsize>,
    successes: Arc<AtomicUsize>,
    last_failure: Arc<std::sync::Mutex<Option<std::time::Instant>>>,
}

// Enum to track circuit state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl TestCircuitBreaker {
    pub fn new(name: &str, config: TestCircuitBreakerConfig) -> Self {
        TestCircuitBreaker {
            name: name.to_string(),
            config,
            failures: Arc::new(AtomicUsize::new(0)),
            successes: Arc::new(AtomicUsize::new(0)),
            last_failure: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    // Execute a function with circuit breaking
    pub async fn execute<F, Fut, T, E>(&self, operation: F) -> Result<T, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: From<anyhow::Error>,
    {
        // Check if circuit is open
        let state = self.get_state();
        
        match state {
            CircuitState::Open => {
                // Circuit is open, check if we should move to half-open
                let last_failure = self.last_failure.lock().unwrap();
                if let Some(time) = *last_failure {
                    if time.elapsed() > self.config.reset_timeout {
                        // Reset timeout has elapsed, allow one test request
                        drop(last_failure);
                        return self.try_execute(operation, CircuitState::HalfOpen).await;
                    }
                }
                
                // Circuit is still open, fail fast
                return Err(anyhow::anyhow!("Circuit breaker is open for {}", self.name).into());
            },
            CircuitState::HalfOpen => {
                // Allow one request to try the service
                return self.try_execute(operation, CircuitState::HalfOpen).await;
            },
            CircuitState::Closed => {
                // Normal operation
                return self.try_execute(operation, CircuitState::Closed).await;
            }
        }
    }
    
    // Try to execute the operation and update circuit state
    async fn try_execute<F, Fut, T, E>(&self, operation: F, state: CircuitState) -> Result<T, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: From<anyhow::Error>,
    {
        match operation().await {
            Ok(value) => {
                // Success, record it and possibly close circuit
                if state == CircuitState::HalfOpen {
                    self.successes.fetch_add(1, Ordering::SeqCst);
                    let successes = self.successes.load(Ordering::SeqCst);
                    
                    if successes >= self.config.success_threshold {
                        // Reset circuit state
                        self.failures.store(0, Ordering::SeqCst);
                        self.successes.store(0, Ordering::SeqCst);
                        *self.last_failure.lock().unwrap() = None;
                    }
                }
                
                Ok(value)
            },
            Err(err) => {
                // Failure, record it and possibly open circuit
                let failures = self.failures.fetch_add(1, Ordering::SeqCst) + 1;
                
                // Reset success counter in half-open state
                if state == CircuitState::HalfOpen {
                    self.successes.store(0, Ordering::SeqCst);
                }
                
                // Update last failure time
                *self.last_failure.lock().unwrap() = Some(std::time::Instant::now());
                
                // Check if we should open the circuit
                if failures >= self.config.failure_threshold {
                    // Circuit should be open now
                    if state == CircuitState::Closed {
                        println!("Opening circuit for {}", self.name);
                    }
                }
                
                Err(err)
            }
        }
    }
    
    // Get current circuit state
    fn get_state(&self) -> CircuitState {
        let failures = self.failures.load(Ordering::SeqCst);
        
        if failures >= self.config.failure_threshold {
            // Might be open or half-open
            let last_failure = self.last_failure.lock().unwrap();
            
            if let Some(time) = *last_failure {
                if time.elapsed() > self.config.reset_timeout {
                    // Reset timeout elapsed, circuit should be half-open
                    return CircuitState::HalfOpen;
                }
            }
            
            // Circuit is open
            return CircuitState::Open;
        }
        
        // Circuit is closed
        CircuitState::Closed
    }
}

// Inlined helper function 
fn create_shared_state() -> Arc<InMemorySharedStateService> {
    Arc::new(InMemorySharedStateService::new())
}

/// Test circuit breaker with local state
#[tokio::test]
async fn test_circuit_breaker_local_state() {
    init_test_tracing();
    
    // Create circuit breaker with local state
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_ms: 500,
        shared_state_scope: None,
    };
    
    let circuit_breaker = CircuitBreaker::new(config, None);
    let key = CircuitBreakerKey::new("test-service", None::<String>);
    
    // First execution - should try the operation and fail
    let result = circuit_breaker.execute(&key, || async {
        Err::<(), ServerError>(ServerError::RuntimeError("Test error".to_string()))
    }).await;
    
    assert!(result.is_err());
    
    // Second execution - should try again and fail
    let result = circuit_breaker.execute(&key, || async {
        Err::<(), ServerError>(ServerError::RuntimeError("Test error".to_string()))
    }).await;
    
    assert!(result.is_err());
    
    // Third execution - should try again and fail, opening the circuit
    let result = circuit_breaker.execute(&key, || async {
        Err::<(), ServerError>(ServerError::RuntimeError("Test error".to_string()))
    }).await;
    
    assert!(result.is_err());
    
    // Fourth execution - circuit should now be open (failing fast)
    let result = circuit_breaker.execute(&key, || async {
        // This should not be called because the circuit is open
        panic!("This should not be called if the circuit is open");
        #[allow(unreachable_code)]
        Ok::<(), ServerError>(())
    }).await;
    
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, ServerError::CircuitBreakerOpen { .. }));
    assert!(err.to_string().contains("Circuit breaker open for"));
}

/// Test circuit breaker with shared state
#[tokio::test]
async fn test_circuit_breaker_shared_state() {
    init_test_tracing();
    
    // Create shared state
    let shared_state = create_shared_state();
    
    // Create circuit breaker with shared state
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        reset_timeout_ms: 500,
        shared_state_scope: Some("test-scope".to_string()),
    };
    
    let circuit_breaker = CircuitBreaker::new(config, Some(shared_state));
    let key = CircuitBreakerKey::new("test-service", None::<String>);
    
    // First execution - should try the operation and fail
    let result = circuit_breaker.execute(&key, || async {
        Err::<(), ServerError>(ServerError::RuntimeError("Test error".to_string()))
    }).await;
    
    assert!(result.is_err());
    
    // Second execution - should try again and fail, opening the circuit
    let result = circuit_breaker.execute(&key, || async {
        Err::<(), ServerError>(ServerError::RuntimeError("Test error".to_string()))
    }).await;
    
    assert!(result.is_err());
    
    // Third execution - circuit should now be open (failing fast)
    let result = circuit_breaker.execute(&key, || async {
        // This should not be called because the circuit is open
        panic!("This should not be called if the circuit is open");
        #[allow(unreachable_code)]
        Ok::<(), ServerError>(())
    }).await;
    
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, ServerError::CircuitBreakerOpen { .. }));
    assert!(err.to_string().contains("Circuit breaker open for"));
}

/// Test circuit breaker with half-open state recovery
#[tokio::test]
async fn test_circuit_breaker_recovery() {
    init_test_tracing();
    
    // Create circuit breaker with local state and short reset timeout
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        reset_timeout_ms: 100, // Very short timeout for testing
        shared_state_scope: None,
    };
    
    let circuit_breaker = CircuitBreaker::new(config, None);
    let key = CircuitBreakerKey::new("test-service", None::<String>);
    
    // First execution - should try the operation and fail
    let result = circuit_breaker.execute(&key, || async {
        Err::<(), ServerError>(ServerError::RuntimeError("Test error".to_string()))
    }).await;
    
    assert!(result.is_err());
    
    // Second execution - should try again and fail, opening the circuit
    let result = circuit_breaker.execute(&key, || async {
        Err::<(), ServerError>(ServerError::RuntimeError("Test error".to_string()))
    }).await;
    
    assert!(result.is_err());
    
    // Third execution - circuit should now be open (failing fast)
    let result = circuit_breaker.execute(&key, || async {
        // This should not be called because the circuit is open
        panic!("This should not be called if the circuit is open");
        #[allow(unreachable_code)]
        Ok::<(), ServerError>(())
    }).await;
    
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, ServerError::CircuitBreakerOpen { .. }));
    assert!(err.to_string().contains("Circuit breaker open for"));
    
    // Wait for reset timeout to expire
    time::sleep(Duration::from_millis(150)).await;
    
    // Circuit should now be half-open, allowing one test request
    let result = circuit_breaker.execute(&key, || async {
        // This will be called because the circuit is half-open
        Ok(())
    }).await;
    
    assert!(result.is_ok());
    
    // Next request should succeed because the circuit is now closed
    let result = circuit_breaker.execute(&key, || async {
        Ok(())
    }).await;
    
    assert!(result.is_ok());
}

/// Test with TestCircuitBreaker implementation
#[tokio::test]
async fn test_with_test_circuit_breaker() {
    init_test_tracing();
    
    // Create a test circuit breaker with failure threshold of 2
    let config = TestCircuitBreakerConfig {
        failure_threshold: 2,
        reset_timeout: Duration::from_millis(100),
        success_threshold: 1,
    };
    let circuit_breaker = TestCircuitBreaker::new("test", config);
    
    // First test should succeed
    let result = circuit_breaker.execute(|| async {
        Ok::<String, anyhow::Error>("success".to_string())
    }).await;
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success");
    
    // Now trigger a failure
    let result: Result<String, anyhow::Error> = circuit_breaker.execute(|| async {
        Err(anyhow::anyhow!("test failure"))
    }).await;
    
    assert!(result.is_err());
    
    // Second failure should open the circuit
    let result: Result<String, anyhow::Error> = circuit_breaker.execute(|| async {
        Err(anyhow::anyhow!("test failure"))
    }).await;
    
    assert!(result.is_err());
    
    // Third attempt should be rejected by the circuit breaker
    let result: Result<String, anyhow::Error> = circuit_breaker.execute(|| async {
        // This should not execute
        panic!("This should not be called when circuit is open");
        #[allow(unreachable_code)]
        Ok::<String, anyhow::Error>("should not succeed".to_string())
    }).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Circuit breaker is open"));
    
    // Wait for reset timeout
    time::sleep(Duration::from_millis(150)).await;
    
    // Circuit should now be half-open, allowing one test request
    let result = circuit_breaker.execute(|| async {
        // This should execute because we're in half-open state
        Ok::<String, anyhow::Error>("success after recovery".to_string())
    }).await;
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success after recovery");
    
    // Circuit should now be closed again, allowing normal operations
    let result = circuit_breaker.execute(|| async {
        Ok::<String, anyhow::Error>("normal operation".to_string())
    }).await;
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "normal operation");
} 
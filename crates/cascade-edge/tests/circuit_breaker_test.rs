use cascade_core::CoreError;
use cascade_edge::resilience::{
    circuit_breaker::{CircuitBreaker, CircuitBreakerState, CircuitBreakerConfig},
    bulkhead::{Bulkhead, BulkheadConfig},
    StateStore,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration, timeout};
use serde_json::{json, Value};
use std::collections::HashMap;
use async_trait::async_trait;
use tokio::sync::Semaphore;
use std::cmp;

// Mocks and helper functions
struct MockStateStore {
    state: Arc<Mutex<HashMap<String, Value>>>
}

impl MockStateStore {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new()))
        }
    }
}

#[async_trait]
impl StateStore for MockStateStore {
    async fn get_flow_instance_state(&self, flow_id: &str, instance_id: &str) -> Result<Option<Value>, CoreError> {
        let key = format!("{}:{}", flow_id, instance_id);
        let state = self.state.lock().await;
        Ok(state.get(&key).cloned())
    }

    async fn set_flow_instance_state(&self, flow_id: &str, instance_id: &str, state: Value) -> Result<(), CoreError> {
        let key = format!("{}:{}", flow_id, instance_id);
        self.state.lock().await.insert(key, state);
        Ok(())
    }

    async fn get_component_state(&self, component_id: &str) -> Result<Option<Value>, CoreError> {
        let state = self.state.lock().await;
        Ok(state.get(component_id).cloned())
    }

    async fn set_component_state(&self, component_id: &str, state: Value) -> Result<(), CoreError> {
        self.state.lock().await.insert(component_id.to_string(), state);
        Ok(())
    }
}

#[tokio::test]
async fn test_circuit_breaker_initial_state() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_ms: 100,
        half_open_max_calls: 1
    };
    
    let state_store = Arc::new(MockStateStore::new());
    let circuit_id = "test_initial_state";
    
    // Create circuit breaker
    let circuit_breaker = CircuitBreaker::new(
        circuit_id.to_string(),
        config,
        state_store.clone()
    );
    
    // Wait for initial state to be loaded
    sleep(Duration::from_millis(50)).await;
    
    // Initially circuit should be closed
    let initial_state = circuit_breaker.get_state().await.unwrap();
    assert_eq!(initial_state, CircuitBreakerState::Closed, 
        "Circuit should start in closed state");
}

#[tokio::test]
async fn test_circuit_breaker_failure_threshold() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_ms: 100,
        half_open_max_calls: 1
    };
    
    let state_store = Arc::new(MockStateStore::new());
    let circuit_id = "test_failure_threshold";
    
    // Create circuit breaker
    let circuit_breaker = CircuitBreaker::new(
        circuit_id.to_string(),
        config,
        state_store.clone()
    );
    
    // Wait for initial state to be loaded
    sleep(Duration::from_millis(50)).await;
    
    // Circuit should be closed and allow failures up to threshold
    for i in 1..=2 {
        let result = circuit_breaker.execute::<_, Value, CoreError>(async {
            Err(CoreError::ComponentError("Test failure".to_string()))
        }).await;
        
        assert!(result.is_err(), "Call should fail but circuit remains closed");
        let state = circuit_breaker.get_state().await.unwrap();
        assert_eq!(state, CircuitBreakerState::Closed, 
            "Circuit should remain closed after {} failures", i);
    }
    
    // Next failure should trip the circuit to open
    let result = circuit_breaker.execute::<_, Value, CoreError>(async {
        Err(CoreError::ComponentError("Test failure".to_string()))
    }).await;
    
    assert!(result.is_err(), "Call should fail and trip circuit");
    let state = circuit_breaker.get_state().await.unwrap();
    assert_eq!(state, CircuitBreakerState::Open, 
        "Circuit should open after reaching failure threshold");
}

#[tokio::test]
async fn test_circuit_breaker_open_rejects_calls() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_ms: 100,
        half_open_max_calls: 1
    };
    
    let state_store = Arc::new(MockStateStore::new());
    let circuit_id = "test_open_rejects";
    
    // Create circuit breaker
    let circuit_breaker = CircuitBreaker::new(
        circuit_id.to_string(),
        config,
        state_store.clone()
    );
    
    // Wait for initial state to be loaded
    sleep(Duration::from_millis(50)).await;
    
    // Trip the circuit
    for _ in 0..3 {
        let _ = circuit_breaker.execute::<_, Value, CoreError>(async {
            Err(CoreError::ComponentError("Test failure".to_string()))
        }).await;
    }
    
    // Verify circuit is open
    let state = circuit_breaker.get_state().await.unwrap();
    assert_eq!(state, CircuitBreakerState::Open, "Circuit should be open");
    
    // Circuit is now open, calls should fail fast without executing
    let start = std::time::Instant::now();
    let result = circuit_breaker.execute::<_, Value, CoreError>(async {
        sleep(Duration::from_millis(100)).await;
        Ok(json!({"result": "success"}))
    }).await;
    
    assert!(result.is_err(), "Circuit should be open and fail fast");
    assert!(start.elapsed() < Duration::from_millis(50), 
        "Open circuit should fail fast without executing");
}

#[tokio::test]
async fn test_circuit_breaker_half_open_transition() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_ms: 100, // 100ms for faster test
        half_open_max_calls: 1
    };
    
    let state_store = Arc::new(MockStateStore::new());
    let circuit_id = "test_half_open";
    
    // Create circuit breaker
    let circuit_breaker = CircuitBreaker::new(
        circuit_id.to_string(),
        config,
        state_store.clone()
    );
    
    // Wait for initial state to be loaded
    sleep(Duration::from_millis(50)).await;
    
    // Trip the circuit
    for _ in 0..3 {
        let _ = circuit_breaker.execute::<_, Value, CoreError>(async {
            Err(CoreError::ComponentError("Test failure".to_string()))
        }).await;
    }
    
    // Verify circuit is open
    let state = circuit_breaker.get_state().await.unwrap();
    assert_eq!(state, CircuitBreakerState::Open, "Circuit should be open");
    
    // Wait for reset timeout plus some buffer
    sleep(Duration::from_millis(150)).await;
    
    // Check if state transitioned to half-open
    let state_after_wait = circuit_breaker.get_state().await.unwrap();
    assert_eq!(state_after_wait, CircuitBreakerState::HalfOpen,
        "Circuit should transition to half-open after reset timeout");
}

#[tokio::test]
async fn test_circuit_breaker_half_open_success() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_ms: 100, // 100ms for faster test
        half_open_max_calls: 1
    };
    
    let state_store = Arc::new(MockStateStore::new());
    let circuit_id = "test_half_open_success";
    
    // Create circuit breaker
    let circuit_breaker = CircuitBreaker::new(
        circuit_id.to_string(),
        config,
        state_store.clone()
    );
    
    // Wait for initial state to be loaded
    sleep(Duration::from_millis(50)).await;
    
    // Trip the circuit
    for _ in 0..3 {
        let _ = circuit_breaker.execute::<_, Value, CoreError>(async {
            Err(CoreError::ComponentError("Test failure".to_string()))
        }).await;
    }
    
    // Wait for reset timeout plus some buffer
    sleep(Duration::from_millis(150)).await;
    
    // Check if state transitioned to half-open
    let state_after_wait = circuit_breaker.get_state().await.unwrap();
    assert_eq!(state_after_wait, CircuitBreakerState::HalfOpen,
        "Circuit should transition to half-open after reset timeout");
    
    // A successful call in half-open state should close the circuit
    let result = circuit_breaker.execute::<_, Value, CoreError>(async {
        Ok(json!({"result": "success"}))
    }).await;
    
    assert!(result.is_ok(), "Half-open circuit should allow the call");
    
    // Wait a bit to allow state to update
    sleep(Duration::from_millis(50)).await;
    
    // Check final state
    let final_state = circuit_breaker.get_state().await.unwrap();
    assert_eq!(final_state, CircuitBreakerState::Closed,
        "Circuit should be closed after successful call in half-open state");
}

#[tokio::test]
async fn test_circuit_breaker_half_open_failure() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_ms: 100, // 100ms for faster test
        half_open_max_calls: 1
    };
    
    let state_store = Arc::new(MockStateStore::new());
    let circuit_id = "test_half_open_failure";
    
    // Create circuit breaker
    let circuit_breaker = CircuitBreaker::new(
        circuit_id.to_string(),
        config,
        state_store.clone()
    );
    
    // Wait for initial state to be loaded
    sleep(Duration::from_millis(50)).await;
    
    // Trip the circuit
    for _ in 0..3 {
        let _ = circuit_breaker.execute::<_, Value, CoreError>(async {
            Err(CoreError::ComponentError("Test failure".to_string()))
        }).await;
    }
    
    // Wait for reset timeout plus some buffer
    sleep(Duration::from_millis(150)).await;
    
    // Check if state transitioned to half-open
    let state_after_wait = circuit_breaker.get_state().await.unwrap();
    assert_eq!(state_after_wait, CircuitBreakerState::HalfOpen,
        "Circuit should transition to half-open after reset timeout");
    
    // A failure in half-open state should open the circuit again
    let result = circuit_breaker.execute::<_, Value, CoreError>(async {
        Err(CoreError::ComponentError("Test failure".to_string()))
    }).await;
    
    assert!(result.is_err(), "Call should fail in half-open state");
    
    // Wait a bit to allow state to update
    sleep(Duration::from_millis(50)).await;
    
    // Check final state
    let final_state = circuit_breaker.get_state().await.unwrap();
    assert_eq!(final_state, CircuitBreakerState::Open,
        "Circuit should be opened again after failed call in half-open state");
}

#[tokio::test]
async fn test_circuit_breaker_state_persistence() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_ms: 100, // 100ms for faster test
        half_open_max_calls: 1
    };
    
    let state_store = Arc::new(MockStateStore::new());
    let circuit_id = "test_state_persistence";
    
    // Create circuit breaker
    let circuit_breaker = CircuitBreaker::new(
        circuit_id.to_string(),
        config,
        state_store.clone()
    );
    
    // Wait for initial state to be loaded
    sleep(Duration::from_millis(50)).await;
    
    // Trip the circuit
    for _ in 0..3 {
        let _ = circuit_breaker.execute::<_, Value, CoreError>(async {
            Err(CoreError::ComponentError("Test failure".to_string()))
        }).await;
    }
    
    // Verify circuit is open and state is persisted
    let state = circuit_breaker.get_state().await.unwrap();
    assert_eq!(state, CircuitBreakerState::Open, "Circuit should be open");
    
    // Check persisted state
    let persisted_state = state_store.get_component_state(circuit_id).await.unwrap().unwrap();
    assert!(persisted_state["state"].is_string(), "State should be persisted");
    assert_eq!(persisted_state["state"].as_str().unwrap(), "OPEN",
        "Persisted state should be OPEN");
    
    // Wait for reset timeout plus some buffer
    sleep(Duration::from_millis(150)).await;
    
    // A successful call in half-open state should close the circuit
    let _ = circuit_breaker.execute::<_, Value, CoreError>(async {
        Ok(json!({"result": "success"}))
    }).await;
    
    // Wait a bit to allow state to update
    sleep(Duration::from_millis(50)).await;
    
    // Verify updated persisted state
    let updated_state = state_store.get_component_state(circuit_id).await.unwrap().unwrap();
    assert!(updated_state["state"].is_string(), "State should be persisted");
    assert_eq!(updated_state["state"].as_str().unwrap(), "CLOSED",
        "Persisted state should be CLOSED after successful call in half-open state");
}

#[tokio::test]
async fn test_bulkhead_pattern_with_timeout() {
    // Configure bulkhead
    let config = BulkheadConfig {
        max_concurrent_calls: 3,
        max_queue_size: 2
    };
    
    // Create bulkhead
    let bulkhead = Bulkhead::new("test_tier".to_string(), config.clone());
    
    // Test concurrency with controlled completion
    let permits = Arc::new(Semaphore::new(0));
    let completion_counter = Arc::new(tokio::sync::Mutex::new(0));
    
    // Launch max_concurrent calls with timeout protection
    let mut handles = vec![];
    for i in 0..config.max_concurrent_calls {
        let bulkhead = bulkhead.clone();
        let permits_clone = permits.clone();
        let counter = completion_counter.clone();
        
        let handle = tokio::spawn(async move {
            // Add timeout to prevent hanging
            let result = timeout(Duration::from_secs(5), bulkhead.execute::<_, Value, CoreError>(async move {
                // Wait for test to release the permit (with timeout)
                if let Ok(permit) = timeout(Duration::from_secs(1), permits_clone.acquire()).await {
                    let _permit = permit.unwrap();
                    // Increment counter to track completed calls
                    let mut count = counter.lock().await;
                    *count += 1;
                    Ok(json!({"call_id": i}))
                } else {
                    // Timeout occurred waiting for permit
                    Ok(json!({"call_id": i, "timeout": true}))
                }
            })).await;
            
            match result {
                Ok(inner_result) => inner_result,
                Err(_) => Err(CoreError::ExternalDependencyError("Test timed out".into())),
            }
        });
        
        handles.push(handle);
    }
    
    // Short delay to ensure tasks have started
    sleep(Duration::from_millis(100)).await;
    
    // Check stats
    let stats = bulkhead.get_stats().await;
    assert_eq!(stats.active_count, config.max_concurrent_calls, 
        "All concurrent slots should be filled");
    
    // Also queue additional calls (but with timeout protection)
    for i in 0..config.max_queue_size {
        let bulkhead = bulkhead.clone();
        let permits_clone = permits.clone();
        let counter = completion_counter.clone();
        
        let handle = tokio::spawn(async move {
            // Add timeout to prevent hanging
            let result = timeout(Duration::from_secs(5), bulkhead.execute::<_, Value, CoreError>(async move {
                // Wait for test to release the permit (with timeout)
                if let Ok(permit) = timeout(Duration::from_secs(1), permits_clone.acquire()).await {
                    let _permit = permit.unwrap();
                    // Increment counter to track completed calls
                    let mut count = counter.lock().await;
                    *count += 1;
                    Ok(json!({"queued_call_id": i}))
                } else {
                    // Timeout occurred waiting for permit
                    Ok(json!({"queued_call_id": i, "timeout": true}))
                }
            })).await;
            
            match result {
                Ok(inner_result) => inner_result,
                Err(_) => Err(CoreError::ExternalDependencyError("Test timed out".into())),
            }
        });
        
        handles.push(handle);
    }
    
    // Short delay to ensure queued tasks have started
    sleep(Duration::from_millis(100)).await;
    
    // One more call should be rejected (bulkhead full)
    let bulkhead = bulkhead.clone();
    let result = timeout(Duration::from_secs(1), bulkhead.execute::<_, Value, CoreError>(async {
        Ok(json!({"should_not_execute": true}))
    })).await;
    
    assert!(result.is_ok(), "Bulkhead execution should not timeout");
    let inner_result = result.unwrap();
    assert!(inner_result.is_err(), "Bulkhead should reject call when full");
    let err = inner_result.unwrap_err();
    assert!(format!("{:?}", err).contains("Bulkhead"), 
        "Error should indicate bulkhead rejection: {:?}", err);
    
    // Release permits to allow tasks to complete (only release what we need)
    let total_permits = cmp::min(
        (config.max_concurrent_calls + config.max_queue_size) as usize, 
        handles.len()
    );
    permits.add_permits(total_permits);
    
    // Wait for all tasks with timeout protection
    let join_result = timeout(Duration::from_secs(5), futures::future::join_all(handles)).await;
    assert!(join_result.is_ok(), "Task joining should not timeout");
    
    // Verify stats after completion
    sleep(Duration::from_millis(100)).await; // Wait for stats to update
    let final_stats = bulkhead.get_stats().await;
    assert_eq!(final_stats.active_count, 0, "All calls should have completed");
    
    // Verify completed count
    let completed = *completion_counter.lock().await;
    assert!(completed > 0, "Some calls should have completed");
    assert!(completed <= total_permits as u32, 
        "Completed count should not exceed permits released");
}

#[tokio::test]
async fn test_circuit_breaker_different_error_types() {
    let state_store = Arc::new(MockStateStore::new());
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_ms: 1000,
        half_open_max_calls: 2,
    };
    
    let circuit_breaker = CircuitBreaker::new("test-cb-3".to_string(), config, state_store);
    
    // Allow time for async state loading
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // First failure with ComponentError
    let result = circuit_breaker.execute(async {
        Err::<(), _>(CoreError::ComponentError("Test component error".to_string()))
    }).await;
    assert!(result.is_err());
    
    // Second failure with StateStoreError
    let result = circuit_breaker.execute(async {
        Err::<(), _>(CoreError::StateStoreError("Test state store error".to_string()))
    }).await;
    assert!(result.is_err());
    
    // Third failure with ExternalDependencyError - should trip circuit
    let result = circuit_breaker.execute(async {
        Err::<(), _>(CoreError::ExternalDependencyError("Test external dependency error".to_string()))
    }).await;
    assert!(result.is_err());
    
    // Verify circuit is now open
    assert_eq!(circuit_breaker.get_state().await.unwrap(), CircuitBreakerState::Open);
}

#[tokio::test]
async fn test_circuit_breaker_rejects_when_open() {
    let state_store = Arc::new(MockStateStore::new());
    let config = CircuitBreakerConfig {
        failure_threshold: 1, // Set to 1 to trip circuit quickly
        reset_timeout_ms: 1000,
        half_open_max_calls: 2,
    };
    
    let circuit_breaker = CircuitBreaker::new("test-cb-4".to_string(), config, state_store);
    
    // Allow time for async state loading
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Trip the circuit with one failure
    let result = circuit_breaker.execute(async {
        Err::<(), _>(CoreError::ComponentError("Test failure".to_string()))
    }).await;
    assert!(result.is_err());
    
    // Verify circuit is now open
    assert_eq!(circuit_breaker.get_state().await.unwrap(), CircuitBreakerState::Open);
    
    // Attempt to execute when circuit is open
    let result = circuit_breaker.execute(async {
        Ok::<_, CoreError>(42) // Even with a successful operation, it should be rejected
    }).await;
    
    // Verify the operation was rejected
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, CoreError::ComponentError(msg) if msg.contains("is open")));
    }
}

#[tokio::test]
async fn test_circuit_breaker_state_transitions() {
    // Create a circuit breaker with a shorter reset timeout for testing
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_ms: 200, // Shorter timeout for faster test
        half_open_max_calls: 1,
    };
    
    let state_store = Arc::new(MockStateStore::new());
    let circuit_id = "test_circuit_transitions";
    
    // Create circuit breaker
    let circuit_breaker = CircuitBreaker::new(
        circuit_id.to_string(),
        config,
        state_store.clone()
    );
    
    // Wait for initial state to be loaded
    sleep(Duration::from_millis(50)).await;
    
    // Initially circuit should be closed
    let initial_state = circuit_breaker.get_state().await.unwrap();
    assert_eq!(initial_state, CircuitBreakerState::Closed, 
        "Circuit should start in closed state");
    
    // Execute successful operation
    let result = circuit_breaker.execute(async {
        Ok::<_, CoreError>(json!({"result": "success"}))
    }).await;
    
    assert!(result.is_ok(), "Successful operation should pass through circuit breaker");
    
    // Record failures to trip the circuit breaker
    for i in 1..=3 {
        let result = circuit_breaker.execute(async {
            Err::<Value, _>(CoreError::ComponentError("Test failure".to_string()))
        }).await;
        
        assert!(result.is_err(), "Circuit should allow call {} but result in failure", i);
        
        let state_after = circuit_breaker.get_state().await.unwrap();
        assert_eq!(state_after, 
            if i >= 3 { CircuitBreakerState::Open } else { CircuitBreakerState::Closed },
            "Circuit state after {} failures", i);
    }
    
    // Circuit is now open, calls should fail fast
    let start = std::time::Instant::now();
    let result = circuit_breaker.execute(async {
        // This would take time if it executed, but should be skipped
        sleep(Duration::from_millis(100)).await;
        Ok::<Value, CoreError>(json!({"result": "success"}))
    }).await;
    
    assert!(result.is_err(), "Circuit should be open and fail fast");
    assert!(start.elapsed() < Duration::from_millis(50), 
        "Open circuit should fail fast without executing operation");
    
    // Wait for reset timeout plus some buffer
    sleep(Duration::from_millis(250)).await;
    
    // Check if state transitioned to half-open
    let state_after_wait = circuit_breaker.get_state().await.unwrap();
    assert_eq!(state_after_wait, CircuitBreakerState::HalfOpen,
        "Circuit should transition to half-open after reset timeout");
    
    // A successful call in half-open state should close the circuit
    let result = circuit_breaker.execute(async {
        Ok::<Value, CoreError>(json!({"result": "success"}))
    }).await;
    
    assert!(result.is_ok(), "Half-open circuit should allow the call");
    
    // Check final state
    sleep(Duration::from_millis(50)).await; // Allow state to update
    let final_state = circuit_breaker.get_state().await.unwrap();
    assert_eq!(final_state, CircuitBreakerState::Closed,
        "Circuit should be closed after successful call in half-open state");
}

#[tokio::test]
async fn test_circuit_breaker_with_failure_in_half_open() {
    // Create a circuit breaker with a shorter reset timeout for testing
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        reset_timeout_ms: 200, // Shorter timeout for faster test
        half_open_max_calls: 1,
    };
    
    let state_store = Arc::new(MockStateStore::new());
    let circuit_id = "test_circuit_half_open_failure";
    
    // Create circuit breaker
    let circuit_breaker = CircuitBreaker::new(
        circuit_id.to_string(),
        config,
        state_store.clone()
    );
    
    // Wait for initial state to be loaded
    sleep(Duration::from_millis(50)).await;
    
    // Trip the circuit with failures
    for _ in 1..=2 {
        let _ = circuit_breaker.execute(async {
            Err::<Value, _>(CoreError::ComponentError("Test failure".to_string()))
        }).await;
    }
    
    // Verify circuit is open
    let state = circuit_breaker.get_state().await.unwrap();
    assert_eq!(state, CircuitBreakerState::Open, "Circuit should be open after failures");
    
    // Wait for reset timeout
    sleep(Duration::from_millis(250)).await;
    
    // Verify circuit is half-open
    let state = circuit_breaker.get_state().await.unwrap();
    assert_eq!(state, CircuitBreakerState::HalfOpen, "Circuit should be half-open after timeout");
    
    // Execute a failing call in half-open state
    let result = circuit_breaker.execute(async {
        Err::<Value, _>(CoreError::ComponentError("Another failure".to_string()))
    }).await;
    
    assert!(result.is_err(), "Call should fail");
    
    // Circuit should return to open state
    sleep(Duration::from_millis(50)).await; // Allow state to update
    let final_state = circuit_breaker.get_state().await.unwrap();
    assert_eq!(final_state, CircuitBreakerState::Open,
        "Circuit should be open after failure in half-open state");
}

#[tokio::test]
async fn test_circuit_breaker_persistence() {
    // Create a circuit breaker with a shorter reset timeout for testing
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        reset_timeout_ms: 200, // Shorter timeout for faster test
        half_open_max_calls: 1,
    };
    
    let state_store = Arc::new(MockStateStore::new());
    let circuit_id = "test_circuit_persistence";
    
    // Create and trip a circuit breaker
    {
        let circuit_breaker = CircuitBreaker::new(
            circuit_id.to_string(),
            config.clone(),
            state_store.clone()
        );
        
        // Wait for initial state to be loaded
        sleep(Duration::from_millis(50)).await;
        
        // Trip the circuit with failures
        for _ in 1..=2 {
            let _ = circuit_breaker.execute(async {
                Err::<Value, _>(CoreError::ComponentError("Test failure".to_string()))
            }).await;
        }
        
        // Verify circuit is open
        let state = circuit_breaker.get_state().await.unwrap();
        assert_eq!(state, CircuitBreakerState::Open, "Circuit should be open after failures");
        
        // Allow state to be saved
        sleep(Duration::from_millis(50)).await;
    }
    
    // Create a new circuit breaker instance with the same ID
    let circuit_breaker2 = CircuitBreaker::new(
        circuit_id.to_string(),
        config,
        state_store.clone()
    );
    
    // Allow state to be loaded
    sleep(Duration::from_millis(50)).await;
    
    // Check if the state was correctly restored
    let loaded_state = circuit_breaker2.get_state().await.unwrap();
    assert_eq!(loaded_state, CircuitBreakerState::Open, 
        "Circuit state should be restored from persistent storage");
} 
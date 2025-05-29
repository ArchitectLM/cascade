//! Integration tests for the RetryWrapper component functionality
//! 
//! These tests verify that the retry component correctly implements
//! the retry pattern with backoff and jitter.
//!
//! Tests components: RetryWrapper, ResiliencePatterns
//! Tests APIs: execute, calculate_delay, save_state, get_state
//! Tests features: backoff, jitter, error handling, resilience

use std::sync::Arc;
use std::collections::HashMap;
use serde_json::json;
use tokio::sync::Mutex;
use regex::Regex;

use cascade_test_utils::mocks::component::{
    create_mock_component_runtime_api,
    ComponentRuntimeAPI
};
use cascade_core::{ComponentExecutor, ExecutionResult};
use cascade_stdlib::components::retry::RetryWrapper;

/// Extended MockComponentRuntimeAPI with helper methods for retry tests
#[derive(Clone)]
struct TestRuntimeAPI {
    api: Arc<cascade_test_utils::mocks::component::MockComponentRuntimeAPI>,
    config_values: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    inputs: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    outputs: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    state: Arc<Mutex<Option<serde_json::Value>>>,
    shared_state: Arc<Mutex<HashMap<String, HashMap<String, serde_json::Value>>>>,
}

impl TestRuntimeAPI {
    async fn new(name: &str) -> Self {
        Self {
            api: Arc::new(create_mock_component_runtime_api(name)),
            config_values: Arc::new(Mutex::new(HashMap::new())),
            inputs: Arc::new(Mutex::new(HashMap::new())),
            outputs: Arc::new(Mutex::new(HashMap::new())),
            state: Arc::new(Mutex::new(None)),
            shared_state: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn set_config_value(&self, key: &str, value: serde_json::Value) {
        let mut config = self.config_values.lock().await;
        config.insert(key.to_string(), value);
    }

    async fn set_input_value(&self, key: &str, value: serde_json::Value) {
        let mut inputs = self.inputs.lock().await;
        inputs.insert(key.to_string(), value);
    }

    async fn set_state(&self, state: serde_json::Value) {
        let mut state_guard = self.state.lock().await;
        *state_guard = Some(state);
    }

    async fn get_saved_state(&self) -> Option<serde_json::Value> {
        let state = self.state.lock().await;
        state.clone()
    }

    async fn get_output_value(&self, key: &str) -> Option<serde_json::Value> {
        let outputs = self.outputs.lock().await;
        outputs.get(key).cloned()
    }

    async fn set_shared_state_value(&self, scope: &str, key: &str, value: serde_json::Value) {
        let mut shared_state = self.shared_state.lock().await;
        let scope_map = shared_state.entry(scope.to_string()).or_insert_with(HashMap::new);
        scope_map.insert(key.to_string(), value);
    }

    async fn get_shared_state_value(&self, scope: &str, key: &str) -> Option<serde_json::Value> {
        let shared_state = self.shared_state.lock().await;
        shared_state.get(scope)
            .and_then(|scope_map| scope_map.get(key))
            .cloned()
    }

    async fn into_runtime_api(self) -> Arc<dyn cascade_core::ComponentRuntimeApi> {
        // Create a new struct that will wrap our API and implement the required trait
        struct RuntimeAPIAdapter {
            config_values: HashMap<String, serde_json::Value>,
            inputs: HashMap<String, serde_json::Value>,
            outputs: Arc<Mutex<HashMap<String, serde_json::Value>>>,
            state: Arc<Mutex<Option<serde_json::Value>>>,
            shared_state: Arc<Mutex<HashMap<String, HashMap<String, serde_json::Value>>>>,
            name: String,
        }

        impl cascade_core::ComponentRuntimeApiBase for RuntimeAPIAdapter {
            fn log(&self, _level: tracing::Level, _message: &str) {
                // No-op for testing
            }
        }

        #[async_trait::async_trait]
        impl cascade_core::ComponentRuntimeApi for RuntimeAPIAdapter {
            async fn get_input(&self, name: &str) -> core::result::Result<cascade_core::types::DataPacket, cascade_core::CoreError> {
                // Check if it's asking for a config value
                if name.starts_with("config.") {
                    let config_key = name.strip_prefix("config.").unwrap_or(name);
                    if let Some(value) = self.config_values.get(config_key) {
                        return Ok(cascade_core::types::DataPacket::new(value.clone()));
                    }
                }
                
                // Check regular inputs
                if let Some(value) = self.inputs.get(name) {
                    return Ok(cascade_core::types::DataPacket::new(value.clone()));
                }
                
                Err(cascade_core::CoreError::IOError(format!("Input not found: {}", name)))
            }

            async fn get_config(&self, name: &str) -> core::result::Result<serde_json::Value, cascade_core::CoreError> {
                if let Some(value) = self.config_values.get(name) {
                    Ok(value.clone())
                } else {
                    Err(cascade_core::CoreError::ConfigurationError(format!("Config not found: {}", name)))
                }
            }

            async fn set_output(&self, name: &str, value: cascade_core::types::DataPacket) -> core::result::Result<(), cascade_core::CoreError> {
                let mut outputs = self.outputs.lock().await;
                outputs.insert(name.to_string(), value.value);
                Ok(())
            }

            async fn get_state(&self) -> core::result::Result<Option<serde_json::Value>, cascade_core::CoreError> {
                let state = self.state.lock().await;
                Ok(state.clone())
            }

            async fn save_state(&self, value: serde_json::Value) -> core::result::Result<(), cascade_core::CoreError> {
                let mut state = self.state.lock().await;
                *state = Some(value);
                Ok(())
            }

            async fn schedule_timer(&self, _duration: std::time::Duration) -> core::result::Result<(), cascade_core::CoreError> {
                // Mock implementation - for tests we don't actually schedule timers
                Ok(())
            }

            async fn correlation_id(&self) -> core::result::Result<cascade_core::CorrelationId, cascade_core::CoreError> {
                Ok(cascade_core::CorrelationId("test-correlation-id".to_string()))
            }

            async fn log(&self, level: cascade_core::LogLevel, message: &str) -> core::result::Result<(), cascade_core::CoreError> {
                println!("[{:?}] {} - {}", level, self.name, message);
                Ok(())
            }

            async fn emit_metric(&self, _name: &str, _value: f64, _labels: std::collections::HashMap<String, String>) -> core::result::Result<(), cascade_core::CoreError> {
                // Mock implementation - just return Ok
                Ok(())
            }

            async fn get_shared_state(&self, scope_key: &str, key: &str) -> core::result::Result<Option<serde_json::Value>, cascade_core::CoreError> {
                let shared_state = self.shared_state.lock().await;
                let result = shared_state.get(scope_key)
                    .and_then(|scope_map| scope_map.get(key))
                    .cloned();
                Ok(result)
            }

            async fn set_shared_state(&self, scope_key: &str, key: &str, value: serde_json::Value) -> core::result::Result<(), cascade_core::CoreError> {
                let mut shared_state = self.shared_state.lock().await;
                let scope_map = shared_state.entry(scope_key.to_string()).or_insert_with(HashMap::new);
                scope_map.insert(key.to_string(), value);
                Ok(())
            }

            async fn set_shared_state_with_ttl(&self, scope_key: &str, key: &str, value: serde_json::Value, _ttl_ms: u64) -> core::result::Result<(), cascade_core::CoreError> {
                // Just use regular set_shared_state for the mock
                self.set_shared_state(scope_key, key, value).await
            }
        }

        let config_values = self.config_values.lock().await.clone();
        let inputs = self.inputs.lock().await.clone();
        let outputs_arc = self.outputs.clone();
        let state_arc = self.state.clone();
        let shared_state_arc = self.shared_state.clone();
        
        let adapter = RuntimeAPIAdapter {
            config_values,
            inputs,
            outputs: outputs_arc,
            state: state_arc,
            shared_state: shared_state_arc,
            name: self.api.name().to_string(),
        };
        
        Arc::new(adapter)
    }
}

/// Test that verifies the retry component correctly implements backoff with jitter
#[tokio::test]
async fn test_retry_backoff_with_jitter() {
    // Create the RetryWrapper component to test
    let retry_wrapper = RetryWrapper::new();
    
    // Create a mock runtime API for testing
    let mock_api = TestRuntimeAPI::new("retry-test").await;
    
    // Configure inputs for a retry scenario
    
    // Setup for first attempt
    mock_api.set_config_value("maxAttempts", json!(3)).await;
    mock_api.set_config_value("initialDelayMs", json!(100)).await;
    mock_api.set_config_value("backoffMultiplier", json!(2.0)).await;
    mock_api.set_config_value("maxDelayMs", json!(1000)).await;
    
    // First attempt fails
    mock_api.set_input_value("operationError", json!({
        "errorCode": "CONNECTION_ERROR",
        "message": "Connection failed"
    })).await;
    
    // First retry should be scheduled
    let mock_api_clone = mock_api.clone();
    let runtime_api = mock_api.into_runtime_api().await;
    let result = retry_wrapper.execute(runtime_api).await;
    assert!(matches!(result, ExecutionResult::Pending(_)), "Expected Pending result when scheduling retry, got {:?}", result);
    
    // Get saved component state
    let state_opt = mock_api_clone.get_saved_state().await;
    assert!(state_opt.is_some());
    
    let state = state_opt.unwrap();
    println!("Retry state after first failure: {}", state);
    
    // Check state properties
    assert_eq!(state["attempt"], 2); // Should be on attempt 2 now
    assert_eq!(state["max_attempts"], 3);
    assert_eq!(state["initial_delay_ms"], 100);
    assert_eq!(state["backoff_multiplier"], 2.0);
    assert_eq!(state["completed"], false);
    
    // Second attempt
    let second_api = TestRuntimeAPI::new("retry-test-2").await;
    second_api.set_config_value("maxAttempts", json!(3)).await;
    second_api.set_config_value("initialDelayMs", json!(100)).await;
    second_api.set_config_value("backoffMultiplier", json!(2.0)).await;
    second_api.set_config_value("maxDelayMs", json!(1000)).await;
    second_api.set_state(state).await;
    
    // Second attempt succeeds
    second_api.set_input_value("operationResult", json!({
        "status": "success",
        "data": { "message": "Operation completed successfully" }
    })).await;
    
    // Execute retry with success
    let second_api_clone = second_api.clone();
    let runtime_api = second_api.into_runtime_api().await;
    let result = retry_wrapper.execute(runtime_api).await;
    assert!(matches!(result, ExecutionResult::Success));
    
    // Check final state
    let final_state = second_api_clone.get_saved_state().await.unwrap();
    assert_eq!(final_state["completed"], true);
    assert_eq!(final_state["final_result"], true);
    
    // Check output was set correctly
    let output = second_api_clone.get_output_value("result").await;
    assert!(output.is_some());
    assert_eq!(output.unwrap()["status"], "success");
}

/// Test that verifies the retry component handles max retries exhaustion correctly
#[tokio::test]
async fn test_retry_max_attempts_exhausted() {
    // Create the RetryWrapper component to test
    let retry_wrapper = RetryWrapper::new();
    
    // Create a mock runtime API
    let api = TestRuntimeAPI::new("retry-exhausted-test").await;
    
    // Configure for 2 max attempts
    api.set_config_value("maxAttempts", json!(2)).await;
    api.set_config_value("initialDelayMs", json!(10)).await;
    
    // First attempt - fail
    api.set_input_value("operationError", json!({
        "error": "Failed operation"
    })).await;
    
    // First execution - should set up retry
    let api_clone = api.clone();
    let runtime_api = api.into_runtime_api().await;
    let result = retry_wrapper.execute(runtime_api).await;
    assert!(matches!(result, ExecutionResult::Pending(_)), "Expected Pending result when scheduling retry, got {:?}", result);
    
    // Get the saved state
    let state = api_clone.get_saved_state().await.unwrap();
    assert_eq!(state["attempt"], 2);
    assert_eq!(state["completed"], false);
    
    // Second attempt - fail again
    let second_api = TestRuntimeAPI::new("retry-exhausted-test-2").await;
    second_api.set_config_value("maxAttempts", json!(2)).await;
    second_api.set_config_value("initialDelayMs", json!(10)).await;
    second_api.set_state(state).await;
    
    second_api.set_input_value("operationError", json!({
        "error": "Failed operation again"
    })).await;
    
    // Execute retry - should exhaust and set error
    let second_api_clone = second_api.clone();
    let runtime_api = second_api.into_runtime_api().await;
    let result = retry_wrapper.execute(runtime_api).await;
    assert!(matches!(result, ExecutionResult::Success));
    
    // Check final state
    let final_state = second_api_clone.get_saved_state().await.unwrap();
    assert_eq!(final_state["completed"], true);
    assert_eq!(final_state["final_result"], false); // Indicates failure
    
    // Check error output was set
    let output = second_api_clone.get_output_value("error").await;
    assert!(output.is_some());
    assert_eq!(output.unwrap()["error"], "Failed operation again");
}

/// Test that verifies the jitter calculation logic produces different delays
#[tokio::test]
async fn test_retry_jitter_varies_delay() {
    // Create multiple RetryWrapper executions to verify jitter
    // We'll run multiple retry attempts with identical configuration
    // and check that the delays vary due to jitter
    
    // Setup constants
    let initial_delay_ms = 100;
    let max_attempts = 3;
    let backoff_multiplier = 2.0;
    let max_delay_ms = 1000;
    let test_runs = 10;
    
    // Collect actual delays from multiple runs
    let mut delays = Vec::with_capacity(test_runs);
    
    // Create retry component and run multiple times
    let retry_wrapper = RetryWrapper::new();
    
    for i in 0..test_runs {
        // Setup the API with identical configuration
        let api = TestRuntimeAPI::new(&format!("retry-jitter-test-{}", i)).await;
        api.set_config_value("maxAttempts", json!(max_attempts)).await;
        api.set_config_value("initialDelayMs", json!(initial_delay_ms)).await;
        api.set_config_value("backoffMultiplier", json!(backoff_multiplier)).await;
        api.set_config_value("maxDelayMs", json!(max_delay_ms)).await;
        
        // First attempt fails
        api.set_input_value("operationError", json!({
            "error": "Test error"
        })).await;
        
        // Execute will compute the next delay internally
        let api_clone = api.clone();
        let runtime_api = api.into_runtime_api().await;
        let result = retry_wrapper.execute(runtime_api).await;
        assert!(matches!(result, ExecutionResult::Pending(_)), "Expected Pending result when scheduling retry, got {:?}", result);
        
        // Get the saved state to extract info about the delay
        let state = api_clone.get_saved_state().await.unwrap();
        
        // Extract delay from the pending message
        if let ExecutionResult::Pending(message) = result {
            // Parse the delay from the message - example format: "Scheduled retry attempt 2 after 123ms"
            let re = Regex::new(r"after (\d+)ms").unwrap();
            if let Some(captures) = re.captures(&message) {
                if let Some(delay_str) = captures.get(1) {
                    if let Ok(delay) = delay_str.as_str().parse::<u64>() {
                        delays.push(delay);
                        continue;
                    }
                }
            }
        }
        
        // If we can't get delay from the message, try to get it from the state
        if let Some(delay_ms) = state.get("delay_ms") {
            if let Some(delay) = delay_ms.as_u64() {
                delays.push(delay);
            }
        }
    }
    
    // Verify we got some delay values
    assert!(!delays.is_empty(), "No delay values were extracted from the retry component");
    
    // Verify multiple runs produce different delays
    let unique_delays = delays.iter().collect::<std::collections::HashSet<_>>();
    println!("Delay variations from jitter (should vary): {:?}", delays);
    
    // Should have more than 1 unique delay if jitter is working
    assert!(unique_delays.len() > 1, 
        "Jitter not working - all delays identical: {:?}", delays);
    
    // Verify delays are in reasonable range
    for &delay in &delays {
        // Calculate expected range with jitter (allow +/- 20%)
        let base_delay_ms = initial_delay_ms as f64 * backoff_multiplier;
        let min_expected = ((base_delay_ms * 0.8) as u64).max(1);
        let max_expected = (base_delay_ms * 1.2) as u64;
        
        assert!(delay >= min_expected && delay <= max_expected,
            "Delay outside expected range: {} not in [{}, {}]", 
            delay, min_expected, max_expected);
    }
}

/// Test that verifies the retry component can use shared state between attempts
#[tokio::test]
async fn test_retry_with_shared_state() {
    // Create the RetryWrapper component
    let retry_wrapper = RetryWrapper::new();
    
    // Create a mock runtime API with shared state support
    let api = TestRuntimeAPI::new("retry-shared-state-test").await;
    
    // Configure with shared state
    api.set_config_value("maxAttempts", json!(3)).await;
    api.set_config_value("initialDelayMs", json!(10)).await;
    api.set_config_value("sharedStateScope", json!("workflow")).await;
    api.set_config_value("retryId", json!("test-retry-123")).await;
    
    // First attempt fails
    api.set_input_value("operationError", json!({
        "error": "Failed operation"
    })).await;

    // We need to add shared state support to the runtime API
    let api_clone = api.clone();
    let api_arc = api.into_runtime_api().await;
    
    // First execution - should set up retry and save to shared state
    let result = retry_wrapper.execute(api_arc).await;
    assert!(matches!(result, ExecutionResult::Pending(_)), "Expected Pending result when scheduling retry, got {:?}", result);
    
    // Check shared state was saved
    let shared_state = api_clone.get_shared_state_value("workflow", "retry:test-retry-123").await;
    assert!(shared_state.is_some());
    
    let state = shared_state.unwrap();
    assert_eq!(state["attempt"], 2);
    assert_eq!(state["retry_id"], "test-retry-123");
    
    // Second attempt with same shared state
    let second_api = TestRuntimeAPI::new("retry-shared-state-test-2").await;
    second_api.set_config_value("sharedStateScope", json!("workflow")).await;
    second_api.set_config_value("retryId", json!("test-retry-123")).await;
    second_api.set_shared_state_value("workflow", "retry:test-retry-123", state.clone()).await;
    
    // Second attempt succeeds
    second_api.set_input_value("operationResult", json!({
        "data": "Operation succeeded"
    })).await;
    
    // Execute retry with success
    let second_api_clone = second_api.clone();
    let runtime_api = second_api.into_runtime_api().await;
    let result = retry_wrapper.execute(runtime_api).await;
    assert!(matches!(result, ExecutionResult::Success));
    
    // Check shared state was updated
    let final_shared_state = second_api_clone.get_shared_state_value("workflow", "retry:test-retry-123").await;
    assert!(final_shared_state.is_some());
    
    let final_state = final_shared_state.unwrap();
    assert_eq!(final_state["completed"], true);
    assert_eq!(final_state["final_result"], true);
} 
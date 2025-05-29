//! Example tests demonstrating resilience patterns
//! 
//! This file replaces and consolidates the functionality that was previously in
//! the separate edge_resilience_patterns_test.rs in the cascade-edge crate.
//! All edge resilience tests are now centralized here.

// This test file implements its own versions of the resilience patterns
// without depending on cascade-edge or cascade-core.

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use std::pin::Pin;
use std::future::Future;
use std::collections::HashMap;
use serde_json::{json, Value};
use async_trait::async_trait;
use anyhow::Result;

// Internal test implementations

// Basic mocks for component interfaces
#[derive(Debug, Clone, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug)]
pub struct ComponentError(pub String);

impl std::fmt::Display for ComponentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ComponentError: {}", self.0)
    }
}

impl std::error::Error for ComponentError {}

#[derive(Debug, Clone)]
pub struct DataPacket {
    pub content_type: String,
    pub data: Value,
}

#[async_trait]
pub trait ComponentRuntimeAPI: Send + Sync {
    async fn log(&self, level: LogLevel, message: &str) -> Result<(), ComponentError>;
    async fn get_input(&self, name: &str) -> Result<Option<DataPacket>, ComponentError>;
    async fn set_output(&self, name: &str, data: DataPacket) -> Result<(), ComponentError>;
    async fn get_state(&self, key: &str) -> Result<Option<Value>, ComponentError>;
    async fn set_state(&self, key: &str, value: Value) -> Result<(), ComponentError>;
    fn name(&self) -> &str;
}

// Helper function to create a mock component runtime API for testing
pub fn create_mock_component_runtime_api(name: &str) -> impl ComponentRuntimeAPI {
    struct MockRuntimeAPI {
        name: String,
        state: Mutex<HashMap<String, Value>>,
    }
    
    impl MockRuntimeAPI {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                state: Mutex::new(HashMap::new()),
            }
        }
    }
    
    #[async_trait]
    impl ComponentRuntimeAPI for MockRuntimeAPI {
        async fn log(&self, level: LogLevel, message: &str) -> Result<(), ComponentError> {
            println!("[{:?}] {}: {}", level, self.name, message);
            Ok(())
        }
        
        async fn get_input(&self, name: &str) -> Result<Option<DataPacket>, ComponentError> {
            println!("Getting input: {}", name);
            Ok(None)
        }
        
        async fn set_output(&self, name: &str, data: DataPacket) -> Result<(), ComponentError> {
            println!("Setting output {}: {}", name, data.data);
            Ok(())
        }
        
        async fn get_state(&self, key: &str) -> Result<Option<Value>, ComponentError> {
            let state = self.state.lock().await;
            Ok(state.get(key).cloned())
        }
        
        async fn set_state(&self, key: &str, value: Value) -> Result<(), ComponentError> {
            let mut state = self.state.lock().await;
            state.insert(key.to_string(), value);
            Ok(())
        }
        
        fn name(&self) -> &str {
            &self.name
        }
    }
    
    MockRuntimeAPI::new(name)
}

// Helper function to create a mock component executor for testing
pub fn create_mock_component_executor() -> MockComponentExecutor {
    MockComponentExecutor::new()
}

/// Internal circuit breaker configuration
#[derive(Debug, Clone)]
pub struct TestCircuitBreakerConfig {
    pub failure_threshold: u32,
    pub reset_timeout_ms: u64,
    pub half_open_max_calls: u32,
}

/// Circuit breaker state
#[derive(Debug, Clone, PartialEq)]
pub enum TestCircuitState {
    CLOSED,
    OPEN,
    HALF_OPEN,
}

impl std::fmt::Display for TestCircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CLOSED => write!(f, "CLOSED"),
            Self::OPEN => write!(f, "OPEN"),
            Self::HALF_OPEN => write!(f, "HALF_OPEN"),
        }
    }
}

/// State store for resilience patterns
#[async_trait]
pub trait StateStore: Send + Sync {
    async fn get_flow_instance_state(&self, flow_id: &str, instance_id: &str) -> Result<Option<serde_json::Value>>;
    async fn set_flow_instance_state(&self, flow_id: &str, instance_id: &str, state: serde_json::Value) -> Result<()>;
    async fn get_component_state(&self, component_id: &str) -> Result<Option<serde_json::Value>>;
    async fn set_component_state(&self, component_id: &str, state: serde_json::Value) -> Result<()>;
}

/// Memory state store implementation
pub struct MemoryStateStore {
    states: Arc<Mutex<HashMap<String, String>>>,
}

impl MemoryStateStore {
    pub fn new() -> Self {
        Self {
            states: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl StateStore for MemoryStateStore {
    async fn get_flow_instance_state(&self, flow_id: &str, instance_id: &str) -> Result<Option<serde_json::Value>> {
        let key = format!("{}:{}", flow_id, instance_id);
        let states = self.states.lock().await;
        Ok(states.get(&key).map(|s| serde_json::from_str(s).unwrap_or(json!({}))))
    }

    async fn set_flow_instance_state(&self, flow_id: &str, instance_id: &str, state: serde_json::Value) -> Result<()> {
        let key = format!("{}:{}", flow_id, instance_id);
        let mut states = self.states.lock().await;
        states.insert(key, state.to_string());
        Ok(())
    }
    
    async fn get_component_state(&self, component_id: &str) -> Result<Option<serde_json::Value>> {
        let states = self.states.lock().await;
        Ok(states.get(component_id).map(|s| serde_json::from_str(s).unwrap_or(json!({}))))
    }
    
    async fn set_component_state(&self, component_id: &str, state: serde_json::Value) -> Result<()> {
        let mut states = self.states.lock().await;
        states.insert(component_id.to_string(), state.to_string());
        Ok(())
    }
}

/// Simple bulkhead config
#[derive(Debug, Clone)]
pub struct TestBulkheadConfig {
    pub max_concurrent: u32,
    pub max_queue_size: u32,
}

/// Circuit breaker implementation  
pub struct CircuitBreaker {
    id: String,
    config: TestCircuitBreakerConfig,
    state_store: Arc<dyn StateStore>,
}

impl CircuitBreaker {
    pub fn new(
        id: String,
        config: TestCircuitBreakerConfig,
        state_store: Arc<dyn StateStore>,
    ) -> Self {
        Self {
            id,
            config,
            state_store,
        }
    }
}

/// Bulkhead implementation
pub struct Bulkhead {
    id: String,
    config: TestBulkheadConfig,
}

impl Bulkhead {
    pub fn new(
        id: String,
        config: TestBulkheadConfig,
    ) -> Self {
        Self {
            id,
            config,
        }
    }
}

// We need to implement our own SimpleTimerService for this test
#[derive(Clone)]
struct SimpleTimerService {
    timers: Arc<Mutex<Vec<(u64, Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync + 'static>)>>>,
}

impl SimpleTimerService {
    fn new() -> Self {
        Self {
            timers: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    async fn schedule_timer(
        &self,
        timeout_ms: u64,
        callback: Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync + 'static>,
    ) -> Result<String, String> {
        let mut timers = self.timers.lock().await;
        timers.push((timeout_ms, callback));
        let timer_id = format!("timer-{}", timers.len());
        Ok(timer_id)
    }
    
    async fn advance_time(&self, _duration_ms: Duration) -> Result<(), String> {
        let timers = {
            let mut guard = self.timers.lock().await;
            std::mem::take(&mut *guard)
        };
        
        for (_, callback) in timers {
            callback().await?;
        }
        
        Ok(())
    }
}

/// Mocked ComponentRuntimeAPI for tests
#[derive(Clone)]
struct MockComponentRuntimeAPI {
    name: String,
    outputs: Arc<Mutex<HashMap<String, Value>>>,
}

impl MockComponentRuntimeAPI {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            outputs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    async fn set_output(&self, key: &str, value: Value) -> Result<(), String> {
        let mut outputs = self.outputs.lock().await;
        outputs.insert(key.to_string(), value);
        Ok(())
    }
    
    async fn set_error(&self, code: &str, details: Value) -> Result<(), String> {
        let mut outputs = self.outputs.lock().await;
        outputs.insert("error".to_string(), json!({
            "code": code,
            "details": details,
        }));
        Ok(())
    }
    
    async fn get_output(&self, key: &str) -> Result<Value, String> {
        let outputs = self.outputs.lock().await;
        outputs.get(key)
            .cloned()
            .ok_or_else(|| format!("Output '{}' not found", key))
    }
}

/// Helper function to simulate an error
async fn simulate_error(
    runtime_api: Arc<MockComponentRuntimeAPI>,
    error_message: String,
) -> Result<(), String> {
    runtime_api.set_error("COMPONENT_ERROR", json!({ "message": error_message })).await?;
    Err(format!("Component error: {}", error_message))
}

/// Simple state tracking for test
#[derive(Debug, Default, Clone)]
struct CircuitState {
    open_count: usize,
    closed_count: usize,
    half_open_count: usize,
    success_count: usize,
    failure_count: usize,
}

/// A minimal implementation of a circuit breaker for testing (Test 1)
struct TestCircuitBreaker {
    failure_threshold: u32,
    reset_timeout_ms: u64,
    state: Arc<Mutex<TestCircuitState>>,
    counters: Arc<Mutex<CircuitState>>,
    timer_service: Arc<SimpleTimerService>,
}

impl TestCircuitBreaker {
    fn new(
        failure_threshold: u32,
        reset_timeout_ms: u64,
        timer_service: Arc<SimpleTimerService>,
    ) -> Self {
        Self {
            failure_threshold,
            reset_timeout_ms,
            state: Arc::new(Mutex::new(TestCircuitState::CLOSED)),
            counters: Arc::new(Mutex::new(CircuitState::default())),
            timer_service,
        }
    }

    async fn get_state(&self) -> TestCircuitState {
        let state = self.state.lock().await;
        state.clone()
    }

    async fn record_success(&self) {
        // Acquire locks separately to avoid issues
        {
            let mut counters = self.counters.lock().await;
            counters.success_count += 1;
        }
        
        let mut state = self.state.lock().await;
        if *state == TestCircuitState::HALF_OPEN {
            *state = TestCircuitState::CLOSED;
            drop(state);
            
            let mut counters = self.counters.lock().await;
            counters.closed_count += 1;
        }
    }

    async fn record_failure(&self) {
        // First check if we should trip the circuit
        let should_trip = {
            let mut counters = self.counters.lock().await;
            counters.failure_count += 1;
            let current_failure_count = counters.failure_count as u32;
            current_failure_count >= self.failure_threshold
        };
        
        let current_state = self.get_state().await;
        
        if current_state == TestCircuitState::CLOSED && should_trip {
            {
                let mut state = self.state.lock().await;
                *state = TestCircuitState::OPEN;
            }
            
            {
                let mut counters = self.counters.lock().await;
                counters.open_count += 1;
            }
            
            // Schedule a transition to half-open after the timeout
            let timer_service = self.timer_service.clone();
            let state_arc = self.state.clone();
            let counters_arc = self.counters.clone();
            
            // Create safe callback for timer
            let callback = Box::new(move || -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                let state = state_arc.clone();
                let counters = counters_arc.clone();
                
                Box::pin(async move {
                    let is_open = {
                        let state_guard = state.lock().await;
                        *state_guard == TestCircuitState::OPEN
                    };
                    
                    if is_open {
                        println!("Timer triggered: changing state from OPEN to HALF_OPEN");
                        {
                            let mut state_guard = state.lock().await;
                            *state_guard = TestCircuitState::HALF_OPEN;
                        }
                        
                        {
                            let mut counters_guard = counters.lock().await;
                            counters_guard.half_open_count += 1;
                        }
                    }
                    Ok(())
                })
            });
            
            let _timer_id = timer_service.schedule_timer(
                self.reset_timeout_ms,
                callback,
            ).await.unwrap();
        } else if current_state == TestCircuitState::HALF_OPEN {
            {
                let mut state = self.state.lock().await;
                *state = TestCircuitState::OPEN;
            }
            
            {
                let mut counters = self.counters.lock().await;
                counters.open_count += 1;
            }
        }
    }

    async fn execute<F, Fut, T>(&self, operation: F) -> Result<T, String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        let state = self.get_state().await;
        
        if state == TestCircuitState::OPEN {
            return Err("Circuit breaker is open".to_string());
        }
        
        match operation().await {
            Ok(result) => {
                self.record_success().await;
                Ok(result)
            }
            Err(e) => {
                self.record_failure().await;
                Err(e)
            }
        }
    }

    async fn get_metrics(&self) -> CircuitState {
        let counters = self.counters.lock().await;
        counters.clone()
    }
}

// Simplified MockComponentExecutor
#[derive(Clone)]
struct MockComponentExecutor {
    fail_count: Arc<Mutex<usize>>,
    current_fail: Arc<Mutex<usize>>,
    success_handler: Arc<Mutex<Option<Box<dyn Fn(Arc<MockComponentRuntimeAPI>) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync + 'static>>>>,
    error_handler: Arc<Mutex<Option<Box<dyn Fn(Arc<MockComponentRuntimeAPI>) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync + 'static>>>>,
}

impl MockComponentExecutor {
    fn new() -> Self {
        Self {
            fail_count: Arc::new(Mutex::new(0)),
            current_fail: Arc::new(Mutex::new(0)),
            success_handler: Arc::new(Mutex::new(None)),
            error_handler: Arc::new(Mutex::new(None)),
        }
    }
    
    fn expect(&self) -> Self {
        self.clone()
    }
    
    fn times(&self, count: usize) -> Self {
        let fail_count = self.fail_count.clone();
        tokio::spawn(async move {
            let mut fail_count_guard = fail_count.lock().await;
            *fail_count_guard = count;
        });
        self.clone()
    }
    
    fn returning<F>(&self, f: F) -> Self 
    where
        F: Fn(Arc<MockComponentRuntimeAPI>) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync + 'static,
    {
        let handler = Box::new(f);
        let fail_count = self.fail_count.clone();
        let error_handler = self.error_handler.clone();
        let success_handler = self.success_handler.clone();
        
        tokio::spawn(async move {
            let fail_count_value = *fail_count.lock().await;
            
            if fail_count_value > 0 {
                // This is an error handler
                let mut error_handler_guard = error_handler.lock().await;
                *error_handler_guard = Some(handler);
            } else {
                // This is a success handler
                let mut success_handler_guard = success_handler.lock().await;
                *success_handler_guard = Some(handler);
            }
        });
        
        self.clone()
    }
    
    async fn execute(&self, runtime_api: Arc<MockComponentRuntimeAPI>) -> Result<(), String> {
        let current_fail = {
            let mut current = self.current_fail.lock().await;
            *current += 1;
            *current
        };
        
        let fail_count = *self.fail_count.lock().await;
        
        if current_fail <= fail_count {
            // Execute failure handler
            if let Some(handler) = self.error_handler.lock().await.as_ref() {
                return handler(runtime_api).await;
            }
            return Err("No error handler defined".to_string());
        } else {
            // Execute success handler
            if let Some(handler) = self.success_handler.lock().await.as_ref() {
                return handler(runtime_api).await;
            }
            return Ok(());
        }
    }
}

// Advanced CircuitBreaker with StateStore (for Test 3)
struct CircuitBreaker2 {
    id: String,
    config: TestCircuitBreakerConfig,
    state_store: Arc<dyn StateStore>,
    state: Arc<Mutex<TestCircuitState>>,
}

impl CircuitBreaker2 {
    fn new(
        id: String,
        config: TestCircuitBreakerConfig,
        state_store: Arc<dyn StateStore>,
    ) -> Self {
        Self {
            id,
            config,
            state_store,
            state: Arc::new(Mutex::new(TestCircuitState::CLOSED)),
        }
    }

    async fn get_state(&self) -> Result<TestCircuitState> {
        let state = self.state.lock().await;
        Ok(state.clone())
    }

    async fn execute<F, T, E>(&self, operation: F) -> Result<T, E>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: From<anyhow::Error>,
    {
        let current_state = {
            let state = self.state.lock().await;
            state.clone()
        };

        if current_state == TestCircuitState::OPEN {
            return Err(anyhow::anyhow!("Circuit breaker {} is open", self.id).into());
        }

        match operation.await {
            Ok(result) => {
                // Success case
                let should_save = {
                    let mut state = self.state.lock().await;
                    let should_save = *state == TestCircuitState::HALF_OPEN;
                    if should_save {
                        // Reset to closed state if we were half-open
                        *state = TestCircuitState::CLOSED;
                    }
                    should_save
                };
                
                if should_save {
                    self.save_state("CLOSED", 0).await.ok();
                }
                Ok(result)
            },
            Err(error) => {
                // Handle failure
                let action = {
                    let state = self.state.lock().await;
                    if *state == TestCircuitState::CLOSED { "CLOSED" } 
                    else if *state == TestCircuitState::HALF_OPEN { "HALF_OPEN" } 
                    else { "NONE" }
                };

                if action == "CLOSED" {
                    // Potentially trip the circuit
                    let component_state = self.state_store.get_component_state(&self.id).await?
                        .unwrap_or(json!({"failure_count": 0}));
                    
                    let failure_count = component_state.get("failure_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32 + 1;
                    
                    if failure_count >= self.config.failure_threshold {
                        // Trip the circuit
                        {
                            let mut state = self.state.lock().await;
                            *state = TestCircuitState::OPEN;
                        }
                        
                        // Save state and schedule reset
                        self.save_state("OPEN", failure_count).await.ok();
                        self.schedule_reset();
                    } else {
                        // Just update failure count
                        self.save_failure_count(failure_count).await.ok();
                    }
                } else if action == "HALF_OPEN" {
                    // Back to open on failure in half-open
                    {
                        let mut state = self.state.lock().await;
                        *state = TestCircuitState::OPEN;
                    }
                    
                    // Save state and schedule reset
                    self.save_state("OPEN", 1).await.ok();
                    self.schedule_reset();
                }
                
                Err(error)
            }
        }
    }

    fn schedule_reset(&self) {
        let state_arc = self.state.clone();
        let id = self.id.clone();
        let state_store = self.state_store.clone();
        let timeout_ms = self.config.reset_timeout_ms;
        
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(timeout_ms)).await;
            
            // Transition to half-open
            let current_state = {
                let state = state_arc.lock().await;
                state.clone()
            };
            
            if current_state == TestCircuitState::OPEN {
                {
                    let mut state = state_arc.lock().await;
                    *state = TestCircuitState::HALF_OPEN;
                }
                
                // Save the new state
                if let Err(e) = state_store.set_component_state(&id, json!({
                    "state": "HALF_OPEN",
                    "failure_count": 0,
                    "half_open_calls": 0,
                })).await {
                    eprintln!("Failed to save circuit breaker state: {}", e);
                }
            }
        });
    }

    async fn save_state(&self, state_str: &str, failure_count: u32) -> Result<()> {
        let state_json = json!({
            "state": state_str,
            "failure_count": failure_count,
        });
        
        self.state_store.set_component_state(&self.id, state_json).await
    }

    async fn save_failure_count(&self, failure_count: u32) -> Result<()> {
        let existing = self.state_store.get_component_state(&self.id).await?
            .unwrap_or(json!({}));
        
        let mut obj = existing.as_object().unwrap_or(&serde_json::Map::new()).clone();
        obj.insert("failure_count".to_string(), json!(failure_count));
        
        self.state_store.set_component_state(&self.id, json!(obj)).await
    }
}

/// Mock implementation of StateStore for tests
pub struct MockRuntimeStateStore {
    runtime_api: Arc<dyn ComponentRuntimeAPI>,
}

impl MockRuntimeStateStore {
    pub fn new(runtime_api: Arc<dyn ComponentRuntimeAPI>) -> Self {
        Self { runtime_api }
    }
}

#[async_trait]
impl StateStore for MockRuntimeStateStore {
    async fn get_flow_instance_state(&self, flow_id: &str, instance_id: &str) -> Result<Option<serde_json::Value>> {
        let key = format!("flow:{}:instance:{}_resilience_state", flow_id, instance_id);
        match self.runtime_api.get_state(&key).await {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Failed to get flow instance state: {}", e)),
        }
    }

    async fn set_flow_instance_state(&self, flow_id: &str, instance_id: &str, state: serde_json::Value) -> Result<()> {
        let key = format!("flow:{}:instance:{}_resilience_state", flow_id, instance_id);
        match self.runtime_api.set_state(&key, state).await {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Failed to set flow instance state: {}", e)),
        }
    }
    
    async fn get_component_state(&self, component_id: &str) -> Result<Option<serde_json::Value>> {
        let key = format!("component:{}_resilience_state", component_id);
        match self.runtime_api.get_state(&key).await {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Failed to get component state: {}", e)),
        }
    }
    
    async fn set_component_state(&self, component_id: &str, state: serde_json::Value) -> Result<()> {
        let key = format!("component:{}_resilience_state", component_id);
        match self.runtime_api.set_state(&key, state).await {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Failed to set component state: {}", e)),
        }
    }
}

#[tokio::test]
async fn test_circuit_breaker_with_test_utils() {
    // Create a circuit breaker with our state store
    let config = TestCircuitBreakerConfig {
        failure_threshold: 2,
        reset_timeout_ms: 100,
        half_open_max_calls: 1,
    };
    
    let state_store = Arc::new(MemoryStateStore::new());
    let circuit_breaker = TestCircuitBreaker::new(
        2, // Failure threshold - should open after 2 failures
        100, // Reset timeout in ms
        Arc::new(SimpleTimerService::new()),
    );
    
    // Initial state should be CLOSED
    assert_eq!(circuit_breaker.get_state().await, TestCircuitState::CLOSED, "Initial state should be CLOSED");
    
    // Create a mock component that will fail
    let mock_component = MockComponentExecutor::new();
    
    // Configure mock to fail for all calls initially
    let runtime_api = create_mock_component_runtime_api("test-component");
    let runtime_arc = Arc::new(runtime_api);
    
    // Execute several calls and watch circuit breaker state
    for i in 1..=4 {
        println!("Executing call {} with circuit state: {}", i, circuit_breaker.get_state().await);
        
        // Use a closure to handle the result
        let result = circuit_breaker.execute(|| async {
            // Always return an error to trigger circuit breaker
            Err::<String, String>(format!("Simulated error on call {}", i))
        }).await;
        
        assert!(result.is_err(), "Call {} should fail", i);
        
        let expected_state = if i < 2 { 
            TestCircuitState::CLOSED 
        } else { 
            TestCircuitState::OPEN 
        };
        
        assert_eq!(
            circuit_breaker.get_state().await, 
            expected_state,
            "After {} failures, circuit should be {}", i, expected_state
        );
    }
    
    // Verify circuit metrics
    let metrics = circuit_breaker.get_metrics().await;
    assert_eq!(metrics.failure_count, 2, "Should record all 2 failures");
    
    // Now advance time to allow circuit half-open
    println!("Current state before advancing time: {}", circuit_breaker.get_state().await);
    let timer_service = SimpleTimerService::new();
    circuit_breaker.timer_service.advance_time(Duration::from_millis(200)).await.unwrap();
    
    // Check state has changed to half-open
    let current_state = circuit_breaker.get_state().await;
    println!("Current state after advancing time: {}", current_state);
    assert_eq!(current_state, TestCircuitState::HALF_OPEN, "Circuit should transition to HALF_OPEN after timeout");
    
    // Configure mock to succeed for the next call
    
    // Execute in half-open state (should succeed and close circuit)
    let success_result = circuit_breaker.execute(|| async {
        // Return success to close circuit
        Ok::<String, String>("Success!".to_string())
    }).await;
    
    assert!(success_result.is_ok(), "Call in HALF_OPEN state should succeed");
    
    // Circuit should now be CLOSED again
    assert_eq!(circuit_breaker.get_state().await, TestCircuitState::CLOSED, "Circuit should be CLOSED after successful call in HALF_OPEN state");
    
    // Check metrics
    let final_metrics = circuit_breaker.get_metrics().await;
    assert_eq!(final_metrics.success_count, 1, "Should record 1 success");
    assert_eq!(final_metrics.closed_count, 1, "Should have 1 transition to CLOSED (initial)");
    assert_eq!(final_metrics.open_count, 1, "Should have 1 transition to OPEN");
    assert_eq!(final_metrics.half_open_count, 1, "Should have 1 transition to HALF_OPEN");
}

#[tokio::test]
async fn test_bulkhead_pattern() {
    // Create a simple bulkhead (concurrency limiter)
    let max_concurrent = 2;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    
    let execute_count = Arc::new(Mutex::new(0));
    let max_seen_concurrent = Arc::new(Mutex::new(0));
    let current_concurrent = Arc::new(Mutex::new(0));
    
    // Function to execute with bulkhead protection
    async fn execute_with_bulkhead<F, Fut, T>(
        semaphore: Arc<tokio::sync::Semaphore>,
        execute_count: Arc<Mutex<usize>>,
        current_concurrent: Arc<Mutex<usize>>,
        max_seen_concurrent: Arc<Mutex<usize>>,
        operation: F,
    ) -> Result<T, String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        // Try to acquire a permit
        let permit = match semaphore.try_acquire() {
            Ok(permit) => permit,
            Err(_) => return Err("Bulkhead full".to_string()),
        };
        
        // Track concurrency
        {
            let mut count = execute_count.lock().await;
            *count += 1;
            let mut current = current_concurrent.lock().await;
            *current += 1;
            
            let mut max = max_seen_concurrent.lock().await;
            if *current > *max {
                *max = *current;
            }
        }
        
        // Execute the operation
        let result = operation().await;
        
        // Reduce concurrency count
        {
            let mut current = current_concurrent.lock().await;
            *current -= 1;
        }
        
        // Permit will be released when dropped
        drop(permit);
        
        result
    }
    
    // Create a series of tasks
    let mut handles = vec![];
    for i in 0..5 {
        let semaphore_clone = semaphore.clone();
        let execute_count_clone = execute_count.clone();
        let current_concurrent_clone = current_concurrent.clone();
        let max_seen_concurrent_clone = max_seen_concurrent.clone();
        
        let handle = tokio::spawn(async move {
            let result = execute_with_bulkhead(
                semaphore_clone,
                execute_count_clone,
                current_concurrent_clone,
                max_seen_concurrent_clone,
                || async move {
                    // Simulate some work
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok(format!("Task {} completed", i))
                },
            ).await;
            
            (i, result)
        });
        
        handles.push(handle);
        
        // Small delay to stagger the tasks
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    
    // Collect results
    let mut results = vec![];
    for handle in handles {
        let result = timeout(Duration::from_secs(1), handle).await
            .expect("Task timed out")
            .expect("Task panicked");
        results.push(result);
    }
    
    // Check metrics
    let total_execute_count = *execute_count.lock().await;
    let max_concurrent_seen = *max_seen_concurrent.lock().await;
    
    // Verify max concurrency was respected
    assert!(max_concurrent_seen <= max_concurrent, 
        "Max concurrency exceeded: saw {}, limit was {}", 
        max_concurrent_seen, max_concurrent);
    
    // Verify not all tasks were able to execute
    assert!(total_execute_count < 5, 
        "Expected some tasks to be rejected, but all {} executed", 
        total_execute_count);
    
    // Check individual task results
    let mut success_count = 0;
    let mut rejection_count = 0;
    
    for (task_id, result) in results {
        match result {
            Ok(_) => success_count += 1,
            Err(e) => {
                assert_eq!(e, "Bulkhead full".to_string(), 
                    "Task {} failed with unexpected error: {}", task_id, e);
                rejection_count += 1;
            }
        }
    }
    
    assert_eq!(success_count + rejection_count, 5, 
        "All tasks should either succeed or be rejected");
    assert_eq!(success_count, total_execute_count, 
        "Execute count should match successful tasks");
}

#[tokio::test]
async fn test_circuit_breaker_integration() -> Result<()> {
    println!("Starting circuit breaker integration test");

    // Create a custom mock runtime implementation
    struct TestMockRuntime {
        state: Mutex<Value>,
    }

    impl TestMockRuntime {
        fn new() -> Self {
            Self {
                state: Mutex::new(json!({})),
            }
        }
    }

    #[async_trait]
    impl ComponentRuntimeAPI for TestMockRuntime {
        async fn log(&self, level: LogLevel, message: &str) -> Result<(), ComponentError> {
            println!("[{:?}] {}", level, message);
            Ok(())
        }

        async fn get_input(&self, _name: &str) -> Result<Option<DataPacket>, ComponentError> {
            Ok(None)
        }

        async fn set_output(&self, _name: &str, _data: DataPacket) -> Result<(), ComponentError> {
            Ok(())
        }

        async fn get_state(&self, key: &str) -> Result<Option<Value>, ComponentError> {
            println!("Getting state for key: {}", key);
            let state = self.state.lock().await;
            if let Some(val) = state.get(key) {
                Ok(Some(val.clone()))
            } else {
                Ok(None)
            }
        }

        async fn set_state(&self, key: &str, value: Value) -> Result<(), ComponentError> {
            println!("Setting state for key: {} to {:?}", key, value);
            let mut state = self.state.lock().await;
            *state = json!({
                key: value
            });
            Ok(())
        }

        fn name(&self) -> &str {
            "test-runtime"
        }
    }

    // Create our test mock runtime
    let runtime = Arc::new(TestMockRuntime::new());
    
    // Create a MockRuntimeStateStore adapter
    let state_store = Arc::new(MockRuntimeStateStore::new(runtime.clone()));
    
    // Create a circuit breaker with our state store
    let config = TestCircuitBreakerConfig {
        failure_threshold: 2,
        reset_timeout_ms: 100,
        half_open_max_calls: 1,
    };
    
    let circuit_breaker = CircuitBreaker2::new(
        "test-circuit".to_string(),
        config,
        state_store,
    );
    
    // Try a successful operation
    let result = circuit_breaker.execute(async {
        println!("Executing successful operation");
        Ok::<_, anyhow::Error>(json!({"success": true}))
    }).await;
    
    assert!(result.is_ok(), "First operation should succeed");
    
    // Fail twice to trip the circuit
    for i in 1..=2 {
        let result: Result<Value, anyhow::Error> = circuit_breaker.execute(async {
            println!("Executing failure {}", i);
            Err(anyhow::anyhow!("Test failure"))
        }).await;
        
        assert!(result.is_err(), "Operation {} should fail", i);
    }
    
    // Circuit should now be open, attempt should fail immediately
    let result: Result<Value, anyhow::Error> = circuit_breaker.execute(async {
        panic!("This code should not be executed when circuit is open");
        #[allow(unreachable_code)]
        Ok(json!({}))
    }).await;
    
    assert!(result.is_err(), "Circuit should be open and fail fast");
    
    // Wait for the circuit to transition to half-open
    println!("Waiting for circuit timeout...");
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    
    // Try a successful call in half-open state
    let result: Result<Value, anyhow::Error> = circuit_breaker.execute(async {
        println!("Executing successful operation in half-open state");
        Ok(json!({"success": true}))
    }).await;
    
    assert!(result.is_ok(), "Call in half-open state should succeed");
    
    // Circuit should now be closed again
    let state = circuit_breaker.get_state().await?;
    println!("Final circuit state: {:?}", state);
    
    Ok(())
} 
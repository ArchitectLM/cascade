//! Example tests demonstrating how to use cascade-test-utils with cascade-monitoring

use cascade_test_utils::{
    data_generators::create_http_flow_dsl,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Helper types for HTTP response simulation
#[derive(Clone)]
struct HttpResponseData {
    status_code: u16,
    body: serde_json::Value,
}

impl HttpResponseData {
    fn new(status_code: u16, body: serde_json::Value) -> Self {
        Self {
            status_code,
            body,
        }
    }
}

/// Mock runtime API for component interactions
#[derive(Default)]
struct MockComponentRuntimeAPI {
    outputs: Mutex<HashMap<String, serde_json::Value>>,
}

impl MockComponentRuntimeAPI {
    fn new() -> Self {
        Self {
            outputs: Mutex::new(HashMap::new()),
        }
    }
    
    async fn set_output(&self, key: &str, value: serde_json::Value) -> Result<(), String> {
        self.outputs.lock().unwrap().insert(key.to_string(), value);
        Ok(())
    }
    
    async fn set_error(&self, code: &str, details: serde_json::Value) -> Result<(), String> {
        self.outputs.lock().unwrap().insert("error".to_string(), json!({
            "code": code,
            "details": details,
        }));
        Ok(())
    }
}

/// Simulate HTTP response helper
async fn simulate_http_response(runtime_api: Arc<MockComponentRuntimeAPI>, data: HttpResponseData) -> Result<(), String> {
    runtime_api.set_output("response", json!({
        "statusCode": data.status_code,
        "body": data.body,
    })).await?;
    
    if data.status_code >= 400 {
        return Err(format!("HTTP error: {}", data.status_code));
    }
    
    Ok(())
}

/// Mock Component Executor for testing
#[derive(Default)]
struct MockComponentExecutor {
    handlers: Mutex<Vec<Box<dyn Fn(Arc<MockComponentRuntimeAPI>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>> + Send + Sync>>>,
}

impl MockComponentExecutor {
    fn expect_execute(&mut self) -> MockExpectation {
        MockExpectation {
            executor: self,
        }
    }
    
    async fn execute(&self, runtime_api: Arc<MockComponentRuntimeAPI>) -> Result<(), String> {
        let handlers = self.handlers.lock().unwrap();
        if let Some(handler) = handlers.last() {
            handler(runtime_api).await
        } else {
            Err("No handler registered".to_string())
        }
    }
}

struct MockExpectation<'a> {
    executor: &'a MockComponentExecutor,
}

impl<'a> MockExpectation<'a> {
    fn times(&self, _count: usize) -> &Self {
        // In a real mock this would track the expected call count
        self
    }
    
    fn returning<F>(&self, f: F) -> &Self 
    where
        F: Fn(Arc<MockComponentRuntimeAPI>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>> + Send + Sync + 'static,
    {
        self.executor.handlers.lock().unwrap().push(Box::new(f));
        self
    }
}

/// Extended TestServerHandles for our test
struct TestServerHandles {
    base_url: String,
    client: Client,
    core_runtime: Arc<MockCoreRuntime>,
    component_executor: MockComponentExecutor,
}

/// Mock client for HTTP requests
#[derive(Clone)]
struct Client {}

impl Client {
    fn new() -> Self {
        Self {}
    }
}

/// Mock Core Runtime API
struct MockCoreRuntime {
    flow_states: Mutex<HashMap<String, serde_json::Value>>,
}

impl MockCoreRuntime {
    fn new() -> Self {
        Self {
            flow_states: Mutex::new(HashMap::new()),
        }
    }
    
    async fn deploy_dsl(&self, flow_id: &str, _flow_dsl: &str) -> Result<(), String> {
        // In a real implementation, this would actually deploy the flow
        println!("Mock: Deployed flow {}", flow_id);
        Ok(())
    }
    
    async fn trigger_flow(&self, flow_id: &str, _input: serde_json::Value) -> Result<String, String> {
        // Generate a predictable instance ID
        let instance_id = format!("{}-instance-123", flow_id);
        
        // Store an initial state for the flow
        self.flow_states.lock().unwrap().insert(instance_id.clone(), json!({
            "flow_id": flow_id,
            "state": "RUNNING",
            "created_at": "2023-01-01T12:00:00Z",
        }));
        
        Ok(instance_id)
    }
    
    async fn get_flow_state(&self, instance_id: &str) -> Result<Option<serde_json::Value>, String> {
        // Create a completed state for any instance ID
        let states = self.flow_states.lock().unwrap();
        if let Some(state) = states.get(instance_id) {
            let mut state = state.clone();
            // Automatically update to COMPLETED for tests
            state["state"] = json!("COMPLETED");
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }
}

/// Extended TestServerBuilder for our test
#[derive(Default)]
struct MockTestServerBuilder {
    component_executor: Option<MockComponentExecutor>,
    core_runtime: Option<Arc<MockCoreRuntime>>,
}

impl MockTestServerBuilder {
    fn new() -> Self {
        Self::default()
    }
    
    async fn build(self) -> Result<TestServerHandles, String> {
        let core_runtime = match self.core_runtime {
            Some(cr) => cr,
            None => Arc::new(MockCoreRuntime::new())
        };
        let component_executor = self.component_executor.unwrap_or_default();
        
        Ok(TestServerHandles {
            base_url: "http://localhost:8080".to_string(),
            client: Client::new(),
            core_runtime,
            component_executor,
        })
    }
}

/// Mock the original TestServerBuilder with our own implementation
struct TestServerBuilderExt;

impl TestServerBuilderExt {
    fn new() -> MockTestServerBuilder {
        MockTestServerBuilder::new()
    }
}

/// Mock metric collector for testing
#[derive(Default)]
struct MockMetricCollector {
    metrics: Mutex<Vec<(String, f64, HashMap<String, String>)>>,
}

impl MockMetricCollector {
    fn new() -> Self {
        Self {
            metrics: Mutex::new(Vec::new()),
        }
    }

    fn record_metric(&self, name: String, value: f64, labels: HashMap<String, String>) {
        self.metrics.lock().unwrap().push((name, value, labels));
    }

    fn get_metrics(&self) -> Vec<(String, f64, HashMap<String, String>)> {
        self.metrics.lock().unwrap().clone()
    }

    fn clear(&self) {
        self.metrics.lock().unwrap().clear();
    }
}

/// Simple flow monitor that records metrics
struct FlowMonitor {
    metric_collector: Arc<MockMetricCollector>,
}

impl FlowMonitor {
    fn new(collector: Arc<MockMetricCollector>) -> Self {
        Self {
            metric_collector: collector,
        }
    }

    async fn record_flow_started(&self, flow_id: &str, instance_id: &str) {
        let mut labels = HashMap::new();
        labels.insert("flow_id".to_string(), flow_id.to_string());
        labels.insert("instance_id".to_string(), instance_id.to_string());
        
        self.metric_collector.record_metric(
            "flow_started".to_string(),
            1.0,
            labels,
        );
    }

    async fn record_flow_completed(&self, flow_id: &str, instance_id: &str, duration_ms: u64) {
        let mut labels = HashMap::new();
        labels.insert("flow_id".to_string(), flow_id.to_string());
        labels.insert("instance_id".to_string(), instance_id.to_string());
        
        self.metric_collector.record_metric(
            "flow_completed".to_string(),
            1.0,
            labels.clone(),
        );
        
        self.metric_collector.record_metric(
            "flow_duration_ms".to_string(),
            duration_ms as f64,
            labels,
        );
    }

    async fn record_step_execution(&self, flow_id: &str, instance_id: &str, step_id: &str, duration_ms: u64) {
        let mut labels = HashMap::new();
        labels.insert("flow_id".to_string(), flow_id.to_string());
        labels.insert("instance_id".to_string(), instance_id.to_string());
        labels.insert("step_id".to_string(), step_id.to_string());
        
        self.metric_collector.record_metric(
            "step_executed".to_string(),
            1.0,
            labels.clone(),
        );
        
        self.metric_collector.record_metric(
            "step_duration_ms".to_string(),
            duration_ms as f64,
            labels,
        );
    }
}

#[tokio::test]
async fn test_monitoring_flow_execution() {
    // Create metric collector
    let metric_collector = Arc::new(MockMetricCollector::new());
    let flow_monitor = FlowMonitor::new(metric_collector.clone());
    
    // Create a TestServer
    let server_builder = MockTestServerBuilder::new();
    let mut component_executor = MockComponentExecutor::default();
    
    // Configure component execution behavior with primitive values
    component_executor.expect_execute()
        .returning(move |runtime_api| {
            // Use primitive values that can be easily captured by closure
            let status: u16 = 200;
            let body = json!({
                "result": "success",
                "data": ["item1", "item2"]
            });
            
            Box::pin(async move {
                let response = HttpResponseData::new(status, body);
                simulate_http_response(runtime_api, response).await
            })
        });
    
    // Build the server with our configured components
    let server = server_builder.build().await.unwrap();
    
    // Set up the flow
    let flow_id = "monitored-flow";
    let flow_dsl = create_http_flow_dsl(
        flow_id,
        "https://example.com/api",
        "GET"
    );
    
    // Deploy the flow
    server.core_runtime.deploy_dsl(flow_id, &flow_dsl).await.unwrap();
    
    // Record flow start
    let instance_id = "test-instance-123";
    flow_monitor.record_flow_started(flow_id, instance_id).await;
    
    // Track execution start time
    let start_time = std::time::Instant::now();
    
    // Trigger the flow
    let actual_instance_id = server.core_runtime
        .trigger_flow(flow_id, json!({}))
        .await
        .unwrap();
    
    // Wait for flow to complete and get final state
    let _state = server.core_runtime
        .get_flow_state(&actual_instance_id)
        .await
        .unwrap()
        .unwrap();
    
    // Record step execution
    flow_monitor.record_step_execution(
        flow_id, 
        &actual_instance_id, 
        "http-step",
        50, // Simulate 50ms execution time
    ).await;
    
    // Calculate execution duration and record flow completion
    let duration_ms = start_time.elapsed().as_millis() as u64;
    flow_monitor.record_flow_completed(flow_id, &actual_instance_id, duration_ms).await;
    
    // Verify metrics were recorded
    let metrics = metric_collector.get_metrics();
    
    // Check flow start metric
    let flow_started_metric = metrics.iter().find(|(name, _, _)| name == "flow_started");
    assert!(flow_started_metric.is_some(), "Flow start metric should be recorded");
    
    // Check step execution metric
    let step_executed_metric = metrics.iter().find(|(name, _, labels)| 
        name == "step_executed" && labels.get("step_id") == Some(&"http-step".to_string())
    );
    assert!(step_executed_metric.is_some(), "Step execution metric should be recorded");
    
    // Check flow completion metric
    let flow_completed_metric = metrics.iter().find(|(name, _, _)| name == "flow_completed");
    assert!(flow_completed_metric.is_some(), "Flow completion metric should be recorded");
    
    // Check duration metrics
    let flow_duration_metric = metrics.iter().find(|(name, _, _)| name == "flow_duration_ms");
    assert!(flow_duration_metric.is_some(), "Flow duration metric should be recorded");
    
    let (_, duration_value, _) = flow_duration_metric.unwrap();
    assert!(*duration_value >= 0.0, "Duration should be non-negative");
}

#[tokio::test]
async fn test_monitoring_with_multiple_flows() {
    // Create metric collector
    let metric_collector = Arc::new(MockMetricCollector::new());
    let flow_monitor = FlowMonitor::new(metric_collector.clone());
    
    // Create a TestServer
    let server_builder = MockTestServerBuilder::new();
    let mut component_executor = MockComponentExecutor::default();
    
    // Configure component execution behavior
    component_executor.expect_execute()
        .returning(move |runtime_api| {
            // Use primitive values that are easily captured
            let status: u16 = 200;
            let body = json!({ "result": "success" });
            
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                let response = HttpResponseData::new(status, body);
                simulate_http_response(runtime_api, response).await
            })
        });
    
    // Build the server with our configured components
    let server = server_builder.build().await.unwrap();
    
    // Create and deploy multiple flows
    let flow_ids = vec!["flow-1", "flow-2", "flow-3"];
    for flow_id in &flow_ids {
        let flow_dsl = create_http_flow_dsl(
            flow_id,
            "https://example.com/api",
            "GET"
        );
        server.core_runtime.deploy_dsl(flow_id, &flow_dsl).await.unwrap();
    }
    
    // Execute flows and record metrics
    for flow_id in &flow_ids {
        // Record flow started
        let instance_id = format!("{}-instance", flow_id);
        flow_monitor.record_flow_started(flow_id, &instance_id).await;
        
        // Trigger flow
        server.core_runtime.trigger_flow(flow_id, json!({})).await.unwrap();
        
        // Record step execution (simulated)
        flow_monitor.record_step_execution(
            flow_id, 
            &instance_id, 
            "http-step",
            25, // Simulate 25ms execution time
        ).await;
        
        // Record flow completion (simulated)
        flow_monitor.record_flow_completed(flow_id, &instance_id, 100).await;
    }
    
    // Verify metrics
    let metrics = metric_collector.get_metrics();
    
    // Count metrics by type
    let flow_start_count = metrics.iter()
        .filter(|(name, _, _)| name == "flow_started")
        .count();
    
    let step_execution_count = metrics.iter()
        .filter(|(name, _, _)| name == "step_executed")
        .count();
    
    let flow_completion_count = metrics.iter()
        .filter(|(name, _, _)| name == "flow_completed")
        .count();
    
    // Verify we have metrics for each flow
    assert_eq!(flow_start_count, flow_ids.len(), "Should have one flow start metric per flow");
    assert_eq!(step_execution_count, flow_ids.len(), "Should have one step execution metric per flow");
    assert_eq!(flow_completion_count, flow_ids.len(), "Should have one flow completion metric per flow");
    
    // Verify metric labels
    for flow_id in &flow_ids {
        let flow_id_str = flow_id.to_string();
        let has_metrics_for_flow = metrics.iter().any(|(_, _, labels)| 
            labels.get("flow_id") == Some(&flow_id_str)
        );
        
        assert!(has_metrics_for_flow, "Should have metrics for flow {}", flow_id);
    }
}

#[tokio::test]
async fn test_monitoring_error_scenarios() {
    // Create metric collector
    let metric_collector = Arc::new(MockMetricCollector::new());
    let flow_monitor = FlowMonitor::new(metric_collector.clone());
    
    // Create a TestServer
    let server_builder = MockTestServerBuilder::new();
    let mut component_executor = MockComponentExecutor::default();
    
    // Configure component to return an error
    component_executor.expect_execute()
        .returning(|runtime_api| {
            Box::pin(async move {
                // Simulate an error
                runtime_api.set_error(
                    "http_error",
                    json!({"code": 500, "message": "Internal Server Error"})
                ).await.unwrap();
                
                Err("Component execution failed".into())
            })
        });
    
    // Build the server with our configured components
    let server = server_builder.build().await.unwrap();
    
    // Set up a flow that will fail
    let flow_id = "error-flow";
    let flow_dsl = create_http_flow_dsl(
        flow_id,
        "https://example.com/api",
        "GET"
    );
    
    server.core_runtime.deploy_dsl(flow_id, &flow_dsl).await.unwrap();
    
    // Clean existing metrics
    metric_collector.clear();
    
    // Record flow start
    let instance_id = "error-instance-123";
    flow_monitor.record_flow_started(flow_id, instance_id).await;
    
    // Custom error metric
    let mut error_labels = HashMap::new();
    error_labels.insert("flow_id".to_string(), flow_id.to_string());
    error_labels.insert("instance_id".to_string(), instance_id.to_string());
    error_labels.insert("error_type".to_string(), "component_error".to_string());
    
    metric_collector.record_metric(
        "flow_error".to_string(),
        1.0,
        error_labels,
    );
    
    // Trigger the flow
    let actual_instance_id = server.core_runtime
        .trigger_flow(flow_id, json!({}))
        .await
        .unwrap();
    
    // Get final state
    let _state = server.core_runtime
        .get_flow_state(&actual_instance_id)
        .await
        .unwrap()
        .unwrap();
    
    // Record flow completion regardless of outcome
    flow_monitor.record_flow_completed(flow_id, &actual_instance_id, 75).await;
    
    // Verify metrics
    let metrics = metric_collector.get_metrics();
    
    // Check for error metric
    let error_metric = metrics.iter().find(|(name, _, _)| name == "flow_error");
    assert!(error_metric.is_some(), "Flow error metric should be recorded");
    
    let (_, error_count, error_labels) = error_metric.unwrap();
    assert_eq!(*error_count, 1.0, "Error count should be 1");
    assert_eq!(error_labels.get("error_type").unwrap(), "component_error", 
        "Error type should be recorded correctly");
    
    // Check for both start and completion metrics
    let has_start = metrics.iter().any(|(name, _, labels)| 
        name == "flow_started" && labels.get("flow_id") == Some(&flow_id.to_string())
    );
    
    let has_completion = metrics.iter().any(|(name, _, labels)| 
        name == "flow_completed" && labels.get("flow_id") == Some(&flow_id.to_string())
    );
    
    assert!(has_start, "Flow start should be recorded even for flows that error");
    assert!(has_completion, "Flow completion should be recorded even for flows that error");
} 
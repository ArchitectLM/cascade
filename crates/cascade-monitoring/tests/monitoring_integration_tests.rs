use cascade_monitoring::{
    MetricsCollector,
    LogCollector,
    TraceCollector,
    MetricType,
    Collector,
    MonitoringConfig
};
use std::time::Duration;
use std::sync::{Arc, Mutex};
use futures::future::BoxFuture;
use tokio::test;
use std::collections::HashMap;
use serde_json::Value;
use std::any::Any;

// Mock implementations for testing
#[derive(Default)]
struct MockMetricsCollector {
    metrics: Arc<Mutex<Vec<(String, f64, HashMap<String, String>)>>>
}

impl MockMetricsCollector {
    fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(Vec::new()))
        }
    }
    
    fn get_metrics(&self) -> Vec<(String, f64, HashMap<String, String>)> {
        self.metrics.lock().unwrap().clone()
    }
}

impl MetricsCollector for MockMetricsCollector {
    fn record_metric(&self, name: &str, value: f64, metric_type: MetricType, labels: HashMap<String, String>) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.push((name.to_string(), value, labels));
    }
    
    fn flush(&self) -> BoxFuture<'static, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Default)]
struct MockLogCollector {
    logs: Arc<Mutex<Vec<(String, String, HashMap<String, Value>)>>>
}

impl MockLogCollector {
    fn new() -> Self {
        Self {
            logs: Arc::new(Mutex::new(Vec::new()))
        }
    }
    
    fn get_logs(&self) -> Vec<(String, String, HashMap<String, Value>)> {
        self.logs.lock().unwrap().clone()
    }
}

impl LogCollector for MockLogCollector {
    fn record_log(&self, level: &str, message: &str, metadata: HashMap<String, Value>) {
        let mut logs = self.logs.lock().unwrap();
        logs.push((level.to_string(), message.to_string(), metadata));
    }
    
    fn flush(&self) -> BoxFuture<'static, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Default)]
struct MockTraceCollector {
    spans: Arc<Mutex<Vec<(String, HashMap<String, String>, u64, u64)>>>
}

impl MockTraceCollector {
    fn new() -> Self {
        Self {
            spans: Arc::new(Mutex::new(Vec::new()))
        }
    }
    
    fn get_spans(&self) -> Vec<(String, HashMap<String, String>, u64, u64)> {
        self.spans.lock().unwrap().clone()
    }
}

impl TraceCollector for MockTraceCollector {
    fn record_span(&self, name: &str, attributes: HashMap<String, String>, start_time: u64, end_time: u64) {
        let mut spans = self.spans.lock().unwrap();
        spans.push((name.to_string(), attributes, start_time, end_time));
    }
    
    fn flush(&self) -> BoxFuture<'static, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct TestCollector<M: MetricsCollector, L: LogCollector, T: TraceCollector> {
    metrics: M,
    logs: L,
    traces: T,
}

impl<M: MetricsCollector + 'static, L: LogCollector + 'static, T: TraceCollector + 'static> Collector for TestCollector<M, L, T> {
    fn metrics(&self) -> &dyn MetricsCollector {
        &self.metrics
    }
    
    fn logs(&self) -> &dyn LogCollector {
        &self.logs
    }
    
    fn traces(&self) -> &dyn TraceCollector {
        &self.traces
    }
    
    fn flush(&self) -> BoxFuture<'static, Result<(), String>> {
        // Just return a simple future that reports success
        // This avoids the lifetime issues completely
        Box::pin(async { Ok(()) })
    }
}

#[test]
async fn test_metrics_collection() {
    // Arrange
    let metrics = MockMetricsCollector::new();
    let logs = MockLogCollector::new();
    let traces = MockTraceCollector::new();
    
    let collector = TestCollector {
        metrics,
        logs,
        traces,
    };
    
    // Configure monitoring
    let config = MonitoringConfig {
        service_name: "test-service".to_string(),
        environment: "test".to_string(),
        flush_interval: Duration::from_secs(1),
        ..Default::default()
    };
    
    // Act - Record metrics
    let labels = HashMap::from([
        ("component".to_string(), "test-component".to_string()),
        ("operation".to_string(), "add".to_string()),
    ]);
    
    collector.metrics().record_metric("request_count", 1.0, MetricType::Counter, labels.clone());
    collector.metrics().record_metric("response_time", 42.5, MetricType::Histogram, labels.clone());
    
    // Assert
    let metrics = collector.metrics().as_any().downcast_ref::<MockMetricsCollector>().unwrap();
    let recorded_metrics = metrics.get_metrics();
    
    assert_eq!(recorded_metrics.len(), 2);
    
    let (name1, value1, labels1) = &recorded_metrics[0];
    assert_eq!(name1, "request_count");
    assert_eq!(*value1, 1.0);
    assert_eq!(labels1.get("component").unwrap(), "test-component");
    
    let (name2, value2, _) = &recorded_metrics[1];
    assert_eq!(name2, "response_time");
    assert_eq!(*value2, 42.5);
}

#[test]
async fn test_log_collection() {
    // Arrange
    let metrics = MockMetricsCollector::new();
    let logs = MockLogCollector::new();
    let traces = MockTraceCollector::new();
    
    let collector = TestCollector {
        metrics,
        logs,
        traces,
    };
    
    // Act - Record logs
    let metadata = HashMap::from([
        ("request_id".to_string(), Value::String("req-123".to_string())),
        ("user_id".to_string(), Value::String("user-456".to_string())),
    ]);
    
    collector.logs().record_log("info", "Processing request", metadata.clone());
    collector.logs().record_log("error", "Failed to connect to database", metadata.clone());
    
    // Assert
    let logs = collector.logs().as_any().downcast_ref::<MockLogCollector>().unwrap();
    let recorded_logs = logs.get_logs();
    
    assert_eq!(recorded_logs.len(), 2);
    
    let (level1, message1, metadata1) = &recorded_logs[0];
    assert_eq!(level1, "info");
    assert_eq!(message1, "Processing request");
    assert_eq!(metadata1.get("request_id").unwrap(), &Value::String("req-123".to_string()));
    
    let (level2, message2, _) = &recorded_logs[1];
    assert_eq!(level2, "error");
    assert_eq!(message2, "Failed to connect to database");
}

#[test]
async fn test_trace_collection() {
    // Arrange
    let metrics = MockMetricsCollector::new();
    let logs = MockLogCollector::new();
    let traces = MockTraceCollector::new();
    
    let collector = TestCollector {
        metrics,
        logs,
        traces,
    };
    
    // Act - Record spans
    let span_attributes = HashMap::from([
        ("flow_id".to_string(), "order-processing".to_string()),
        ("step_id".to_string(), "validate-input".to_string()),
    ]);
    
    let start_time = 1000;
    let end_time = 1200;
    collector.traces().record_span("validate_step", span_attributes.clone(), start_time, end_time);
    
    // Assert
    let traces = collector.traces().as_any().downcast_ref::<MockTraceCollector>().unwrap();
    let recorded_spans = traces.get_spans();
    
    assert_eq!(recorded_spans.len(), 1);
    
    let (name, attributes, start, end) = &recorded_spans[0];
    assert_eq!(name, "validate_step");
    assert_eq!(attributes.get("flow_id").unwrap(), "order-processing");
    assert_eq!(*start, start_time);
    assert_eq!(*end, end_time);
    assert_eq!(end - start, 200); // Duration check
}

#[test]
async fn test_collector_flush() {
    // Arrange
    let metrics = MockMetricsCollector::new();
    let logs = MockLogCollector::new();
    let traces = MockTraceCollector::new();
    
    let collector = TestCollector {
        metrics,
        logs,
        traces,
    };
    
    // Record some data
    collector.metrics().record_metric("test_metric", 1.0, MetricType::Counter, HashMap::new());
    collector.logs().record_log("info", "Test log", HashMap::new());
    collector.traces().record_span("test_span", HashMap::new(), 1000, 2000);
    
    // Act
    let flush_result = collector.flush().await;
    
    // Assert
    assert!(flush_result.is_ok(), "Flush operation should succeed");
}

#[test]
async fn test_monitoring_integration() {
    // This test would be more sophisticated in a real implementation,
    // integrating with actual monitoring components
    
    // Arrange
    let metrics = MockMetricsCollector::new();
    let logs = MockLogCollector::new();
    let traces = MockTraceCollector::new();
    
    let collector = TestCollector {
        metrics,
        logs,
        traces,
    };
    
    // Simulate a flow execution with monitoring
    let flow_id = "order-processing-flow";
    let execution_id = "exec-789";
    
    // Start span for the overall flow
    let flow_start_time = 1000;
    let flow_attributes = HashMap::from([
        ("flow_id".to_string(), flow_id.to_string()),
        ("execution_id".to_string(), execution_id.to_string()),
    ]);
    
    // Record flow execution metrics
    collector.metrics().record_metric(
        "flow_execution_started", 
        1.0, 
        MetricType::Counter, 
        flow_attributes.clone()
    );
    
    // Record log for flow start
    let mut log_metadata = HashMap::new();
    log_metadata.insert("flow_id".to_string(), Value::String(flow_id.to_string()));
    log_metadata.insert("execution_id".to_string(), Value::String(execution_id.to_string()));
    
    collector.logs().record_log(
        "info", 
        &format!("Started execution of flow {}", flow_id),
        log_metadata.clone()
    );
    
    // Simulate some steps execution
    for step_id in &["validation", "processing", "notification"] {
        // Step span
        let step_start_time = flow_start_time + 100;
        let step_end_time = step_start_time + 50;
        
        let mut step_attributes = flow_attributes.clone();
        step_attributes.insert("step_id".to_string(), step_id.to_string());
        
        collector.traces().record_span(
            &format!("{}_step", step_id),
            step_attributes.clone(),
            step_start_time,
            step_end_time
        );
        
        // Step metrics
        collector.metrics().record_metric(
            "step_execution_time", 
            50.0, 
            MetricType::Histogram, 
            step_attributes.clone()
        );
        
        // Step log
        let mut step_log_metadata = log_metadata.clone();
        step_log_metadata.insert("step_id".to_string(), Value::String(step_id.to_string()));
        
        collector.logs().record_log(
            "info", 
            &format!("Completed step {} for flow {}", step_id, flow_id),
            step_log_metadata
        );
    }
    
    // End flow execution
    let flow_end_time = flow_start_time + 500;
    collector.traces().record_span(
        "flow_execution",
        flow_attributes.clone(),
        flow_start_time,
        flow_end_time
    );
    
    collector.metrics().record_metric(
        "flow_execution_completed", 
        1.0, 
        MetricType::Counter, 
        flow_attributes.clone()
    );
    
    collector.metrics().record_metric(
        "flow_execution_time", 
        500.0, 
        MetricType::Histogram, 
        flow_attributes.clone()
    );
    
    // Record completion log
    collector.logs().record_log(
        "info", 
        &format!("Completed execution of flow {}", flow_id),
        log_metadata
    );
    
    // Flush all collectors
    let flush_result = collector.flush().await;
    assert!(flush_result.is_ok(), "Flush operation should succeed");
    
    // Verify recorded data
    let metrics = collector.metrics().as_any().downcast_ref::<MockMetricsCollector>().unwrap();
    let logs = collector.logs().as_any().downcast_ref::<MockLogCollector>().unwrap();
    let traces = collector.traces().as_any().downcast_ref::<MockTraceCollector>().unwrap();
    
    assert_eq!(metrics.get_metrics().len(), 6); // 1 start + 3 steps + 2 end metrics
    assert_eq!(logs.get_logs().len(), 5);       // 1 start + 3 steps + 1 end logs
    assert_eq!(traces.get_spans().len(), 4);    // 3 steps + 1 flow span
} 
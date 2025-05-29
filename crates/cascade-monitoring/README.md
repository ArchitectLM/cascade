# Cascade Monitoring

A comprehensive monitoring solution for the Cascade platform that provides:

- **Distributed Tracing**: Using OpenTelemetry with Tempo/Jaeger backends
- **Metrics Collection**: Prometheus metrics for all components
- **Structured Logging**: JSON-formatted logs using tracing

## Features

- Complete OpenTelemetry integration for distributed tracing
- Prometheus metrics with predefined metrics for common operations
- Structured logging with file rotation and correlation IDs
- Middleware for HTTP request/response tracing and metrics
- Resilience pattern instrumentation (circuit breaker, bulkhead, etc.)

## Usage

### Basic Setup

```rust
use cascade_monitoring::{init, MonitoringConfig};

fn main() {
    // Create a configuration
    let config = MonitoringConfig {
        service_name: "cascade-server".to_string(),
        enable_tracing: true,
        otlp_endpoint: Some("http://tempo:4317".to_string()),
        enable_metrics: true,
        metrics_path: "/metrics".to_string(),
        enable_json_logging: true,
        log_file: Some("logs/cascade-server.log".to_string()),
        log_filter: "info,cascade=debug".to_string(),
        ..Default::default()
    };

    // Initialize monitoring
    init(config).expect("Failed to initialize monitoring");

    // The rest of your application...

    // Shutdown monitoring (typically in a shutdown handler)
    cascade_monitoring::shutdown();
}
```

### Integration with Axum

```rust
use axum::{
    routing::get,
    Router,
};
use cascade_monitoring::{
    metrics::axum_integration::{metrics_middleware, metrics_route},
    telemetry::axum_integration::trace_middleware,
    logging::axum_integration::correlation_id_middleware,
};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

async fn create_app() -> Router {
    // Create a router
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        // Add the metrics route
        .merge(metrics_route("/metrics"))
        // Add middleware for tracing, metrics, and correlation IDs
        .layer(
            ServiceBuilder::new()
                .layer(axum::middleware::from_fn(correlation_id_middleware))
                .layer(axum::middleware::from_fn(trace_middleware))
                .layer(axum::middleware::from_fn(metrics_middleware))
                .layer(TraceLayer::new_for_http())
        );

    app
}
```

### Recording Custom Metrics

```rust
use cascade_monitoring::metrics::{ServerMetrics, EdgeMetrics, ResilienceMetrics};

// Record flow deployment
ServerMetrics::record_flow_deployment(
    "flow-123",
    "edge",
    150.0, // duration in ms
    true,  // success
);

// Record edge-to-server communication
EdgeMetrics::record_server_communication(
    "/api/status",
    75.0,  // duration in ms
    true,  // success
);

// Record circuit breaker state change
ResilienceMetrics::record_circuit_breaker_state(
    "external-api",
    "open",
);
```

## Running the Monitoring Stack

A docker-compose file is provided in the `monitoring/docker` directory that includes:

- Prometheus for metrics collection
- Grafana for visualization
- Tempo for distributed tracing
- Loki for log aggregation

To start the stack:

```bash
cd monitoring/docker
docker-compose up -d
```

Then access:

- Prometheus: http://localhost:9090
- Grafana: http://localhost:3000 (admin/admin)
- Jaeger UI: http://localhost:16686

## Predefined Dashboards

Predefined Grafana dashboards are available for:

- Cascade Server overview
- Edge node performance
- Flow execution metrics
- Resilience patterns monitoring 
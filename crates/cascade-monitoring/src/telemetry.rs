//! Distributed tracing module using OpenTelemetry.
//!
//! This module handles initializing OpenTelemetry for distributed tracing and
//! integrating it with the tracing crate.

use anyhow::Context;
use once_cell::sync::OnceCell;
use opentelemetry::global;
use opentelemetry::sdk::propagation::TraceContextPropagator;
use opentelemetry::sdk::trace::{self, RandomIdGenerator, Sampler};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use std::time::Duration;
use tracing::info;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

use crate::MonitoringConfig;

/// Tracer name for OpenTelemetry
const TRACER_NAME: &str = "cascade-tracer";

/// Global trace provider for shutdown
static GLOBAL_TRACER_PROVIDER: OnceCell<trace::TracerProvider> = OnceCell::new();

/// Initialize OpenTelemetry tracing with the provided configuration
pub fn init_tracing(config: &MonitoringConfig) -> anyhow::Result<()> {
    info!("Initializing OpenTelemetry tracing");

    // Set up the global propagator for trace context
    global::set_text_map_propagator(TraceContextPropagator::new());

    // Prepare tracer options
    let mut tracer_options = trace::config()
        .with_sampler(Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .with_max_events_per_span(64)
        .with_resource(Resource::new(vec![
            SERVICE_NAME.string(config.service_name.clone()),
            SERVICE_VERSION.string(env!("CARGO_PKG_VERSION").to_string()),
        ]));

    // Create the tracer provider with appropriate exporter
    let tracer_provider = if let Some(otlp_endpoint) = &config.otlp_endpoint {
        info!("Using OTLP exporter at {}", otlp_endpoint);
        
        let otlp_exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(otlp_endpoint)
            .with_timeout(Duration::from_secs(3));
        
        let batch_config = opentelemetry::sdk::trace::BatchConfig::default()
            .with_max_export_batch_size(512)
            .with_scheduled_delay(Duration::from_secs(1))
            .with_max_concurrent_exports(4);
            
        tracer_options = tracer_options.with_batch_config(batch_config);
        
        opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(otlp_exporter)
            .with_trace_config(tracer_options)
            .install_batch(opentelemetry_sdk::runtime::Tokio)?
    } else {
        info!("Using Jaeger exporter with default configuration");
        
        // Without specific OTLP endpoint, use Jaeger
        opentelemetry_jaeger::new_agent_pipeline()
            .with_service_name(&config.service_name)
            .with_trace_config(tracer_options)
            .install_batch(opentelemetry_sdk::runtime::Tokio)?
    };

    // Store the tracer provider for later shutdown
    GLOBAL_TRACER_PROVIDER.set(tracer_provider).ok();

    // Create a tracer
    let tracer = global::tracer(TRACER_NAME);
    
    // Create and register an OpenTelemetry tracing layer
    let telemetry_layer = OpenTelemetryLayer::new(tracer);
    
    let subscriber = Registry::default().with(telemetry_layer);
    
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set global default subscriber")?;

    info!("OpenTelemetry tracing initialized successfully");
    Ok(())
}

/// Shut down the OpenTelemetry tracer provider
pub fn shutdown_tracer_provider() {
    if let Some(provider) = GLOBAL_TRACER_PROVIDER.get() {
        info!("Shutting down OpenTelemetry tracer provider");
        provider.shutdown();
    }
}

/// Helper function to create HTTP propagator
/// 
/// This function creates a propagator that can be used with HTTP requests
/// to propagate trace context.
pub fn get_http_propagator() -> impl opentelemetry::propagation::TextMapPropagator {
    TraceContextPropagator::new()
}

/// Creates a span ID extractor for HTTP requests
pub fn create_span_extractor() -> impl Fn(&hyper::Request<hyper::Body>) -> opentelemetry::Context {
    let propagator = get_http_propagator();
    move |request: &hyper::Request<hyper::Body>| {
        propagator.extract(&opentelemetry_http::HeaderExtractor(request.headers()))
    }
}

#[cfg(feature = "axum")]
pub mod axum_integration {
    use axum::{
        extract::MatchedPath,
        http::{Request, HeaderMap},
        middleware::Next,
        response::Response,
    };
    use opentelemetry::{
        global,
        trace::{Span, Status, TraceContextExt},
    };
    use std::time::Instant;
    use tracing::{Instrument, info_span};
    
    /// Axum middleware for OpenTelemetry tracing
    pub async fn trace_middleware<B>(req: Request<B>, next: Next<B>) -> Response {
        let path = if let Some(path) = req.extensions().get::<MatchedPath>() {
            path.as_str().to_owned()
        } else {
            req.uri().path().to_owned()
        };
        
        let method = req.method().clone();
        let headers = extract_headers(req.headers());
        
        let start = Instant::now();
        
        // Create a span for this request
        let span = info_span!(
            "http_request",
            otel.name = %format!("{} {}", method, path),
            http.method = %method,
            http.url = %path,
            http.client_ip = headers.get("client_ip").cloned().unwrap_or_default(),
            http.user_agent = headers.get("user_agent").cloned().unwrap_or_default(),
            otel.kind = %"server",
        );
        
        async move {
            // Process the request
            let response = next.run(req).await;
            
            // Record request duration
            let latency = start.elapsed();
            
            let status = response.status().as_u16();
            
            span.record("http.status_code", &status);
            span.record("http.response_time_ms", &latency.as_millis());
            
            if status >= 400 {
                span.record("error", &true);
                let status_code = format!("{}", status);
                current_span_set_status(Status::Error { description: status_code.into() });
            } else {
                current_span_set_status(Status::Ok);
            }
            
            response
        }
        .instrument(span)
        .await
    }
    
    /// Extract important headers for logging and tracing
    fn extract_headers(headers: &HeaderMap) -> std::collections::HashMap<&'static str, String> {
        let mut result = std::collections::HashMap::new();
        
        if let Some(user_agent) = headers.get("user-agent") {
            if let Ok(value) = user_agent.to_str() {
                result.insert("user_agent", value.to_string());
            }
        }
        
        if let Some(x_forwarded_for) = headers.get("x-forwarded-for") {
            if let Ok(value) = x_forwarded_for.to_str() {
                result.insert("client_ip", value.to_string());
            }
        } else if let Some(remote_addr) = headers.get("x-real-ip") {
            if let Ok(value) = remote_addr.to_str() {
                result.insert("client_ip", value.to_string());
            }
        }
        
        result
    }
    
    fn current_span_set_status(status: Status) {
        let current_context = global::get_text_map_propagator(|propagator| {
            propagator.extract(&opentelemetry::Context::current())
        });
        
        if let Some(span) = current_context.span() {
            span.set_status(status);
        }
    }
} 
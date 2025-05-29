use std::sync::Arc;
use axum::{
    Router,
    routing::get,
    http::{Request, StatusCode},
    response::Response,
    extract::State,
    body::Body,
};
use tower::{Service, ServiceExt};
use http_body_util::BodyExt;
use hyper::body::Bytes;

use crate::resilience::{RateLimiterConfig, CircuitBreakerConfig};
use crate::shared_state::SharedStateService;
use crate::state_inmemory::InMemorySharedStateService;

use super::factory::{MiddlewareFactory, MiddlewareConfig};
use super::rate_limiter::{RateLimiterMiddleware, RateLimiterLayer};
use super::circuit_breaker::{CircuitBreakerMiddleware, CircuitBreakerLayer};

// Simple handler that always succeeds
async fn success_handler() -> &'static str {
    "OK"
}

// Handler that always fails with 500
async fn failure_handler() -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from("Failed"))
        .unwrap()
}

#[tokio::test]
async fn test_rate_limiter_e2e() {
    // Create shared state
    let shared_state = Arc::new(InMemorySharedStateService::new());
    
    // Create rate limiter config with a small limit
    let rate_config = RateLimiterConfig {
        max_requests: 2,
        window_ms: 1000, // 1 second
        shared_state_scope: Some("test-rate-limit".to_string()),
    };
    
    // Create middleware factory
    let factory = MiddlewareFactory::new(Some(shared_state.clone()));
    
    // Create router with rate limiter middleware
    let app = Router::new()
        .route("/test", get(success_handler))
        .layer(RateLimiterLayer::new(rate_config.clone(), Some(shared_state.clone())));
    
    // Create test client
    let client = axum_test::TestClient::new(app);
    
    // First request should succeed
    let response = client.get("/test").send().await;
    assert_eq!(response.status(), StatusCode::OK);
    
    // Get the remaining count from headers
    let remaining = response.headers()
        .get("X-RateLimit-Remaining")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    
    assert_eq!(remaining, 1, "First request should leave 1 remaining");
    
    // Second request should succeed
    let response = client.get("/test").send().await;
    assert_eq!(response.status(), StatusCode::OK);
    
    // Get the remaining count from headers
    let remaining = response.headers()
        .get("X-RateLimit-Remaining")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    
    assert_eq!(remaining, 0, "Second request should leave 0 remaining");
    
    // Third request should be rate limited
    let response = client.get("/test").send().await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    
    // Wait for the rate limit window to expire
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    
    // Should be able to make a request again
    let response = client.get("/test").send().await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_circuit_breaker_e2e() {
    // Create shared state
    let shared_state = Arc::new(InMemorySharedStateService::new());
    
    // Create circuit breaker config
    let circuit_config = CircuitBreakerConfig {
        failure_threshold: 2,
        reset_timeout_ms: 1000, // 1 second
        shared_state_scope: Some("test-circuit".to_string()),
    };
    
    // Create middleware
    let middleware = CircuitBreakerMiddleware::new(
        circuit_config.clone(), 
        Some(shared_state.clone())
    );
    
    // Create paths for testing
    let success_path = "/success";
    let failure_path = "/fail";
    
    // Create router with circuit breaker middleware
    let app = Router::new()
        .route(success_path, get(success_handler))
        .route(failure_path, get(failure_handler))
        .layer(CircuitBreakerLayer::new(
            circuit_config.clone(), 
            Some(shared_state.clone())
        ));
    
    // Create test client
    let client = axum_test::TestClient::new(app);
    
    // Success path should work
    let response = client.get(success_path).send().await;
    assert_eq!(response.status(), StatusCode::OK);
    
    // Check circuit state from headers
    assert_eq!(
        response.headers().get("X-Circuit-State").unwrap().to_str().unwrap(),
        "CLOSED"
    );
    
    // First failure should just record a failure
    let response = client.get(failure_path).send().await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    
    // Second failure should open the circuit
    let response = client.get(failure_path).send().await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    
    // Next request to failure path should be blocked by circuit breaker
    let response = client.get(failure_path).send().await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    
    // Success path should also be blocked because they share the service name
    let response = client.get(success_path).send().await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    
    // Wait for the circuit to go half-open
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    
    // A success should close the circuit
    let response = client.get(success_path).send().await;
    assert_eq!(response.status(), StatusCode::OK);
    
    // Check circuit state - should be HALF_OPEN or CLOSED
    let circuit_state = response.headers().get("X-Circuit-State").unwrap().to_str().unwrap();
    assert!(
        circuit_state == "HALF_OPEN" || circuit_state == "CLOSED", 
        "Circuit should be in HALF_OPEN or CLOSED state, but was {}", 
        circuit_state
    );
    
    // Now further requests should succeed
    let response = client.get(success_path).send().await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_middleware_factory_e2e() {
    // Create shared state
    let shared_state = Arc::new(InMemorySharedStateService::new());
    
    // Create middleware factory
    let factory = MiddlewareFactory::new(Some(shared_state.clone()));
    
    // Create middleware config with both rate limiting and circuit breaking
    let config = MiddlewareConfig {
        enable_rate_limiting: true,
        enable_circuit_breaking: true,
        rate_limiter_config: Some(RateLimiterConfig {
            max_requests: 3,
            window_ms: 1000,
            shared_state_scope: Some("test-combined".to_string()),
        }),
        circuit_breaker_config: Some(CircuitBreakerConfig {
            failure_threshold: 2,
            reset_timeout_ms: 1000,
            shared_state_scope: Some("test-combined".to_string()),
        }),
    };
    
    // Create router with combined middleware
    let app = Router::new()
        .route("/success", get(success_handler))
        .route("/fail", get(failure_handler));
    
    // Apply middleware
    let app = factory.apply_middleware(app, config);
    
    // Create test client
    let client = axum_test::TestClient::new(app);
    
    // First success request should work
    let response = client.get("/success").send().await;
    assert_eq!(response.status(), StatusCode::OK);
    
    // Two failures should open the circuit
    let response = client.get("/fail").send().await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    
    let response = client.get("/fail").send().await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    
    // Circuit should be open now
    let response = client.get("/success").send().await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    
    // Wait for reset timeout
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    
    // Circuit should be half-open, and three requests should be allowed
    let response = client.get("/success").send().await;
    assert_eq!(response.status(), StatusCode::OK);
    
    // Check rate limit headers
    let remaining = response.headers()
        .get("X-RateLimit-Remaining")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    
    assert!(remaining > 0, "Rate limit should have remaining requests");
    
    // Should be able to make additional requests within rate limit
    let response = client.get("/success").send().await;
    assert_eq!(response.status(), StatusCode::OK);
    
    let response = client.get("/success").send().await;
    assert_eq!(response.status(), StatusCode::OK);
    
    // Fourth request should be rate limited
    let response = client.get("/success").send().await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
} 
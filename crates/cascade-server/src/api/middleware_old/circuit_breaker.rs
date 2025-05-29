use std::sync::Arc;
use std::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use futures::future::BoxFuture;
use hyper::body::Body;
use tower::{Layer, Service};

use crate::error::{ServerError, ServerResult};
use crate::resilience::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerKey, CircuitState};
use crate::shared_state::SharedStateService;

/// Circuit breaker middleware for API endpoints
pub struct CircuitBreakerMiddleware {
    circuit_breaker: Arc<CircuitBreaker>,
}

impl CircuitBreakerMiddleware {
    /// Create a new circuit breaker middleware
    pub fn new(config: CircuitBreakerConfig, shared_state: Option<Arc<dyn SharedStateService>>) -> Self {
        Self {
            circuit_breaker: Arc::new(CircuitBreaker::new(config, shared_state)),
        }
    }
    
    /// The middleware handler function
    pub async fn handle<B>(
        &self,
        req: Request<B>,
        next: Next<B>
    ) -> Result<Response, StatusCode> {
        // Extract the service name from the path
        let service = req.uri().path().split('/').take(2).collect::<Vec<_>>().join("/");
        
        // Extract the operation name if available
        let operation = req.uri().path().split('/').skip(2).take(1).next().map(String::from);
        
        // Create circuit breaker key
        let key = CircuitBreakerKey::new(service, operation);
        
        // Check if request is allowed by circuit breaker
        match self.circuit_breaker.allow(&key).await {
            Ok(state) => {
                // Request is allowed, proceed to next middleware or handler
                let response = next.run(req).await;
                
                // Record success if needed
                if state == CircuitState::HalfOpen {
                    // For half-open state, record success to close the circuit
                    if response.status().is_success() {
                        if let Err(e) = self.circuit_breaker.record_success(&key).await {
                            tracing::warn!("Failed to record circuit breaker success: {}", e);
                        }
                    } else {
                        // For a failure response, record failure to keep circuit open
                        if let Err(e) = self.circuit_breaker.report_failure(&key).await {
                            tracing::warn!("Failed to record circuit breaker failure: {}", e);
                        }
                    }
                }
                
                // Add circuit breaker headers to response
                if let Some(headers) = response.headers_mut() {
                    headers.insert(
                        "X-Circuit-State",
                        HeaderValue::from_str(&state.to_string()).unwrap()
                    );
                }
                
                Ok(response)
            },
            Err(err) => {
                if let ServerError::CircuitBreakerOpen { service, operation, failures, timeout_ms } = &err {
                    // Create a 503 Service Unavailable response
                    let mut response = Response::builder()
                        .status(StatusCode::SERVICE_UNAVAILABLE)
                        .body(Body::from(format!("{}", err)))
                        .unwrap();
                    
                    // Add circuit breaker headers
                    if let Some(headers) = response.headers_mut() {
                        headers.insert(
                            "X-Circuit-State",
                            HeaderValue::from_static("OPEN")
                        );
                        headers.insert(
                            "X-Circuit-Failures",
                            HeaderValue::from(*failures)
                        );
                        headers.insert(
                            "X-Circuit-Retry-After",
                            HeaderValue::from(*timeout_ms / 1000)
                        );
                        headers.insert(
                            "Retry-After",
                            HeaderValue::from(*timeout_ms / 1000)
                        );
                    }
                    
                    Ok(response)
                } else {
                    // Other error, return 500 Internal Server Error
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
    }
}

/// Layer for applying circuit breaker middleware
#[derive(Clone)]
pub struct CircuitBreakerLayer {
    circuit_breaker: Arc<CircuitBreaker>,
}

impl CircuitBreakerLayer {
    /// Create a new circuit breaker layer
    pub fn new(config: CircuitBreakerConfig, shared_state: Option<Arc<dyn SharedStateService>>) -> Self {
        Self {
            circuit_breaker: Arc::new(CircuitBreaker::new(config, shared_state)),
        }
    }
}

impl<S> Layer<S> for CircuitBreakerLayer {
    type Service = CircuitBreakerService<S>;
    
    fn layer(&self, inner: S) -> Self::Service {
        CircuitBreakerService {
            inner,
            circuit_breaker: self.circuit_breaker.clone(),
        }
    }
}

/// Service for applying circuit breaker middleware
#[derive(Clone)]
pub struct CircuitBreakerService<S> {
    inner: S,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for CircuitBreakerService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>> + 'static,
    ReqBody: Send + 'static,
    ResBody: Send + 'static,
{
    type Response = S::Response;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
    
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }
    
    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        // Clone the service and circuit breaker for the future
        let inner = self.inner.clone();
        let circuit_breaker = self.circuit_breaker.clone();
        
        // Extract the service name from the path
        let service = req.uri().path().split('/').take(2).collect::<Vec<_>>().join("/");
        
        // Extract the operation name if available
        let operation = req.uri().path().split('/').skip(2).take(1).next().map(String::from);
        
        // Create circuit breaker key
        let key = CircuitBreakerKey::new(service, operation);
        
        // Return the future that checks circuit breaker and then calls the inner service
        Box::pin(async move {
            // Check if request is allowed by circuit breaker
            match circuit_breaker.allow(&key).await {
                Ok(state) => {
                    // Request is allowed, call the inner service
                    let response = inner.call(req).await.map_err(|e| {
                        // Record failure on error from inner service
                        if let Err(err) = circuit_breaker.report_failure(&key) {
                            tracing::warn!("Failed to record circuit breaker failure: {}", err);
                        }
                        e.into()
                    })?;
                    
                    // Record success for half-open circuit
                    if state == CircuitState::HalfOpen {
                        // For half-open state, record success to close the circuit
                        if response.status().is_success() {
                            if let Err(e) = circuit_breaker.record_success(&key).await {
                                tracing::warn!("Failed to record circuit breaker success: {}", e);
                            }
                        } else {
                            // For a failure response, record failure to keep circuit open
                            if let Err(e) = circuit_breaker.report_failure(&key).await {
                                tracing::warn!("Failed to record circuit breaker failure: {}", e);
                            }
                        }
                    }
                    
                    // Add circuit breaker headers to response
                    if let Some(headers) = response.headers_mut() {
                        headers.insert(
                            "X-Circuit-State",
                            HeaderValue::from_str(&state.to_string()).unwrap()
                        );
                    }
                    
                    Ok(response)
                },
                Err(err) => {
                    if let ServerError::CircuitBreakerOpen { service, operation, failures, timeout_ms } = &err {
                        // Create a 503 Service Unavailable response
                        let mut response = Response::builder()
                            .status(StatusCode::SERVICE_UNAVAILABLE)
                            .body(Body::from(format!("{}", err)))
                            .unwrap();
                        
                        // Add circuit breaker headers
                        if let Some(headers) = response.headers_mut() {
                            headers.insert(
                                "X-Circuit-State",
                                HeaderValue::from_static("OPEN")
                            );
                            headers.insert(
                                "X-Circuit-Failures",
                                HeaderValue::from(*failures)
                            );
                            headers.insert(
                                "X-Circuit-Retry-After",
                                HeaderValue::from(*timeout_ms / 1000)
                            );
                            headers.insert(
                                "Retry-After",
                                HeaderValue::from(*timeout_ms / 1000)
                            );
                        }
                        
                        Ok(response)
                    } else {
                        // Other error, return 500 Internal Server Error
                        Err(Box::new(err))
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::body::Body;
    use tower::ServiceExt;
    
    struct TestService;
    
    impl Service<Request<Body>> for TestService {
        type Response = Response<Body>;
        type Error = Box<dyn std::error::Error + Send + Sync>;
        type Future = futures::future::Ready<Result<Self::Response, Self::Error>>;
        
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        
        fn call(&mut self, _req: Request<Body>) -> Self::Future {
            futures::future::ready(Ok(Response::new(Body::from("OK"))))
        }
    }
    
    #[tokio::test]
    async fn test_circuit_breaker_middleware() {
        // Create circuit breaker middleware
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout_ms: 1000, // 1 second
            shared_state_scope: None,
        };
        
        let middleware = CircuitBreakerMiddleware::new(config.clone(), None);
        
        // Create test request
        let request = Request::builder()
            .uri("/api/test")
            .body(Body::empty())
            .unwrap();
        
        // First request should pass
        let response = middleware.handle(request.clone(), Next::new(|req| async move {
            TestService.oneshot(req).await.unwrap()
        })).await.unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        // Record failures to open the circuit
        for _ in 0..3 {
            if let Err(e) = middleware.circuit_breaker.report_failure(
                &CircuitBreakerKey::new("/api", Some("test"))
            ).await {
                panic!("Failed to record failure: {}", e);
            }
        }
        
        // Next request should be blocked
        let response = middleware.handle(request.clone(), Next::new(|req| async move {
            TestService.oneshot(req).await.unwrap()
        })).await.unwrap();
        
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        
        // Check headers
        assert_eq!(
            response.headers().get("X-Circuit-State").unwrap().to_str().unwrap(),
            "OPEN"
        );
    }
} 
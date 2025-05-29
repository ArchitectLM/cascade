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
use crate::resilience::{RateLimiter, RateLimiterConfig, RateLimiterKey};
use crate::shared_state::SharedStateService;

/// Extract rate limiting headers from a request
fn extract_request_identifier<B>(req: &Request<B>) -> Option<String> {
    // Try to get API key from header
    if let Some(api_key) = req.headers().get("X-API-Key").and_then(|h| h.to_str().ok()) {
        return Some(api_key.to_string());
    }
    
    // Try to get client IP from ConnectInfo
    if let Some(connect_info) = req.extensions().get::<axum::extract::ConnectInfo<std::net::SocketAddr>>() {
        return Some(connect_info.0.ip().to_string());
    }
    
    // Try X-Forwarded-For header
    if let Some(forwarded_for) = req.headers().get("X-Forwarded-For").and_then(|h| h.to_str().ok()) {
        if let Some(ip) = forwarded_for.split(',').next().map(|s| s.trim()) {
            return Some(ip.to_string());
        }
    }
    
    None
}

/// Add rate limiting headers to a response
fn add_rate_limit_headers(
    headers: &mut HeaderMap,
    remaining: u32,
    max_requests: u32,
    window_ms: u64
) {
    headers.insert(
        "X-RateLimit-Remaining",
        HeaderValue::from(remaining)
    );
    
    headers.insert(
        "X-RateLimit-Limit",
        HeaderValue::from(max_requests)
    );
    
    headers.insert(
        "X-RateLimit-Reset",
        HeaderValue::from(window_ms)
    );
}

/// Rate limiter middleware for API endpoints
pub struct RateLimiterMiddleware {
    rate_limiter: Arc<RateLimiter>,
}

impl RateLimiterMiddleware {
    /// Create a new rate limiter middleware
    pub fn new(config: RateLimiterConfig, shared_state: Option<Arc<dyn SharedStateService>>) -> Self {
        Self {
            rate_limiter: Arc::new(RateLimiter::new(config, shared_state)),
        }
    }
    
    /// The middleware handler function
    pub async fn handle<B>(
        &self,
        req: Request<B>,
        next: Next<B>
    ) -> Result<Response, StatusCode> {
        // Extract the path for the rate limit key
        let path = req.uri().path().to_string();
        
        // Extract client IP or API key for identifier
        let identifier = extract_request_identifier(&req);
        
        // Create rate limiter key
        let key = RateLimiterKey::new(path, identifier);
        
        // Check if request is allowed by rate limiter
        match self.rate_limiter.allow(&key).await {
            Ok(remaining) => {
                // Request is allowed, proceed to next middleware or handler
                let mut response = next.run(req).await;
                
                // Add rate limit headers to response
                if let Some(headers) = response.headers_mut() {
                    add_rate_limit_headers(
                        headers,
                        remaining,
                        self.rate_limiter.config.max_requests,
                        self.rate_limiter.config.window_ms
                    );
                }
                
                Ok(response)
            },
            Err(err) => {
                if let ServerError::RateLimitExceeded { max_requests, window_ms, .. } = &err {
                    // Create a 429 response
                    let mut response = Response::builder()
                        .status(StatusCode::TOO_MANY_REQUESTS)
                        .body(Body::from(format!("{}", err)))
                        .unwrap();
                    
                    // Add rate limit headers to response
                    if let Some(headers) = response.headers_mut() {
                        add_rate_limit_headers(
                            headers,
                            0,
                            *max_requests,
                            *window_ms
                        );
                        
                        // Add Retry-After header
                        headers.insert(
                            "Retry-After",
                            HeaderValue::from(*window_ms / 1000)
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

/// Layer for applying rate limiter middleware
#[derive(Clone)]
pub struct RateLimiterLayer {
    rate_limiter: Arc<RateLimiter>,
}

impl RateLimiterLayer {
    /// Create a new rate limiter layer
    pub fn new(config: RateLimiterConfig, shared_state: Option<Arc<dyn SharedStateService>>) -> Self {
        Self {
            rate_limiter: Arc::new(RateLimiter::new(config, shared_state)),
        }
    }
}

impl<S> Layer<S> for RateLimiterLayer {
    type Service = RateLimiterService<S>;
    
    fn layer(&self, inner: S) -> Self::Service {
        RateLimiterService {
            inner,
            rate_limiter: self.rate_limiter.clone(),
        }
    }
}

/// Service for applying rate limiter middleware
#[derive(Clone)]
pub struct RateLimiterService<S> {
    inner: S,
    rate_limiter: Arc<RateLimiter>,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for RateLimiterService<S>
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
        // Clone the service and rate limiter for the future
        let inner = self.inner.clone();
        let rate_limiter = self.rate_limiter.clone();
        
        // Extract the path for the rate limit key
        let path = req.uri().path().to_string();
        
        // Extract client IP or API key for identifier
        let identifier = extract_request_identifier(&req);
        
        // Create rate limiter key
        let key = RateLimiterKey::new(path, identifier);
        
        // Return the future that checks rate limits and then calls the inner service
        Box::pin(async move {
            // Check if request is allowed by rate limiter
            match rate_limiter.allow(&key).await {
                Ok(remaining) => {
                    // Request is allowed, call the inner service
                    let mut response = inner.call(req).await.map_err(Into::into)?;
                    
                    // Add rate limit headers to response
                    if let Some(headers) = response.headers_mut() {
                        add_rate_limit_headers(
                            headers,
                            remaining,
                            rate_limiter.config.max_requests,
                            rate_limiter.config.window_ms
                        );
                    }
                    
                    Ok(response)
                },
                Err(err) => {
                    if let ServerError::RateLimitExceeded { max_requests, window_ms, .. } = &err {
                        // Create a 429 response
                        let mut response = Response::builder()
                            .status(StatusCode::TOO_MANY_REQUESTS)
                            .body(Body::from(format!("{}", err)))
                            .unwrap();
                        
                        // Add rate limit headers to response
                        if let Some(headers) = response.headers_mut() {
                            add_rate_limit_headers(
                                headers,
                                0,
                                *max_requests,
                                *window_ms
                            );
                            
                            // Add Retry-After header
                            headers.insert(
                                "Retry-After",
                                HeaderValue::from(*window_ms / 1000)
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
    use hyper::body::to_bytes;
    use hyper::body::Body;
    use http_body_util::BodyExt;
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
    async fn test_rate_limiter_middleware() {
        // Create rate limiter middleware
        let config = RateLimiterConfig {
            max_requests: 2,
            window_ms: 10000, // 10 seconds
            shared_state_scope: None,
        };
        
        let middleware = RateLimiterMiddleware::new(config.clone(), None);
        
        // Create test request
        let request = Request::builder()
            .uri("/api/test")
            .header("X-API-Key", "test-key")
            .body(Body::empty())
            .unwrap();
        
        // First request should pass
        let response = middleware.handle(request.clone(), Next::new(|req| async move {
            TestService.oneshot(req).await.unwrap()
        })).await.unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        // Check headers
        assert_eq!(
            response.headers().get("X-RateLimit-Remaining").unwrap().to_str().unwrap(),
            "1"
        );
        
        // Second request should pass
        let response = middleware.handle(request.clone(), Next::new(|req| async move {
            TestService.oneshot(req).await.unwrap()
        })).await.unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        // Check headers
        assert_eq!(
            response.headers().get("X-RateLimit-Remaining").unwrap().to_str().unwrap(),
            "0"
        );
        
        // Third request should be rate limited
        let response = middleware.handle(request.clone(), Next::new(|req| async move {
            TestService.oneshot(req).await.unwrap()
        })).await.unwrap();
        
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }
} 
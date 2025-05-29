use std::sync::Arc;

use axum::Router;
use axum::http::Request;
use axum::middleware::from_fn_with_state;
use tower::ServiceBuilder;
use tower::ServiceExt;

use crate::error::ServerResult;
use crate::resilience::{RateLimiterConfig, CircuitBreakerConfig, ResilienceConfig};
use crate::shared_state::SharedStateService;

use super::rate_limiter::{RateLimiterMiddleware, RateLimiterLayer};
use super::circuit_breaker::{CircuitBreakerMiddleware, CircuitBreakerLayer};

/// Middleware configuration for API endpoints
#[derive(Clone, Debug)]
pub struct MiddlewareConfig {
    /// Enable rate limiting
    pub enable_rate_limiting: bool,
    
    /// Enable circuit breaking
    pub enable_circuit_breaking: bool,
    
    /// Rate limiter configuration (if enabled)
    pub rate_limiter_config: Option<RateLimiterConfig>,
    
    /// Circuit breaker configuration (if enabled)
    pub circuit_breaker_config: Option<CircuitBreakerConfig>,
}

impl Default for MiddlewareConfig {
    fn default() -> Self {
        Self {
            enable_rate_limiting: true,
            enable_circuit_breaking: true,
            rate_limiter_config: Some(RateLimiterConfig::default()),
            circuit_breaker_config: Some(CircuitBreakerConfig::default()),
        }
    }
}

impl From<ResilienceConfig> for MiddlewareConfig {
    fn from(resilience_config: ResilienceConfig) -> Self {
        let rate_limiter_config = RateLimiterConfig {
            max_requests: resilience_config.rate_limit_max,
            window_ms: resilience_config.rate_limit_window_ms,
            shared_state_scope: resilience_config.shared_state_scope.clone(),
        };
        
        let circuit_breaker_config = CircuitBreakerConfig {
            failure_threshold: resilience_config.circuit_threshold,
            reset_timeout_ms: resilience_config.circuit_reset_ms,
            shared_state_scope: resilience_config.shared_state_scope,
        };
        
        Self {
            enable_rate_limiting: true,
            enable_circuit_breaking: true,
            rate_limiter_config: Some(rate_limiter_config),
            circuit_breaker_config: Some(circuit_breaker_config),
        }
    }
}

/// Factory for creating middleware
pub struct MiddlewareFactory {
    shared_state: Option<Arc<dyn SharedStateService>>,
}

impl MiddlewareFactory {
    /// Create a new middleware factory
    pub fn new(shared_state: Option<Arc<dyn SharedStateService>>) -> Self {
        Self { shared_state }
    }
    
    /// Apply middleware to a router based on configuration
    pub fn apply_middleware<S>(
        &self,
        router: Router<S>,
        config: MiddlewareConfig,
    ) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let mut middleware_layers = ServiceBuilder::new();
        
        // Add rate limiting middleware if enabled
        if config.enable_rate_limiting {
            if let Some(rate_config) = config.rate_limiter_config {
                middleware_layers = middleware_layers.layer(
                    RateLimiterLayer::new(rate_config, self.shared_state.clone())
                );
                
                tracing::info!("Rate limiting middleware enabled");
            }
        }
        
        // Add circuit breaker middleware if enabled
        if config.enable_circuit_breaking {
            if let Some(circuit_config) = config.circuit_breaker_config {
                middleware_layers = middleware_layers.layer(
                    CircuitBreakerLayer::new(circuit_config, self.shared_state.clone())
                );
                
                tracing::info!("Circuit breaker middleware enabled");
            }
        }
        
        // Apply middleware layers to router
        router.layer(middleware_layers)
    }
    
    /// Create rate limiter middleware with the given configuration
    pub fn create_rate_limiter(
        &self,
        config: RateLimiterConfig,
    ) -> RateLimiterMiddleware {
        RateLimiterMiddleware::new(config, self.shared_state.clone())
    }
    
    /// Create circuit breaker middleware with the given configuration
    pub fn create_circuit_breaker(
        &self,
        config: CircuitBreakerConfig,
    ) -> CircuitBreakerMiddleware {
        CircuitBreakerMiddleware::new(config, self.shared_state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::response::Response;
    use tower::Service;
    use http::StatusCode;
    
    #[tokio::test]
    async fn test_middleware_factory() {
        // Create middleware factory
        let factory = MiddlewareFactory::new(None);
        
        // Create configurations
        let rate_config = RateLimiterConfig {
            max_requests: 10,
            window_ms: 1000,
            shared_state_scope: None,
        };
        
        let circuit_config = CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout_ms: 1000,
            shared_state_scope: None,
        };
        
        // Create middleware
        let rate_middleware = factory.create_rate_limiter(rate_config);
        let circuit_middleware = factory.create_circuit_breaker(circuit_config);
        
        // Create test router
        let router = Router::new().route("/test", axum::routing::get(|| async { "OK" }));
        
        // Apply middleware
        let middleware_config = MiddlewareConfig {
            enable_rate_limiting: true,
            enable_circuit_breaking: true,
            rate_limiter_config: Some(rate_config),
            circuit_breaker_config: Some(circuit_config),
        };
        
        let _router_with_middleware = factory.apply_middleware(router, middleware_config);
        
        // Verify middlewares were created successfully
        assert!(rate_middleware.rate_limiter.config.max_requests == 10);
        assert!(circuit_middleware.circuit_breaker.config.failure_threshold == 3);
    }
    
    #[test]
    fn test_config_from_resilience_config() {
        let resilience_config = ResilienceConfig {
            shared_state_scope: Some("test-scope".to_string()),
            max_retries: 3,
            backoff_factor: 2.0,
            max_backoff_ms: 30000,
            rate_limit_max: 100,
            rate_limit_window_ms: 60000,
            circuit_threshold: 5,
            circuit_reset_ms: 30000,
        };
        
        let middleware_config = MiddlewareConfig::from(resilience_config);
        
        assert!(middleware_config.enable_rate_limiting);
        assert!(middleware_config.enable_circuit_breaking);
        
        assert_eq!(middleware_config.rate_limiter_config.unwrap().max_requests, 100);
        assert_eq!(middleware_config.rate_limiter_config.unwrap().window_ms, 60000);
        assert_eq!(middleware_config.rate_limiter_config.unwrap().shared_state_scope, Some("test-scope".to_string()));
        
        assert_eq!(middleware_config.circuit_breaker_config.unwrap().failure_threshold, 5);
        assert_eq!(middleware_config.circuit_breaker_config.unwrap().reset_timeout_ms, 30000);
        assert_eq!(middleware_config.circuit_breaker_config.unwrap().shared_state_scope, Some("test-scope".to_string()));
    }
} 
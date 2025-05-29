//! API middleware for Cascade Server
//! Provides middleware for rate limiting, circuit breaking, and other resilience patterns

pub mod rate_limiter;
pub mod circuit_breaker;
pub mod factory;

pub use rate_limiter::{RateLimiterMiddleware, RateLimiterLayer, RateLimiterService};
pub use circuit_breaker::{CircuitBreakerMiddleware, CircuitBreakerLayer, CircuitBreakerService};
pub use factory::{MiddlewareFactory, MiddlewareConfig}; 
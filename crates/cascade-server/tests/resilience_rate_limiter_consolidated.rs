use std::sync::Arc;
use cascade_server::error::ServerError;
use cascade_server::shared_state::InMemorySharedStateService;
use cascade_server::resilience::rate_limiter::{RateLimiter, RateLimiterConfig, RateLimiterKey};

// Inlined helper function
fn create_shared_state() -> Arc<InMemorySharedStateService> {
    Arc::new(InMemorySharedStateService::new())
}

/// Test rate limiter with local state
#[tokio::test]
async fn test_local_rate_limiter() {
    // Create a rate limiter with local state
    let config = RateLimiterConfig {
        max_requests: 3,
        window_ms: 1000,
        shared_state_scope: None,
    };
    
    let rate_limiter = RateLimiter::new(config, None);
    
    // Create a key for our resource
    let key = RateLimiterKey::new("test_resource".to_string(), None::<String>, None);
    
    // First request should succeed
    let result = rate_limiter.allow(&key).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 2); // 2 requests remaining
    
    // Second request should succeed
    let result = rate_limiter.allow(&key).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1); // 1 request remaining
    
    // Third request should succeed
    let result = rate_limiter.allow(&key).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0); // 0 requests remaining
    
    // Fourth request should fail
    let result = rate_limiter.allow(&key).await;
    assert!(result.is_err());
    
    let err = result.unwrap_err();
    match err {
        ServerError::RateLimitExceeded { resource, identifier, max_requests, window_ms } => {
            assert_eq!(resource, "test_resource");
            assert_eq!(identifier, "");
            assert_eq!(max_requests, 3);
            assert_eq!(window_ms, 1000);
        },
        _ => panic!("Unexpected error: {:?}", err),
    }
    
    // Test separate resources
    let key2 = RateLimiterKey::new("another_resource".to_string(), None::<String>, None);
    let result = rate_limiter.allow(&key2).await;
    assert!(result.is_ok());
}

/// Test rate limiter with distributed state
#[tokio::test]
async fn test_distributed_rate_limiter() {
    // Create a shared state service
    let shared_state = create_shared_state();
    
    // Create a rate limiter with distributed state
    let config = RateLimiterConfig {
        max_requests: 3,
        window_ms: 1000,
        shared_state_scope: Some("test_scope".to_string()),
    };
    
    let rate_limiter = RateLimiter::new(config.clone(), Some(shared_state.clone()));
    
    // Create a key for our resource
    let key = RateLimiterKey::new("test_resource".to_string(), None::<String>, None);
    
    // First request should succeed
    let result = rate_limiter.allow(&key).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 2); // 2 requests remaining
    
    // Second request should succeed
    let result = rate_limiter.allow(&key).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1); // 1 request remaining
    
    // Create a second rate limiter with the same config and shared state
    // This simulates another instance sharing the same state
    let rate_limiter2 = RateLimiter::new(config, Some(shared_state));
    
    // Third request from the second rate limiter should succeed
    // but should see the state from the first rate limiter
    let result = rate_limiter2.allow(&key).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0); // 0 requests remaining
    
    // Fourth request should fail
    let result = rate_limiter2.allow(&key).await;
    assert!(result.is_err());
    
    let err = result.unwrap_err();
    match err {
        ServerError::RateLimitExceeded { resource, identifier, max_requests, window_ms } => {
            assert_eq!(resource, "test_resource");
            assert_eq!(identifier, "");
            assert_eq!(max_requests, 3);
            assert_eq!(window_ms, 1000);
        },
        _ => panic!("Unexpected error: {:?}", err),
    }
}

/// Test rate limiter with identifier
#[tokio::test]
async fn test_rate_limiter_with_identifier() {
    // Create a shared state service
    let shared_state = create_shared_state();
    
    // Create a rate limiter with distributed state
    let config = RateLimiterConfig {
        max_requests: 2,
        window_ms: 1000,
        shared_state_scope: Some("user_limits".to_string()),
    };
    
    let rate_limiter = RateLimiter::new(config, Some(shared_state));
    
    // Create keys for different users
    let user1_key = RateLimiterKey::new("api".to_string(), Some("user1".to_string()), None);
    let user2_key = RateLimiterKey::new("api".to_string(), Some("user2".to_string()), None);
    
    // First request from user1 should succeed
    let result = rate_limiter.allow(&user1_key).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1); // 1 request remaining
    
    // First request from user2 should succeed (separate bucket)
    let result = rate_limiter.allow(&user2_key).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1); // 1 request remaining
    
    // Second request from user1 should succeed
    let result = rate_limiter.allow(&user1_key).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0); // 0 requests remaining
    
    // Third request from user1 should fail
    let result = rate_limiter.allow(&user1_key).await;
    assert!(result.is_err());
    
    // Second request from user2 should still succeed
    let result = rate_limiter.allow(&user2_key).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0); // 0 requests remaining
    
    // Third request from user2 should fail
    let result = rate_limiter.allow(&user2_key).await;
    assert!(result.is_err());
}

/// Test execute function with rate limiter
#[tokio::test]
async fn test_rate_limiter_execute() {
    // Create a rate limiter with local state
    let config = RateLimiterConfig {
        max_requests: 2,
        window_ms: 1000,
        shared_state_scope: None,
    };
    
    let rate_limiter = RateLimiter::new(config, None);
    
    // Create a key for our resource
    let key = RateLimiterKey::new("test_operation".to_string(), None::<String>, None);
    
    // First operation should succeed
    let result = rate_limiter.execute(&key, || async {
        Ok(42)
    }).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
    
    // Second operation should succeed
    let result = rate_limiter.execute(&key, || async {
        Ok("success")
    }).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success");
    
    // Third operation should fail due to rate limiting
    let result = rate_limiter.execute(&key, || async {
        // This closure should not be called
        panic!("This should not be called");
        #[allow(unreachable_code)]
        Ok::<(), ServerError>(())
    }).await;
    assert!(result.is_err());
    
    let err = result.unwrap_err();
    assert!(err.is_rate_limit_error());
}

/// Test rate limiter with local fallback when shared state fails
#[tokio::test]
async fn test_rate_limiter_fallback_on_error() {
    // Create a shared state service that will be dropped
    let shared_state = Arc::new(InMemorySharedStateService::new());
    
    // Create a rate limiter with distributed state
    let config = RateLimiterConfig {
        max_requests: 2,
        window_ms: 1000,
        shared_state_scope: Some("test_scope".to_string()),
    };
    
    let weak_state = Arc::downgrade(&shared_state);
    
    let rate_limiter = RateLimiter::new(config, Some(shared_state));
    
    // Create a key for our resource
    let key = RateLimiterKey::new("test_resource".to_string(), None::<String>, None);
    
    // First request should succeed
    let result = rate_limiter.allow(&key).await;
    assert!(result.is_ok());
    
    // Drop the shared state service to simulate an error
    drop(weak_state);
    
    // Second request should still succeed using local fallback
    let result = rate_limiter.allow(&key).await;
    assert!(result.is_ok());
} 
use std::time::Duration;
use tokio::time::sleep;
use cascade_core::CoreError;
use cascade_edge::resilience::{RateLimiter, RateLimiterConfig, TimeWindow};
use serde_json::json;

#[tokio::test]
async fn test_rate_limiter_basic() {
    // Create a rate limiter that allows 3 requests per second
    let config = RateLimiterConfig {
        max_requests: 3,
        time_window: TimeWindow::Second,
    };
    
    let rate_limiter = RateLimiter::new("test_limiter".to_string(), config);
    
    // Should allow up to max_requests
    for i in 1..=3 {
        let result = rate_limiter.execute(async {
            Ok::<_, CoreError>(json!({"request": i}))
        }).await;
        
        assert!(result.is_ok(), "Request {} should be allowed", i);
    }
    
    // Next request should be throttled
    let result = rate_limiter.execute(async {
        Ok::<_, CoreError>(json!({"request": 4}))
    }).await;
    
    assert!(result.is_err(), "Request 4 should be throttled");
    assert!(
        format!("{:?}", result.unwrap_err()).contains("Rate limit exceeded"),
        "Error should indicate rate limiting"
    );
    
    // Wait for the window to reset
    sleep(Duration::from_secs(1)).await;
    
    // Should allow requests again
    let result = rate_limiter.execute(async {
        Ok::<_, CoreError>(json!({"request": "after reset"}))
    }).await;
    
    assert!(result.is_ok(), "Request after window reset should be allowed");
}

#[tokio::test]
async fn test_rate_limiter_stats() {
    // Create a rate limiter
    let config = RateLimiterConfig {
        max_requests: 5,
        time_window: TimeWindow::Second,
    };
    
    let rate_limiter = RateLimiter::new("test_stats_limiter".to_string(), config);
    
    // Check initial stats
    let initial_stats = rate_limiter.get_stats().await;
    assert_eq!(initial_stats.id, "test_stats_limiter", "Stats should report correct ID");
    assert_eq!(initial_stats.current_count, 0, "Initial count should be 0");
    assert_eq!(initial_stats.max_requests, 5, "Max requests should be 5");
    assert_eq!(initial_stats.time_window, TimeWindow::Second, "Time window should be Second");
    
    // Execute some requests
    for _ in 1..=3 {
        let _ = rate_limiter.execute(async {
            Ok::<_, CoreError>(())
        }).await;
    }
    
    // Check updated stats
    let updated_stats = rate_limiter.get_stats().await;
    assert_eq!(updated_stats.current_count, 3, "Count should be updated to 3");
    assert!(updated_stats.reset_in_seconds <= 1, "Reset time should be 1 second or less");
    
    // Wait for the window to reset - use slightly more than 1 second to ensure reset
    sleep(Duration::from_millis(1100)).await;
    
    // Check stats after reset
    let reset_stats = rate_limiter.get_stats().await;
    assert_eq!(reset_stats.current_count, 0, "Count should reset to 0");
}

#[tokio::test]
async fn test_different_time_windows() {
    // Test with minute window
    let minute_config = RateLimiterConfig {
        max_requests: 2,
        time_window: TimeWindow::Minute,
    };
    
    let minute_limiter = RateLimiter::new("minute_limiter".to_string(), minute_config);
    
    // Execute allowed requests
    for i in 1..=2 {
        let result = minute_limiter.execute(async {
            Ok::<_, CoreError>(i)
        }).await;
        
        assert!(result.is_ok(), "Request {} should be allowed", i);
    }
    
    // Third request should be throttled
    let result = minute_limiter.execute(async {
        Ok::<_, CoreError>(3)
    }).await;
    
    assert!(result.is_err(), "Request should be throttled");
    
    // Check stats
    let stats = minute_limiter.get_stats().await;
    assert_eq!(stats.time_window, TimeWindow::Minute, "Time window should be Minute");
    assert!(stats.reset_in_seconds > 1, "Reset time for minute window should be greater than 1 second");
}

#[tokio::test]
async fn test_client_isolation() {
    // Create two rate limiters with the same config but different client IDs
    let config = RateLimiterConfig {
        max_requests: 2,
        time_window: TimeWindow::Second,
    };
    
    let client1 = RateLimiter::new("client1".to_string(), config.clone());
    let client2 = RateLimiter::new("client2".to_string(), config.clone());
    
    // Fill up client1's quota
    for i in 1..=2 {
        let result = client1.execute(async {
            Ok::<_, CoreError>(i)
        }).await;
        
        assert!(result.is_ok(), "Client1 request {} should be allowed", i);
    }
    
    // Next request to client1 should be throttled
    let result = client1.execute(async {
        Ok::<_, CoreError>(3)
    }).await;
    
    assert!(result.is_err(), "Client1 request 3 should be throttled");
    
    // But client2 should still have its own quota
    for i in 1..=2 {
        let result = client2.execute(async {
            Ok::<_, CoreError>(i)
        }).await;
        
        assert!(result.is_ok(), "Client2 request {} should be allowed", i);
    }
    
    // Now client2 should be throttled too
    let result = client2.execute(async {
        Ok::<_, CoreError>(3)
    }).await;
    
    assert!(result.is_err(), "Client2 request 3 should be throttled");
} 
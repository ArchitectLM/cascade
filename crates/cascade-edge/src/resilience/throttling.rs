use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};
use cascade_core::CoreError;
use tracing::debug;

/// Time window for rate limiting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeWindow {
    /// Second-based window
    Second,
    /// Minute-based window
    Minute,
    /// Hour-based window
    Hour,
    /// Day-based window
    Day,
}

impl TimeWindow {
    /// Convert to Duration
    fn to_duration(self) -> Duration {
        match self {
            TimeWindow::Second => Duration::from_secs(1),
            TimeWindow::Minute => Duration::from_secs(60),
            TimeWindow::Hour => Duration::from_secs(3600),
            TimeWindow::Day => Duration::from_secs(86400),
        }
    }
}

/// Rate limiter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiterConfig {
    /// Maximum number of requests allowed in the time window
    pub max_requests: u32,
    /// Time window for the rate limit
    pub time_window: TimeWindow,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            time_window: TimeWindow::Minute,
        }
    }
}

/// Rate limiter statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiterStats {
    /// Identifier for the rate limiter
    pub id: String,
    /// Current number of requests in the time window
    pub current_count: u32,
    /// Maximum requests allowed in the time window
    pub max_requests: u32,
    /// Time window for the rate limit
    pub time_window: TimeWindow,
    /// Time until the window resets (in seconds)
    pub reset_in_seconds: u64,
}

/// Rate limiter internal state
struct RateLimiterState {
    /// Current number of requests in the time window
    count: u32,
    /// Start time of the current window
    window_start: Instant,
}

/// Rate limiter implementation
#[derive(Clone)]
pub struct RateLimiter {
    /// Identifier for the rate limiter
    id: String,
    /// Configuration for the rate limiter
    config: RateLimiterConfig,
    /// Internal state
    state: Arc<Mutex<RateLimiterState>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(id: String, config: RateLimiterConfig) -> Self {
        Self {
            id,
            config,
            state: Arc::new(Mutex::new(RateLimiterState {
                count: 0,
                window_start: Instant::now(),
            })),
        }
    }

    /// Execute an operation with rate limiting
    pub async fn execute<F, T, E>(&self, operation: F) -> Result<T, CoreError>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: Into<CoreError>,
    {
        let can_execute = {
            let mut state = self.state.lock().await;
            
            // Check if we need to reset the window
            let now = Instant::now();
            let window_duration = self.config.time_window.to_duration();
            
            if now.duration_since(state.window_start) >= window_duration {
                // Reset window
                state.count = 0;
                state.window_start = now;
                debug!("Rate limiter {} window reset", self.id);
            }
            
            // Check if we're at the limit
            if state.count >= self.config.max_requests {
                false
            } else {
                // Increment the counter
                state.count += 1;
                debug!(
                    "Rate limiter {} count: {}/{}",
                    self.id, state.count, self.config.max_requests
                );
                true
            }
        };
        
        if !can_execute {
            return Err(CoreError::ComponentError(format!(
                "Rate limit exceeded for {}",
                self.id
            )));
        }
        
        // Execute the operation
        match operation.await {
            Ok(value) => Ok(value),
            Err(e) => Err(e.into()),
        }
    }
    
    /// Get current statistics for the rate limiter
    pub async fn get_stats(&self) -> RateLimiterStats {
        let mut state = self.state.lock().await;
        
        let now = Instant::now();
        let window_duration = self.config.time_window.to_duration();
        let elapsed = now.duration_since(state.window_start);
        
        // Check if we need to reset the window - just like in execute method
        if elapsed >= window_duration {
            // Reset window
            state.count = 0;
            state.window_start = now;
            debug!("Rate limiter {} window reset during stats check", self.id);
            
            // When the window has reset, there are 0 seconds until next reset,
            // but we'll return the full window duration as that's when the next reset will happen
            RateLimiterStats {
                id: self.id.clone(),
                current_count: 0,
                max_requests: self.config.max_requests,
                time_window: self.config.time_window,
                reset_in_seconds: window_duration.as_secs(),
            }
        } else {
            // Normal case - window hasn't expired yet
            let reset_in_seconds = (window_duration - elapsed).as_secs();
            
            RateLimiterStats {
                id: self.id.clone(),
                current_count: state.count,
                max_requests: self.config.max_requests,
                time_window: self.config.time_window,
                reset_in_seconds,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;
    
    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let config = RateLimiterConfig {
            max_requests: 3,
            time_window: TimeWindow::Second,
        };
        
        let rate_limiter = RateLimiter::new("test_limiter".to_string(), config);
        
        // Should allow max_requests
        for i in 1..=3 {
            let result = rate_limiter
                .execute(async { Ok::<_, CoreError>(()) })
                .await;
            assert!(result.is_ok(), "Request {} should be allowed", i);
        }
        
        // Next request should be rejected
        let result = rate_limiter
            .execute(async { Ok::<_, CoreError>(()) })
            .await;
        assert!(result.is_err(), "Request 4 should be rejected");
        assert!(
            format!("{:?}", result.unwrap_err()).contains("Rate limit exceeded"),
            "Error should indicate rate limit exceeded"
        );
        
        // After window expires, should allow more requests
        sleep(Duration::from_secs(1)).await;
        
        let result = rate_limiter
            .execute(async { Ok::<_, CoreError>(()) })
            .await;
        assert!(result.is_ok(), "Request after window expires should be allowed");
    }
    
    #[tokio::test]
    async fn test_rate_limiter_stats() {
        let config = RateLimiterConfig {
            max_requests: 5,
            time_window: TimeWindow::Second,
        };
        
        let rate_limiter = RateLimiter::new("stats_test".to_string(), config);
        
        // Initial stats
        let stats = rate_limiter.get_stats().await;
        assert_eq!(stats.current_count, 0);
        assert_eq!(stats.max_requests, 5);
        assert_eq!(stats.time_window, TimeWindow::Second);
        assert!(stats.reset_in_seconds <= 1);
        
        // Make some requests
        for _ in 0..3 {
            rate_limiter
                .execute(async { Ok::<_, CoreError>(()) })
                .await
                .unwrap();
        }
        
        // Check updated stats
        let stats = rate_limiter.get_stats().await;
        assert_eq!(stats.current_count, 3);
    }
} 
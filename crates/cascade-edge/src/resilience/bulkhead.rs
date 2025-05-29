use std::sync::Arc;
use tokio::sync::{Semaphore, Mutex};
use serde::{Serialize, Deserialize};
use cascade_core::CoreError;
use tracing::debug;

/// Bulkhead configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkheadConfig {
    /// Maximum number of concurrent calls allowed
    pub max_concurrent_calls: u32,
    /// Maximum size of the queue for pending calls
    pub max_queue_size: u32,
}

impl Default for BulkheadConfig {
    fn default() -> Self {
        Self {
            max_concurrent_calls: 10,
            max_queue_size: 5,
        }
    }
}

/// Statistics for the bulkhead
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkheadStats {
    /// Identifier for the bulkhead
    pub id: String,
    /// Current number of active calls
    pub active_count: u32,
    /// Maximum concurrent calls allowed
    pub max_concurrent: u32,
    /// Current queue size
    pub queue_size: u32,
    /// Maximum queue size allowed
    pub max_queue_size: u32,
}

/// Bulkhead implementation
#[derive(Clone)]
pub struct Bulkhead {
    id: String,
    config: BulkheadConfig,
    execution_semaphore: Arc<Semaphore>,
    queue_semaphore: Arc<Semaphore>,
    active_count: Arc<Mutex<u32>>,
}

impl Bulkhead {
    /// Create a new bulkhead
    pub fn new(id: String, config: BulkheadConfig) -> Self {
        Self {
            id,
            execution_semaphore: Arc::new(Semaphore::new(config.max_concurrent_calls as usize)),
            queue_semaphore: Arc::new(Semaphore::new(config.max_queue_size as usize)),
            active_count: Arc::new(Mutex::new(0)),
            config,
        }
    }

    /// Execute an operation with bulkhead protection
    pub async fn execute<F, T, E>(&self, operation: F) -> Result<T, CoreError>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: Into<CoreError>,
    {
        // Try to acquire a slot in the queue
        let queue_permit = match self.queue_semaphore.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                return Err(CoreError::ComponentError(format!(
                    "Bulkhead {} queue is full",
                    self.id
                )));
            }
        };

        // Now we're in the queue, wait for execution permit
        let execution_permit = match self.execution_semaphore.acquire().await {
            Ok(permit) => permit,
            Err(_) => {
                return Err(CoreError::ComponentError(format!(
                    "Bulkhead {} execution was interrupted",
                    self.id
                )));
            }
        };

        // Don't need the queue permit anymore
        drop(queue_permit);

        // Update active count
        {
            let mut count = self.active_count.lock().await;
            *count += 1;
            debug!(
                "Bulkhead {} active count: {}/{}",
                self.id, *count, self.config.max_concurrent_calls
            );
        }

        // Execute the operation
        let result = operation.await;

        // Update active count and release permit
        {
            let mut count = self.active_count.lock().await;
            *count = count.saturating_sub(1);
            debug!(
                "Bulkhead {} active count: {}/{}",
                self.id, *count, self.config.max_concurrent_calls
            );
        }
        drop(execution_permit);

        // Return the result
        match result {
            Ok(value) => Ok(value),
            Err(e) => Err(e.into()),
        }
    }

    /// Get current statistics for the bulkhead
    pub async fn get_stats(&self) -> BulkheadStats {
        let active_count = *self.active_count.lock().await;
        
        BulkheadStats {
            id: self.id.clone(),
            active_count,
            max_concurrent: self.config.max_concurrent_calls,
            queue_size: (self.config.max_queue_size - self.queue_semaphore.available_permits() as u32),
            max_queue_size: self.config.max_queue_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_bulkhead_limits_concurrency() {
        let config = BulkheadConfig {
            max_concurrent_calls: 2,
            max_queue_size: 1,
        };

        let bulkhead = Bulkhead::new("test_bulkhead".to_string(), config);

        // Start 2 long-running tasks (max concurrency)
        let task1 = tokio::spawn({
            let bulkhead = bulkhead.clone();
            async move {
                bulkhead
                    .execute(async {
                        sleep(Duration::from_millis(200)).await;
                        Ok::<_, CoreError>(1)
                    })
                    .await
            }
        });

        let task2 = tokio::spawn({
            let bulkhead = bulkhead.clone();
            async move {
                bulkhead
                    .execute(async {
                        sleep(Duration::from_millis(200)).await;
                        Ok::<_, CoreError>(2)
                    })
                    .await
            }
        });

        // Give them time to start
        sleep(Duration::from_millis(50)).await;

        // This one should go into the queue
        let task3 = tokio::spawn({
            let bulkhead = bulkhead.clone();
            async move {
                bulkhead
                    .execute(async {
                        sleep(Duration::from_millis(100)).await;
                        Ok::<_, CoreError>(3)
                    })
                    .await
            }
        });

        // Give it time to queue
        sleep(Duration::from_millis(50)).await;

        // This one should be rejected (queue full)
        let result = bulkhead
            .execute(async {
                sleep(Duration::from_millis(100)).await;
                Ok::<_, CoreError>(4)
            })
            .await;

        assert!(result.is_err());
        assert!(format!("{:?}", result.unwrap_err()).contains("queue is full"));

        // Wait for all tasks to complete
        let results = tokio::join!(task1, task2, task3);
        assert!(results.0.unwrap().is_ok());
        assert!(results.1.unwrap().is_ok());
        assert!(results.2.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_bulkhead_stats() {
        let config = BulkheadConfig {
            max_concurrent_calls: 3,
            max_queue_size: 2,
        };

        let bulkhead = Bulkhead::new("stats_test".to_string(), config);
        
        // Check initial stats
        let stats = bulkhead.get_stats().await;
        assert_eq!(stats.active_count, 0);
        assert_eq!(stats.max_concurrent, 3);
        assert_eq!(stats.queue_size, 0);
        assert_eq!(stats.max_queue_size, 2);
        
        // Start a task
        let _task = tokio::spawn({
            let bulkhead = bulkhead.clone();
            async move {
                bulkhead
                    .execute(async {
                        sleep(Duration::from_millis(500)).await;
                        Ok::<_, CoreError>(())
                    })
                    .await
            }
        });
        
        // Give it time to start
        sleep(Duration::from_millis(50)).await;
        
        // Check updated stats
        let stats = bulkhead.get_stats().await;
        assert_eq!(stats.active_count, 1);
    }
} 
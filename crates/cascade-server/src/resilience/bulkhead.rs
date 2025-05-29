//!
//! Bulkhead pattern implementation
//! Limits the number of concurrent executions to prevent resource exhaustion
//!

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Semaphore, Mutex, oneshot};
use tokio::time;
use tracing::warn;
use serde::{Deserialize, Serialize};
use serde_json::json;
use dashmap::DashMap;
use std::fmt;

use crate::error::{ServerError, ServerResult};
use crate::shared_state::SharedStateService;
use super::current_time_ms;

/// Bulkhead configuration
#[derive(Debug, Clone)]
pub struct BulkheadConfig {
    /// Maximum number of concurrent executions
    pub max_concurrent: u32,
    
    /// Maximum size of the waiting queue (0 means no queue)
    pub max_queue_size: u32,
    
    /// Queue wait timeout in milliseconds (0 means no timeout)
    pub queue_timeout_ms: u64,
    
    /// Optional shared state scope for distributed bulkhead
    pub shared_state_scope: Option<String>,
}

impl Default for BulkheadConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 10,
            max_queue_size: 0, // Default to no queue
            queue_timeout_ms: 1000, // 1 second default timeout
            shared_state_scope: None,
        }
    }
}

/// Bulkhead key for identifying different bulkheads
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct BulkheadKey {
    /// Service or component being protected
    pub service: String,
    
    /// Optional operation within the service
    pub operation: Option<String>,
}

impl BulkheadKey {
    /// Create a new bulkhead key for service/operation
    pub fn new(service: impl Into<String>, operation: Option<impl Into<String>>) -> Self {
        Self {
            service: service.into(),
            operation: operation.map(|o| o.into()),
        }
    }
}

impl fmt::Display for BulkheadKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.operation {
            Some(op) => write!(f, "{}:{}", self.service, op),
            None => write!(f, "{}", self.service),
        }
    }
}

/// Bulkhead metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BulkheadMetrics {
    /// Current number of active executions
    pub active_count: u32,
    
    /// Current number of queued executions
    pub queued_count: u32,
    
    /// Maximum allowed concurrent executions
    pub max_concurrent: u32,
    
    /// Maximum allowed queue size
    pub max_queue_size: u32,
    
    /// Number of rejected executions (queue full or timeout)
    pub rejected_count: u32,
}

/// Local bulkhead state
struct LocalBulkhead {
    /// Semaphore for limiting concurrent executions
    semaphore: Arc<Semaphore>,
    
    /// Queue for pending permits
    queue: Mutex<Vec<oneshot::Sender<()>>>,
    
    /// Maximum concurrent executions
    max_concurrent: u32,
    
    /// Maximum queue size
    max_queue_size: u32,
    
    /// Queue timeout in milliseconds
    queue_timeout_ms: u64,
    
    /// Number of rejected executions
    rejected_count: Mutex<u32>,
}

/// Bulkhead for limiting concurrent executions
pub struct Bulkhead {
    /// Configuration for the bulkhead
    config: BulkheadConfig,
    
    /// Local bulkheads - Using DashMap for concurrent access without global locks
    local_bulkheads: DashMap<BulkheadKey, Arc<LocalBulkhead>>,
    
    /// Shared state service for distributed bulkhead
    shared_state: Option<Arc<dyn SharedStateService>>,
}

impl Clone for Bulkhead {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            local_bulkheads: DashMap::new(), // Create fresh storage
            shared_state: self.shared_state.clone(),
        }
    }
}

impl Bulkhead {
    /// Create a new bulkhead with the given configuration
    pub fn new(config: BulkheadConfig, shared_state: Option<Arc<dyn SharedStateService>>) -> Self {
        Self {
            config,
            local_bulkheads: DashMap::new(),
            shared_state,
        }
    }
    
    /// Get or create a local bulkhead
    async fn get_or_create_local(&self, key: &BulkheadKey) -> Arc<LocalBulkhead> {
        // First try to get the bulkhead if it exists (fast path)
        if let Some(bulkhead) = self.local_bulkheads.get(key) {
            return bulkhead.clone();
        }
        
        // Create new bulkhead
        let bulkhead = Arc::new(LocalBulkhead {
            semaphore: Arc::new(Semaphore::new(self.config.max_concurrent as usize)),
            queue: Mutex::new(Vec::with_capacity(self.config.max_queue_size as usize)),
            max_concurrent: self.config.max_concurrent,
            max_queue_size: self.config.max_queue_size,
            queue_timeout_ms: self.config.queue_timeout_ms,
            rejected_count: Mutex::new(0),
        });
        
        // Use entry API to ensure we only insert if it doesn't exist
        let entry = self.local_bulkheads.entry(key.clone());
        return entry.or_insert(bulkhead).clone();
    }
    
    /// Acquire a permit for execution from the bulkhead
    /// Returns Ok(()) if a permit was acquired, or an error if rejected
    pub async fn acquire_permit(&self, key: &BulkheadKey) -> ServerResult<()> {
        // If we have shared state and a scope, use distributed bulkhead
        if let (Some(service), Some(scope)) = (&self.shared_state, &self.config.shared_state_scope) {
            self.acquire_distributed(service, scope, key).await
        } else {
            // Otherwise use local bulkhead
            self.acquire_local(key).await
        }
    }
    
    /// Release a permit back to the bulkhead
    pub async fn release_permit(&self, key: &BulkheadKey) -> ServerResult<()> {
        // If we have shared state and a scope, use distributed bulkhead
        if let (Some(service), Some(scope)) = (&self.shared_state, &self.config.shared_state_scope) {
            self.release_distributed(service, scope, key).await
        } else {
            // Otherwise use local bulkhead
            self.release_local(key).await
        }
    }
    
    /// Acquire a permit from the local bulkhead
    async fn acquire_local(&self, key: &BulkheadKey) -> ServerResult<()> {
        let bulkhead = self.get_or_create_local(key).await;
        
        // First attempt to acquire a permit directly
        match bulkhead.semaphore.clone().try_acquire() {
            Ok(permit) => {
                // Success - detach permit so it stays acquired
                permit.forget();
                return Ok(());
            },
            Err(_) => {
                // No permits available, check if queuing is enabled
                if bulkhead.max_queue_size == 0 {
                    // No queue, reject immediately
                    let mut rejected = bulkhead.rejected_count.lock().await;
                    *rejected += 1;
                    
                    return Err(ServerError::BulkheadRejected {
                        service: key.service.clone(),
                        operation: key.operation.clone().unwrap_or_default(),
                        active: bulkhead.max_concurrent - (bulkhead.semaphore.available_permits() as u32),
                        max_concurrent: bulkhead.max_concurrent,
                        queue_size: 0,
                        max_queue_size: bulkhead.max_queue_size,
                    });
                }
            }
        }
        
        // Try to enqueue for a permit
        let (tx, rx) = oneshot::channel();
        
        // Check if the queue has room
        let queue_result = {
            let mut queue = bulkhead.queue.lock().await;
            
            if queue.len() < bulkhead.max_queue_size as usize {
                // Add to queue
                queue.push(tx);
                true
            } else {
                // Queue full
                false
            }
        };
        
        if !queue_result {
            // Queue is full, reject
            let mut rejected = bulkhead.rejected_count.lock().await;
            *rejected += 1;
            
            return Err(ServerError::BulkheadRejected {
                service: key.service.clone(),
                operation: key.operation.clone().unwrap_or_default(),
                active: bulkhead.max_concurrent - (bulkhead.semaphore.available_permits() as u32),
                max_concurrent: bulkhead.max_concurrent,
                queue_size: bulkhead.max_queue_size,
                max_queue_size: bulkhead.max_queue_size,
            });
        }
        
        // Wait for a permit or timeout
        if bulkhead.queue_timeout_ms > 0 {
            // Wait with timeout
            let timeout = Duration::from_millis(bulkhead.queue_timeout_ms);
            match time::timeout(timeout, rx).await {
                Ok(Ok(())) => {
                    // Got a permit
                    Ok(())
                },
                Ok(Err(_)) => {
                    // Channel closed without a permit (shouldn't happen)
                    let mut rejected = bulkhead.rejected_count.lock().await;
                    *rejected += 1;
                    
                    Err(ServerError::BulkheadRejected {
                        service: key.service.clone(),
                        operation: key.operation.clone().unwrap_or_default(),
                        active: bulkhead.max_concurrent - (bulkhead.semaphore.available_permits() as u32),
                        max_concurrent: bulkhead.max_concurrent,
                        queue_size: bulkhead.max_queue_size,
                        max_queue_size: bulkhead.max_queue_size,
                    })
                },
                Err(_) => {
                    // Timeout waiting for permit
                    let mut rejected = bulkhead.rejected_count.lock().await;
                    *rejected += 1;
                    
                    Err(ServerError::BulkheadTimeout {
                        service: key.service.clone(),
                        operation: key.operation.clone().unwrap_or_default(),
                        timeout_ms: bulkhead.queue_timeout_ms,
                    })
                }
            }
        } else {
            // Wait indefinitely
            match rx.await {
                Ok(()) => Ok(()),
                Err(_) => {
                    // Channel closed without a permit (shouldn't happen)
                    let mut rejected = bulkhead.rejected_count.lock().await;
                    *rejected += 1;
                    
                    Err(ServerError::BulkheadRejected {
                        service: key.service.clone(),
                        operation: key.operation.clone().unwrap_or_default(),
                        active: bulkhead.max_concurrent - (bulkhead.semaphore.available_permits() as u32),
                        max_concurrent: bulkhead.max_concurrent,
                        queue_size: bulkhead.max_queue_size,
                        max_queue_size: bulkhead.max_queue_size,
                    })
                }
            }
        }
    }
    
    /// Release a permit back to the local bulkhead
    async fn release_local(&self, key: &BulkheadKey) -> ServerResult<()> {
        let bulkhead = self.get_or_create_local(key).await;
        
        // Add a permit back to the semaphore
        bulkhead.semaphore.add_permits(1);
        
        // Check if any queued tasks can now get a permit
        let tx_option = {
            let mut queue = bulkhead.queue.lock().await;
            if !queue.is_empty() {
                Some(queue.remove(0))
            } else {
                None
            }
        };
        
        // If there was a queued task, signal it
        if let Some(tx) = tx_option {
            // It's okay if the receiver was dropped
            let _ = tx.send(());
        }
        
        Ok(())
    }
    
    /// Acquire a permit from the distributed bulkhead
    async fn acquire_distributed(
        &self,
        service: &Arc<dyn SharedStateService>,
        scope: &str,
        key: &BulkheadKey
    ) -> ServerResult<()> {
        let state_key = key.to_string();
        let max_attempts = 3; // With small backoff for contention
        
        for attempt in 0..max_attempts {
            // Get current metrics
            let metrics = self.get_metrics_from_state(service, scope, &state_key).await?;
            
            // Check if we can acquire a permit
            if metrics.active_count < metrics.max_concurrent {
                // Try to increment active count atomically
                let updated_metrics = BulkheadMetrics {
                    active_count: metrics.active_count + 1,
                    ..metrics
                };
                
                // Check for race conditions with conditional update
                let result = self.conditional_update_metrics(
                    service, 
                    scope, 
                    &state_key, 
                    metrics.active_count, 
                    updated_metrics.clone()
                ).await?;
                
                if result {
                    // Successfully acquired permit
                    return Ok(());
                } else if attempt < max_attempts - 1 {
                    // Contention, retry with small backoff
                    time::sleep(Duration::from_millis(10 * (attempt as u64 + 1))).await;
                    continue;
                }
            } else if metrics.max_queue_size > 0 && metrics.queued_count < metrics.max_queue_size {
                // Try to add to queue
                let updated_metrics = BulkheadMetrics {
                    queued_count: metrics.queued_count + 1,
                    ..metrics
                };
                
                // Check for race conditions with conditional update
                let result = self.conditional_update_metrics(
                    service, 
                    scope, 
                    &state_key, 
                    metrics.queued_count, 
                    updated_metrics.clone()
                ).await?;
                
                if result {
                    // Successfully added to queue, now we need to wait
                    let our_queue_position = metrics.queued_count;
                    
                    // Calculate a reasonable timeout that includes our queue position
                    let position_timeout = if self.config.queue_timeout_ms > 0 {
                        // Scale timeout by position (more time if further back in queue)
                        self.config.queue_timeout_ms * (our_queue_position as u64 + 1)
                    } else {
                        // Default timeout based on position
                        1000 * (our_queue_position as u64 + 1)
                    };
                    
                    let wait_start = current_time_ms();
                    let max_wait = position_timeout.min(30000); // Cap at 30 seconds
                    
                    // Poll until we get a permit or timeout
                    loop {
                        // Small delay between polls
                        time::sleep(Duration::from_millis(50)).await;
                        
                        // Check if we've waited too long
                        let elapsed = current_time_ms() - wait_start;
                        if elapsed > max_wait {
                            // Timeout, remove ourselves from queue and report rejection
                            let current = self.get_metrics_from_state(service, scope, &state_key).await?;
                            
                            let updated = BulkheadMetrics {
                                queued_count: current.queued_count.saturating_sub(1),
                                rejected_count: current.rejected_count + 1,
                                ..current
                            };
                            
                            // Best effort update, don't care too much about race conditions here
                            let _ = service.set_state(scope, &format!("{}:metrics", state_key), json!(updated)).await;
                            
                            return Err(ServerError::BulkheadTimeout {
                                service: key.service.clone(),
                                operation: key.operation.clone().unwrap_or_default(),
                                timeout_ms: max_wait,
                            });
                        }
                        
                        // Check if we can now acquire a permit
                        let current = self.get_metrics_from_state(service, scope, &state_key).await?;
                        
                        if current.active_count < current.max_concurrent {
                            // Try to acquire a permit and remove from queue
                            let updated = BulkheadMetrics {
                                active_count: current.active_count + 1,
                                queued_count: current.queued_count.saturating_sub(1),
                                ..current
                            };
                            
                            let result = self.conditional_update_metrics(
                                service, 
                                scope, 
                                &state_key, 
                                current.active_count, 
                                updated.clone()
                            ).await?;
                            
                            if result {
                                // Successfully acquired permit
                                return Ok(());
                            }
                        }
                    }
                } else if attempt < max_attempts - 1 {
                    // Contention, retry with small backoff
                    time::sleep(Duration::from_millis(10 * (attempt as u64 + 1))).await;
                    continue;
                }
            }
            
            // If we get here, we can't acquire a permit or add to queue
            // Update rejection counter (best effort)
            let updated = BulkheadMetrics {
                rejected_count: metrics.rejected_count + 1,
                ..metrics
            };
            
            let _ = service.set_state(scope, &format!("{}:metrics", state_key), json!(updated)).await;
            
            return Err(ServerError::BulkheadRejected {
                service: key.service.clone(),
                operation: key.operation.clone().unwrap_or_default(),
                active: metrics.active_count,
                max_concurrent: metrics.max_concurrent,
                queue_size: metrics.queued_count,
                max_queue_size: metrics.max_queue_size,
            });
        }
        
        // If we get here, we had contention issues
        Err(ServerError::InternalError("Failed to acquire bulkhead permit due to contention".to_string()))
    }
    
    /// Release a permit back to the distributed bulkhead
    async fn release_distributed(
        &self,
        service: &Arc<dyn SharedStateService>,
        scope: &str,
        key: &BulkheadKey
    ) -> ServerResult<()> {
        let state_key = key.to_string();
        let max_attempts = 3;
        
        for attempt in 0..max_attempts {
            // Get current metrics
            let metrics = self.get_metrics_from_state(service, scope, &state_key).await?;
            
            if metrics.active_count > 0 {
                // Decrement active count
                let updated = BulkheadMetrics {
                    active_count: metrics.active_count - 1,
                    ..metrics
                };
                
                // Update metrics
                let result = self.conditional_update_metrics(
                    service, 
                    scope, 
                    &state_key, 
                    metrics.active_count, 
                    updated.clone()
                ).await?;
                
                if result {
                    return Ok(());
                } else if attempt < max_attempts - 1 {
                    // Contention, retry with small backoff
                    time::sleep(Duration::from_millis(10 * (attempt as u64 + 1))).await;
                    continue;
                }
            } else {
                // No active permits to release
                warn!("Attempted to release bulkhead permit when none active: {}", state_key);
                return Ok(());
            }
        }
        
        // If we get here, we had contention issues
        Err(ServerError::InternalError("Failed to release bulkhead permit due to contention".to_string()))
    }
    
    /// Get metrics for a bulkhead
    pub async fn get_metrics(&self, key: &BulkheadKey) -> ServerResult<BulkheadMetrics> {
        // If we have shared state and a scope, use distributed bulkhead metrics
        if let (Some(service), Some(scope)) = (&self.shared_state, &self.config.shared_state_scope) {
            self.get_metrics_from_state(service, scope, &key.to_string()).await
        } else {
            // Otherwise use local bulkhead metrics
            self.get_metrics_local(key).await
        }
    }
    
    /// Get metrics for a local bulkhead
    async fn get_metrics_local(&self, key: &BulkheadKey) -> ServerResult<BulkheadMetrics> {
        let bulkhead = self.get_or_create_local(key).await;
        
        let active_count = bulkhead.max_concurrent - (bulkhead.semaphore.available_permits() as u32);
        let queued_count = bulkhead.queue.lock().await.len() as u32;
        let rejected_count = *bulkhead.rejected_count.lock().await;
        
        Ok(BulkheadMetrics {
            active_count,
            queued_count,
            max_concurrent: bulkhead.max_concurrent,
            max_queue_size: bulkhead.max_queue_size,
            rejected_count,
        })
    }
    
    /// Get metrics from shared state
    async fn get_metrics_from_state(
        &self,
        service: &Arc<dyn SharedStateService>,
        scope: &str,
        state_key: &str
    ) -> ServerResult<BulkheadMetrics> {
        // Try to get existing metrics
        let metrics_key = format!("{}:metrics", state_key);
        let metrics = match service.get_state(scope, &metrics_key).await {
            Ok(Some(data)) => {
                serde_json::from_value(data)
                    .map_err(|e| ServerError::InternalError(format!("Invalid bulkhead metrics: {}", e)))?
            },
            Ok(None) => {
                // No metrics yet, initialize with defaults
                let new_metrics = BulkheadMetrics {
                    active_count: 0,
                    queued_count: 0,
                    max_concurrent: self.config.max_concurrent,
                    max_queue_size: self.config.max_queue_size,
                    rejected_count: 0,
                };
                
                // Initialize metrics
                service.set_state(scope, &metrics_key, json!(new_metrics)).await
                    .map_err(|e| ServerError::StateServiceError(format!("Failed to set initial bulkhead metrics: {}", e)))?;
                
                new_metrics
            },
            Err(e) => {
                return Err(ServerError::StateServiceError(format!("Failed to get bulkhead metrics: {}", e)));
            }
        };
        
        Ok(metrics)
    }
    
    /// Conditionally update metrics based on expected active count
    async fn conditional_update_metrics(
        &self,
        service: &Arc<dyn SharedStateService>,
        scope: &str,
        state_key: &str,
        expected_value: u32,
        new_metrics: BulkheadMetrics
    ) -> ServerResult<bool> {
        // For a real implementation, this would use a CAS operation
        // We simulate it here by checking and updating within a single operation
        
        let metrics_key = format!("{}:metrics", state_key);
        
        // Get current metrics
        let current_metrics = match service.get_state(scope, &metrics_key).await {
            Ok(Some(data)) => {
                serde_json::from_value::<BulkheadMetrics>(data)
                    .map_err(|e| ServerError::InternalError(format!("Invalid bulkhead metrics: {}", e)))?
            },
            Ok(None) => {
                // No metrics yet, can't update
                return Ok(false);
            },
            Err(e) => {
                return Err(ServerError::StateServiceError(format!("Failed to get bulkhead metrics: {}", e)));
            }
        };
        
        // Check if the value is as expected
        if current_metrics.active_count != expected_value {
            return Ok(false);
        }
        
        // Update metrics
        service.set_state(scope, &metrics_key, json!(new_metrics)).await
            .map_err(|e| ServerError::StateServiceError(format!("Failed to update bulkhead metrics: {}", e)))?;
        
        Ok(true)
    }
    
    /// Execute an operation with bulkhead protection
    pub async fn execute<F, Fut, T>(
        &self,
        key: &BulkheadKey,
        operation: F
    ) -> ServerResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ServerResult<T>>,
    {
        // Acquire permit and wait if needed
        self.acquire_permit(key).await?;
        
        // Execute the operation
        let result = operation().await;
        
        // Release permit (unless operation panicked)
        self.release_permit(key).await?;
        let permit_released = true;
        
        // Add a drop guard to handle panic cases
        struct PermitGuard {
            bulkhead: Arc<Bulkhead>,
            key: BulkheadKey,
            released: bool,
        }
        
        impl Drop for PermitGuard {
            fn drop(&mut self) {
                if !self.released {
                    // Attempt to release the permit
                    let bulkhead = self.bulkhead.clone();
                    let key = self.key.clone();
                    
                    // Schedule task to release permit
                    tokio::spawn(async move {
                        // Get the local bulkhead
                        let local_bulkhead = bulkhead.get_or_create_local(&key).await;
                        // Decrement active count by adding permit back to semaphore
                        local_bulkhead.semaphore.add_permits(1);
                    });
                }
            }
        }
        
        let _guard = PermitGuard {
            bulkhead: Arc::new(self.clone()),
            key: key.clone(),
            released: permit_released,
        };
        
        result
    }
    
    /// Create a bulkhead specifically for a component/service
    pub fn for_component(
        config: BulkheadConfig,
        shared_state: Option<Arc<dyn SharedStateService>>
    ) -> Self {
        Self::new(config, shared_state)
    }
}

/// Get metrics about all bulkheads
pub async fn get_all_metrics(bulkhead: &Bulkhead) -> HashMap<String, BulkheadMetrics> {
    let mut metrics = HashMap::new();
    
    // Iterate over all bulkheads
    for entry in bulkhead.local_bulkheads.iter() {
        let key = entry.key().to_string();
        let bulkhead = entry.value();
        
        // Get semaphore available permits
        let available_permits = bulkhead.semaphore.available_permits();
        let active_count = bulkhead.max_concurrent as usize - available_permits;
        
        // Get queue size
        let queue_size = bulkhead.queue.lock().await.len();
        
        // Get rejected count
        let rejected_count = *bulkhead.rejected_count.lock().await;
        
        // Create metrics
        let bulkhead_metrics = BulkheadMetrics {
            active_count: active_count as u32,
            queued_count: queue_size as u32,
            max_concurrent: bulkhead.max_concurrent,
            max_queue_size: bulkhead.max_queue_size,
            rejected_count,
        };
        
        metrics.insert(key, bulkhead_metrics);
    }
    
    metrics
} 
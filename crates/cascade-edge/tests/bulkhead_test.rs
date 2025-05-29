use cascade_core::CoreError;
use cascade_edge::resilience::bulkhead::{Bulkhead, BulkheadConfig};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{sleep, Duration};
use futures::future;
use futures::future::join_all;

#[tokio::test]
async fn test_bulkhead_initial_state() {
    // Configure a simple bulkhead
    let config = BulkheadConfig {
        max_concurrent_calls: 5,
        max_queue_size: 3
    };
    
    let bulkhead = Bulkhead::new("test_service".to_string(), config);
    
    // Check initial stats
    let stats = bulkhead.get_stats().await;
    assert_eq!(stats.active_count, 0, "Initial active count should be 0");
    assert_eq!(stats.queue_size, 0, "Initial queue size should be 0");
}

#[tokio::test]
async fn test_bulkhead_max_concurrency() {
    // Configure bulkhead with small capacity
    let config = BulkheadConfig {
        max_concurrent_calls: 2,
        max_queue_size: 0 // No queueing
    };
    
    let bulkhead = Bulkhead::new("test_service".to_string(), config);
    
    // Validate the configuration is set correctly
    let stats = bulkhead.get_stats().await;
    assert_eq!(stats.max_concurrent, 2, "Max concurrent should be 2");
    assert_eq!(stats.max_queue_size, 0, "Max queue size should be 0");
    
    // Hold a lock to prevent execution from completing
    let execution_lock = Arc::new(Mutex::new(()));
    let lock1 = execution_lock.lock().await;
    
    // Start a task that will block on the lock
    let bulkhead_clone = bulkhead.clone();
    let lock_clone = execution_lock.clone();
    
    let _handle = tokio::spawn(async move {
        bulkhead_clone.execute::<_, Value, CoreError>(async move {
            // Wait for the lock to be released
            let _guard = lock_clone.lock().await;
            Ok(json!({"task_id": 1}))
        }).await
    });
    
    // Start another task
    let bulkhead_clone = bulkhead.clone();
    let lock_clone = execution_lock.clone();
    
    let _handle2 = tokio::spawn(async move {
        bulkhead_clone.execute::<_, Value, CoreError>(async move {
            // Wait for the lock to be released
            let _guard = lock_clone.lock().await;
            Ok(json!({"task_id": 2}))
        }).await
    });
    
    // Give tasks time to register with the bulkhead
    sleep(Duration::from_millis(50)).await;
    
    // Try to execute one more task, should be rejected because we're at capacity
    let result = bulkhead.execute::<_, Value, CoreError>(async {
        Ok(json!({"extra_task": true}))
    }).await;
    
    // This is the key assertion - we should get rejected when trying to exceed capacity
    assert!(result.is_err(), "Should reject task when at capacity");
    
    // Release the lock to allow tasks to complete
    drop(lock1);
}

#[tokio::test]
async fn test_bulkhead_queuing() {
    // Configure bulkhead with queueing
    let config = BulkheadConfig {
        max_concurrent_calls: 1,
        max_queue_size: 2 // Allow 2 queued tasks
    };
    
    let bulkhead = Arc::new(Bulkhead::new("test_queue".to_string(), config));
    
    // Task coordination
    let task_barrier = Arc::new(Semaphore::new(0));
    let execution_order = Arc::new(Mutex::new(Vec::new()));
    
    // First task - will occupy the only concurrent slot
    let bulkhead_clone = bulkhead.clone();
    let barrier_clone = task_barrier.clone();
    let order_clone = execution_order.clone();
    
    let handle1 = tokio::spawn(async move {
        let result = bulkhead_clone.execute::<_, Value, CoreError>(async {
            order_clone.lock().await.push(1);
            let _permit = barrier_clone.acquire().await.unwrap();
            Ok(json!({"task": "concurrent"}))
        }).await;
        
        result
    });
    
    // Give time for first task to start
    sleep(Duration::from_millis(100)).await;
    
    // Two more tasks that will queue
    let bulkhead_clone = bulkhead.clone();
    let barrier_clone = task_barrier.clone();
    let order_clone = execution_order.clone();
    
    let handle2 = tokio::spawn(async move {
        let result = bulkhead_clone.execute::<_, Value, CoreError>(async {
            order_clone.lock().await.push(2);
            let _permit = barrier_clone.acquire().await.unwrap();
            Ok(json!({"task": "queued_1"}))
        }).await;
        
        result
    });
    
    let bulkhead_clone = bulkhead.clone();
    let barrier_clone = task_barrier.clone();
    let order_clone = execution_order.clone();
    
    let handle3 = tokio::spawn(async move {
        let result = bulkhead_clone.execute::<_, Value, CoreError>(async {
            order_clone.lock().await.push(3);
            let _permit = barrier_clone.acquire().await.unwrap();
            Ok(json!({"task": "queued_2"}))
        }).await;
        
        result
    });
    
    // Give time for tasks to register
    sleep(Duration::from_millis(200)).await;
    
    // Verify stats
    let stats = bulkhead.get_stats().await;
    assert_eq!(stats.active_count, 1, "Should have 1 active task");
    assert_eq!(stats.queue_size, 2, "Should have 2 queued tasks");
    
    // Fourth task should be rejected
    let result = bulkhead.execute::<_, Value, CoreError>(async {
        Ok(json!({"should_be_rejected": true}))
    }).await;
    
    assert!(result.is_err(), "Should reject when queue is full");
    
    // Release all tasks
    task_barrier.add_permits(3);
    
    // Wait for all tasks to complete
    let _ = future::join_all(vec![handle1, handle2, handle3]).await;
    
    // Verify final stats
    let final_stats = bulkhead.get_stats().await;
    assert_eq!(final_stats.active_count, 0, "No active tasks after completion");
    assert_eq!(final_stats.queue_size, 0, "No queued tasks after completion");
    
    // Verify execution order
    let order = execution_order.lock().await;
    assert_eq!(order.len(), 3, "All three tasks should have executed");
    assert_eq!(order[0], 1, "First task should execute first");
}

#[tokio::test]
async fn test_bulkhead_multiple_tiers() {
    // Create two bulkheads with different configurations
    let standard_config = BulkheadConfig {
        max_concurrent_calls: 2,
        max_queue_size: 1
    };
    
    let premium_config = BulkheadConfig {
        max_concurrent_calls: 4,
        max_queue_size: 2
    };
    
    let standard_bulkhead = Arc::new(Bulkhead::new("standard".to_string(), standard_config));
    let premium_bulkhead = Arc::new(Bulkhead::new("premium".to_string(), premium_config));
    
    // Simple usage test - verify basic properties
    let standard_stats = standard_bulkhead.get_stats().await;
    let premium_stats = premium_bulkhead.get_stats().await;
    
    assert_eq!(standard_stats.max_concurrent, 2);
    assert_eq!(premium_stats.max_concurrent, 4);
    
    assert_eq!(standard_stats.max_queue_size, 1);
    assert_eq!(premium_stats.max_queue_size, 2);
}

#[tokio::test]
async fn test_bulkhead_limits_concurrency() {
    // Configure a bulkhead with max 2 concurrent calls and a queue size of 1
    let config = BulkheadConfig {
        max_concurrent_calls: 2,
        max_queue_size: 1,
    };
    
    let bulkhead = Bulkhead::new("test_bulkhead".to_string(), config);
    
    // Create a success counter to track completed executions
    let success_counter = Arc::new(Mutex::new(0));
    let max_concurrent = Arc::new(Mutex::new(0));
    let current_concurrent = Arc::new(Mutex::new(0));
    
    // Start 2 long-running tasks (max concurrency)
    let task1 = tokio::spawn({
        let bulkhead = bulkhead.clone();
        let counter = success_counter.clone();
        let max_concurrent = max_concurrent.clone();
        let current_concurrent = current_concurrent.clone();
        
        async move {
            let result = bulkhead.execute(async {
                // Increment concurrency counters
                {
                    let mut current = current_concurrent.lock().await;
                    *current += 1;
                    
                    let mut max = max_concurrent.lock().await;
                    if *current > *max {
                        *max = *current;
                    }
                }
                
                // Simulate work
                sleep(Duration::from_millis(200)).await;
                
                // Decrement concurrency counter
                {
                    let mut current = current_concurrent.lock().await;
                    *current -= 1;
                }
                
                // Record success
                let mut successes = counter.lock().await;
                *successes += 1;
                
                Ok::<_, CoreError>(1)
            }).await;
            
            result
        }
    });
    
    let task2 = tokio::spawn({
        let bulkhead = bulkhead.clone();
        let counter = success_counter.clone();
        let max_concurrent = max_concurrent.clone();
        let current_concurrent = current_concurrent.clone();
        
        async move {
            let result = bulkhead.execute(async {
                // Increment concurrency counters
                {
                    let mut current = current_concurrent.lock().await;
                    *current += 1;
                    
                    let mut max = max_concurrent.lock().await;
                    if *current > *max {
                        *max = *current;
                    }
                }
                
                // Simulate work
                sleep(Duration::from_millis(200)).await;
                
                // Decrement concurrency counter
                {
                    let mut current = current_concurrent.lock().await;
                    *current -= 1;
                }
                
                // Record success
                let mut successes = counter.lock().await;
                *successes += 1;
                
                Ok::<_, CoreError>(2)
            }).await;
            
            result
        }
    });
    
    // Give them time to start
    sleep(Duration::from_millis(50)).await;
    
    // This one should go into the queue
    let task3 = tokio::spawn({
        let bulkhead = bulkhead.clone();
        let counter = success_counter.clone();
        let max_concurrent = max_concurrent.clone();
        let current_concurrent = current_concurrent.clone();
        
        async move {
            let result = bulkhead.execute(async {
                // Increment concurrency counters
                {
                    let mut current = current_concurrent.lock().await;
                    *current += 1;
                    
                    let mut max = max_concurrent.lock().await;
                    if *current > *max {
                        *max = *current;
                    }
                }
                
                // Simulate work
                sleep(Duration::from_millis(100)).await;
                
                // Decrement concurrency counter
                {
                    let mut current = current_concurrent.lock().await;
                    *current -= 1;
                }
                
                // Record success
                let mut successes = counter.lock().await;
                *successes += 1;
                
                Ok::<_, CoreError>(3)
            }).await;
            
            result
        }
    });
    
    // Give it time to queue
    sleep(Duration::from_millis(50)).await;
    
    // This one should be rejected (queue full)
    let result = bulkhead.execute(async {
        let mut successes = success_counter.lock().await;
        *successes += 1;
        Ok::<_, CoreError>(4)
    }).await;
    
    // Should be rejected
    assert!(result.is_err(), "Request should be rejected when bulkhead queue is full");
    assert!(
        format!("{:?}", result.unwrap_err()).contains("Bulkhead"),
        "Error should indicate bulkhead rejection"
    );
    
    // Wait for all tasks to complete
    let _ = tokio::time::timeout(Duration::from_secs(1), join_all(vec![task1, task2, task3])).await;
    
    // Verify maximum concurrency
    let max_seen = *max_concurrent.lock().await;
    assert!(max_seen <= 2, "Maximum concurrency should be respected");
    
    // Verify success count (tasks 1, 2, and 3 should succeed, task 4 rejected)
    let successes = *success_counter.lock().await;
    assert_eq!(successes, 3, "Exactly 3 tasks should succeed");
    
    // Check bulkhead stats
    let stats = bulkhead.get_stats().await;
    assert_eq!(stats.id, "test_bulkhead", "Stats should report correct ID");
    assert_eq!(stats.max_concurrent, 2, "Stats should report correct max concurrency");
    assert_eq!(stats.active_count, 0, "No active calls should remain");
}

#[tokio::test]
async fn test_bulkhead_stats() {
    // Configure a bulkhead
    let config = BulkheadConfig {
        max_concurrent_calls: 3,
        max_queue_size: 2,
    };
    
    let bulkhead = Bulkhead::new("test_stats_bulkhead".to_string(), config);
    
    // Initially all stats should be zeroed
    let initial_stats = bulkhead.get_stats().await;
    assert_eq!(initial_stats.active_count, 0, "Initial active count should be 0");
    assert_eq!(initial_stats.max_concurrent, 3, "Max concurrent should be 3");
    assert_eq!(initial_stats.queue_size, 0, "Initial queue size should be 0");
    assert_eq!(initial_stats.max_queue_size, 2, "Max queue size should be 2");
    
    // Start some tasks to populate the bulkhead
    let task_handles = vec![
        tokio::spawn({
            let bulkhead = bulkhead.clone();
            async move {
                bulkhead.execute(async {
                    sleep(Duration::from_millis(300)).await;
                    Ok::<_, CoreError>(json!({"id": 1}))
                }).await
            }
        }),
        tokio::spawn({
            let bulkhead = bulkhead.clone();
            async move {
                bulkhead.execute(async {
                    sleep(Duration::from_millis(300)).await;
                    Ok::<_, CoreError>(json!({"id": 2}))
                }).await
            }
        })
    ];
    
    // Give them time to start
    sleep(Duration::from_millis(50)).await;
    
    // Check stats while they're running
    let mid_stats = bulkhead.get_stats().await;
    assert_eq!(mid_stats.active_count, 2, "Two tasks should be active");
    assert_eq!(mid_stats.queue_size, 0, "Queue should be empty");
    
    // Wait for tasks to complete
    let _ = tokio::time::timeout(Duration::from_secs(1), join_all(task_handles)).await;
    
    // Check final stats
    let final_stats = bulkhead.get_stats().await;
    assert_eq!(final_stats.active_count, 0, "No tasks should be active at the end");
    assert_eq!(final_stats.queue_size, 0, "Queue should be empty at the end");
}

#[tokio::test]
async fn test_bulkhead_queue_execution() {
    // Configure a bulkhead with max 1 concurrent call and a queue size of 3
    let config = BulkheadConfig {
        max_concurrent_calls: 1,
        max_queue_size: 3,
    };
    
    let bulkhead = Bulkhead::new("test_queue_bulkhead".to_string(), config);
    
    // Track execution order
    let execution_order = Arc::new(Mutex::new(Vec::new()));
    
    // Start a long-running task that will occupy the bulkhead
    let task1 = tokio::spawn({
        let bulkhead = bulkhead.clone();
        let order = execution_order.clone();
        
        async move {
            bulkhead.execute(async {
                // Record that we're executing first
                {
                    let mut exec_order = order.lock().await;
                    exec_order.push(1);
                }
                
                // Run for a while to block others
                sleep(Duration::from_millis(200)).await;
                
                Ok::<_, CoreError>(1)
            }).await
        }
    });
    
    // Give it time to start
    sleep(Duration::from_millis(50)).await;
    
    // Queue up some more tasks
    let task2 = tokio::spawn({
        let bulkhead = bulkhead.clone();
        let order = execution_order.clone();
        
        async move {
            bulkhead.execute(async {
                {
                    let mut exec_order = order.lock().await;
                    exec_order.push(2);
                }
                
                sleep(Duration::from_millis(50)).await;
                
                Ok::<_, CoreError>(2)
            }).await
        }
    });
    
    let task3 = tokio::spawn({
        let bulkhead = bulkhead.clone();
        let order = execution_order.clone();
        
        async move {
            bulkhead.execute(async {
                {
                    let mut exec_order = order.lock().await;
                    exec_order.push(3);
                }
                
                sleep(Duration::from_millis(50)).await;
                
                Ok::<_, CoreError>(3)
            }).await
        }
    });
    
    // Wait for all tasks to complete
    let _ = tokio::time::timeout(Duration::from_secs(1), join_all(vec![task1, task2, task3])).await;
    
    // Check execution order
    let order = execution_order.lock().await;
    assert_eq!(order.len(), 3, "All tasks should have executed");
    assert_eq!(order[0], 1, "Task 1 should execute first");
    
    // Tasks 2 and 3 should execute after task 1, but we can't guarantee their relative order
    assert!(order.contains(&2), "Task 2 should have executed");
    assert!(order.contains(&3), "Task 3 should have executed");
} 
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use std::future::Future;
use std::pin::Pin;
use tokio::time;
use cascade_server::resilience::idempotency::{IdempotencyConfig, IdempotencyHandler};
use cascade_server::shared_state::InMemorySharedStateService;

// Inlined helper function
fn create_shared_state() -> Arc<InMemorySharedStateService> {
    Arc::new(InMemorySharedStateService::new())
}

/// Test the basic idempotency functionality with a simple operation
#[tokio::test]
async fn test_idempotency_basic() {
    // Setup
    let shared_state = create_shared_state();
    let config = IdempotencyConfig::default();
    let handler = IdempotencyHandler::new(config, shared_state);
    
    // Create a counter to track executions
    let counter = Arc::new(AtomicUsize::new(0));
    
    // Define a function that returns a successful operation
    let create_operation1 = {
        let counter = counter.clone();
        move || {
            let counter = counter.clone();
            Box::pin(async move {
                // Increment counter
                counter.fetch_add(1, Ordering::SeqCst);
                Ok::<String, String>("success".to_string())
            }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        }
    };
    
    // Execute operation with idempotency
    let key = "test-key-1";
    let result1 = handler.execute_test_string(key, create_operation1).await;
    
    // Verify operation was executed
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), "success");
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    
    // Define another function for the second call
    let create_operation2 = {
        let counter = counter.clone();
        move || {
            let counter = counter.clone();
            Box::pin(async move {
                // Increment counter
                counter.fetch_add(1, Ordering::SeqCst);
                Ok::<String, String>("success".to_string())
            }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        }
    };
    
    // Execute again with same key
    let result2 = handler.execute_test_string(key, create_operation2).await;
    
    // Verify cached result is returned without executing the operation again
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap(), "success");
    assert_eq!(counter.load(Ordering::SeqCst), 1); // Still 1
}

/// Test idempotency with a successful operation
#[tokio::test]
async fn test_idempotency_success() {
    let shared_state = create_shared_state();
    
    let config = IdempotencyConfig {
        ttl_ms: 60000,
        shared_state_scope: "idempotency-test".to_string(),
        max_result_size_bytes: 1024 * 1024,
    };
    
    let handler = IdempotencyHandler::new(config, shared_state);
    
    let key = "test-key-1";
    let counter = Arc::new(AtomicUsize::new(0));
    
    // For the first call
    let create_operation1 = {
        let counter = counter.clone();
        move || {
            let counter = counter.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok::<String, String>("success-1".to_string())
            }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        }
    };
    
    let result1 = handler.execute_test_string(key, create_operation1).await;
    assert!(result1.is_ok());
    let success_value = result1.unwrap();
    assert_eq!(success_value, "success-1");
    
    // For the second call - creating a new closure with the same functionality
    let create_operation2 = {
        let counter = counter.clone();
        move || {
            let counter = counter.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok::<String, String>("success-1".to_string())
            }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        }
    };
    
    // Second execution should return the cached result
    let result2 = handler.execute_test_string(key, create_operation2).await;
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap(), "success-1");
    
    // Counter should be incremented only once
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

/// Test that errors are also cached appropriately
#[tokio::test]
async fn test_idempotency_error() {
    let shared_state = create_shared_state();
    
    let config = IdempotencyConfig {
        ttl_ms: 60000,
        shared_state_scope: "idempotency-test".to_string(),
        max_result_size_bytes: 1024 * 1024,
    };
    
    let handler = IdempotencyHandler::new(config, shared_state);
    
    let key = "test-key-2";
    let counter = Arc::new(AtomicUsize::new(0));
    
    // For the first call
    let create_operation1 = {
        let counter = counter.clone();
        move || {
            let counter = counter.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err::<String, String>("error".to_string())
            }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        }
    };
    
    let result1 = handler.execute_test_string(key, create_operation1).await;
    assert!(result1.is_err());
    let error_value = result1.unwrap_err();
    assert_eq!(error_value, "error");
    
    // For the second call - creating a new closure with the same functionality
    let create_operation2 = {
        let counter = counter.clone();
        move || {
            let counter = counter.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err::<String, String>("error".to_string())
            }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        }
    };
    
    // Second execution should return the cached error
    let result2 = handler.execute_test_string(key, create_operation2).await;
    assert!(result2.is_err());
    assert_eq!(result2.unwrap_err(), "error");
    
    // Counter should be incremented only once
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

/// Test using different keys leads to separate executions
#[tokio::test]
async fn test_idempotency_with_different_keys() {
    let shared_state = create_shared_state();
    
    let config = IdempotencyConfig {
        ttl_ms: 60000,
        shared_state_scope: "idempotency-test".to_string(),
        max_result_size_bytes: 1024 * 1024,
    };
    
    let handler = IdempotencyHandler::new(config, shared_state);
    
    let counter = Arc::new(AtomicUsize::new(0));
    
    // First key
    let key1 = "test-key-3";
    let operation1 = {
        let counter = counter.clone();
        move || {
            let counter = counter.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok::<String, String>("success-1".to_string())
            }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        }
    };
    
    let result1 = handler.execute_test_string(key1, operation1).await;
    assert!(result1.is_ok());
    let value1 = result1.unwrap();
    assert_eq!(value1, "success-1");
    
    // Second key
    let key2 = "test-key-4";
    let operation2 = {
        let counter = counter.clone();
        move || {
            let counter = counter.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok::<String, String>("success-2".to_string())
            }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        }
    };
    
    let result2 = handler.execute_test_string(key2, operation2).await;
    assert!(result2.is_ok());
    let value2 = result2.unwrap();
    assert_eq!(value2, "success-2");
    
    // Repeat first key - should use cache
    let operation3 = {
        let counter = counter.clone();
        move || {
            let counter = counter.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok::<String, String>("should-not-be-returned".to_string())
            }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        }
    };
    
    let result3 = handler.execute_test_string(key1, operation3).await;
    assert!(result3.is_ok());
    assert_eq!(result3.unwrap(), "success-1");
    
    // Repeat second key - should use cache
    let operation4 = {
        let counter = counter.clone();
        move || {
            let counter = counter.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok::<String, String>("should-not-be-returned".to_string())
            }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        }
    };
    
    let result4 = handler.execute_test_string(key2, operation4).await;
    assert!(result4.is_ok());
    assert_eq!(result4.unwrap(), "success-2");
    
    // Counter should be incremented twice (once for each unique key)
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

/// Test concurrent requests with the same key
#[tokio::test]
async fn test_idempotency_concurrent_requests() {
    let shared_state = create_shared_state();
    
    let config = IdempotencyConfig {
        ttl_ms: 60000,
        shared_state_scope: "idempotency-test".to_string(),
        max_result_size_bytes: 1024 * 1024,
    };
    
    let handler = Arc::new(IdempotencyHandler::new(config, shared_state));
    let key = "test-key-concurrent";
    
    let counter = Arc::new(AtomicUsize::new(0));
    
    // Create multiple tasks that all execute the same operation with the same key
    let num_tasks = 5;
    let mut handles = Vec::with_capacity(num_tasks);
    
    for _ in 0..num_tasks {
        let handler_clone = Arc::clone(&handler);
        let counter_clone = Arc::clone(&counter);
        
        let handle = tokio::spawn(async move {
            let operation = move || {
                let counter = counter_clone.clone();
                Box::pin(async move {
                    // Sleep to simulate work
                    time::sleep(Duration::from_millis(100)).await;
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok::<String, String>("success".to_string())
                }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
            };
            
            handler_clone.execute_test_string(key, operation).await
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        let _ = handle.await.unwrap();
    }
    
    // The operation should only be executed once
    assert_eq!(counter.load(Ordering::SeqCst), 1, 
              "The operation should only be executed once, but was executed {} times", 
              counter.load(Ordering::SeqCst));
}

/// Test custom TTL and expiration
#[tokio::test]
async fn test_idempotency_ttl_expiration() {
    // Setup with short TTL
    let shared_state = create_shared_state();
    let config = IdempotencyConfig {
        ttl_ms: 100, // Very short TTL for testing
        shared_state_scope: "test-idempotency".to_string(),
        max_result_size_bytes: 1024,
    };
    
    let handler = IdempotencyHandler::new(config, shared_state);
    let counter = Arc::new(AtomicUsize::new(0));
    
    // Create an operation
    let counter_clone = counter.clone();
    let create_operation = || {
        let counter = counter_clone.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok("success".to_string()) as Result<String, String>
        }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
    };
    
    // Execute operation
    let key = "test-key-ttl";
    let result1 = handler.execute_test_string(key, create_operation).await;
    assert!(result1.is_ok());
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    
    // Execute again immediately - should use cached result
    let result2 = handler.execute_test_string(key, create_operation).await;
    assert!(result2.is_ok());
    assert_eq!(counter.load(Ordering::SeqCst), 1); // Still 1
    
    // Wait for TTL to expire
    time::sleep(Duration::from_millis(150)).await;
    
    // Execute again after TTL expiration - should execute again
    let result3 = handler.execute_test_string(key, create_operation).await;
    assert!(result3.is_ok());
    assert_eq!(counter.load(Ordering::SeqCst), 2); // Now 2
}

/// Test combining distributed/shared state with idempotency
#[tokio::test]
async fn test_idempotency_distributed() {
    // Create shared state
    let shared_state = create_shared_state();
    
    // Create two handler instances with the same shared state
    let config = IdempotencyConfig {
        ttl_ms: 5000,
        shared_state_scope: "distributed-idempotency-test".to_string(),
        max_result_size_bytes: 1024,
    };
    
    let handler1 = IdempotencyHandler::new(config.clone(), shared_state.clone());
    let handler2 = IdempotencyHandler::new(config, shared_state);
    
    // Create a counter to track executions
    let counter = Arc::new(AtomicUsize::new(0));
    
    // Define operations for both handlers
    let counter_clone1 = counter.clone();
    let operation1 = || {
        let counter = counter_clone1.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok("from-handler-1".to_string()) as Result<String, String>
        }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
    };
    
    let counter_clone2 = counter.clone();
    let operation2 = || {
        let counter = counter_clone2.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok("from-handler-2".to_string()) as Result<String, String>
        }) as Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
    };
    
    // Execute operation with first handler
    let key = "shared-key";
    let result1 = handler1.execute_test_string(key, operation1).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), "from-handler-1");
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    
    // Execute with second handler - should use cached result from first handler
    let result2 = handler2.execute_test_string(key, operation2).await;
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap(), "from-handler-1"); // Note: not "from-handler-2"
    assert_eq!(counter.load(Ordering::SeqCst), 1); // Still 1
} 
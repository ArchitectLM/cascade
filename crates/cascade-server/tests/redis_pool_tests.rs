#![cfg(feature = "redis")]

use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use cascade_server::shared_state::redis::{RedisSharedStateService, RedisPoolConfig};
use cascade_server::shared_state::SharedStateService;
use serde_json::json;

#[tokio::test]
#[ignore] // Requires Redis server running on localhost:6379
async fn test_redis_pool_basic() {
    // Create a Redis shared state service with default pool config
    let redis_url = "redis://127.0.0.1:6379";
    
    // Attempt to connect to Redis
    let service = RedisSharedStateService::new(redis_url);
    if service.is_err() {
        eprintln!("Skipping test_redis_pool_basic - couldn't connect to Redis: {:?}", service.err());
        return;
    }
    
    let service = Arc::new(service.unwrap());
    
    // Set some test data
    let scope = "test_pool";
    let key = "test_key";
    let value = json!({
        "name": "pool test",
        "timestamp": chrono::Utc::now().timestamp()
    });
    
    // Write data
    service.set_state(scope, key, value.clone()).await.unwrap();
    
    // Read data
    let retrieved = service.get_state(scope, key).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap(), value);
    
    // Get metrics to verify pool is working
    let metrics = service.get_metrics().await.unwrap();
    println!("Redis pool metrics: {:?}", metrics);
    
    // Check that metrics include pool information
    assert!(metrics.as_object().unwrap().contains_key("pool"));
}

#[tokio::test]
#[ignore] // Requires Redis server running on localhost:6379
async fn test_redis_pool_concurrent_operations() {
    // Create a Redis shared state service with custom pool config
    let config = RedisPoolConfig {
        max_connections: 5,
        connection_timeout_ms: 1000,
        pool_timeout_ms: 2000,
    };
    
    let redis_url = "redis://127.0.0.1:6379";
    
    // Attempt to connect to Redis
    let service_result = RedisSharedStateService::with_config(redis_url, config);
    if service_result.is_err() {
        eprintln!("Skipping test_redis_pool_concurrent_operations - couldn't connect to Redis: {:?}", service_result.err());
        return;
    }
    
    let service = Arc::new(service_result.unwrap());
    
    // Create multiple concurrent operations
    let mut handles = Vec::new();
    
    for i in 0..20 {
        let service_clone = service.clone();
        let scope = format!("test_pool_{}", i % 5);
        let key = format!("concurrent_key_{}", i);
        
        // Spawn concurrent tasks
        let handle = tokio::spawn(async move {
            // Set some value
            let value = json!({
                "task_id": i,
                "timestamp": chrono::Utc::now().timestamp()
            });
            
            service_clone.set_state(&scope, &key, value.clone()).await.unwrap();
            
            // Add a small delay to create some overlap
            time::sleep(Duration::from_millis(50)).await;
            
            // Get the value back
            let retrieved = service_clone.get_state(&scope, &key).await.unwrap();
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap(), value);
            
            i
        });
        
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Check metrics after all operations
    let metrics = service.get_metrics().await.unwrap();
    println!("Redis pool metrics after concurrent operations: {:?}", metrics);
    
    // Verify pool metrics show activity
    let pool_metrics = metrics.as_object().unwrap().get("pool").unwrap();
    let success_count = pool_metrics.get("success_count").unwrap().as_u64().unwrap();
    assert!(success_count >= 40); // At least 40 successful connection acquisitions (20 sets + 20 gets)
}

#[tokio::test]
#[ignore] // Requires Redis server running on localhost:6379
async fn test_redis_pool_ttl_operations() {
    // Create a Redis shared state service
    let redis_url = "redis://127.0.0.1:6379";
    
    // Attempt to connect to Redis
    let service = RedisSharedStateService::new(redis_url);
    if service.is_err() {
        eprintln!("Skipping test_redis_pool_ttl_operations - couldn't connect to Redis: {:?}", service.err());
        return;
    }
    
    let service = Arc::new(service.unwrap());
    
    // Set a key with TTL
    let scope = "test_pool_ttl";
    let key = "ttl_key";
    let value = json!("temporary value");
    let ttl_ms = 1000; // 1 second
    
    service.set_state_with_ttl(scope, key, value.clone(), ttl_ms).await.unwrap();
    
    // Immediately retrieve the value (should exist)
    let retrieved = service.get_state(scope, key).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap(), value);
    
    // Wait for TTL to expire
    time::sleep(Duration::from_millis(ttl_ms + 500)).await;
    
    // Try to retrieve again (should be gone)
    let retrieved = service.get_state(scope, key).await.unwrap();
    assert!(retrieved.is_none());
}

#[tokio::test]
#[ignore] // Requires Redis server running on localhost:6379
async fn test_redis_pool_health_check() {
    // Create a Redis shared state service
    let redis_url = "redis://127.0.0.1:6379";
    
    // Attempt to connect to Redis
    let service = RedisSharedStateService::new(redis_url);
    if service.is_err() {
        eprintln!("Skipping test_redis_pool_health_check - couldn't connect to Redis: {:?}", service.err());
        return;
    }
    
    let service = Arc::new(service.unwrap());
    
    // Test health check
    let health = service.health_check().await.unwrap();
    assert!(health, "Redis health check should return true when Redis is available");
    
    // Test with invalid Redis connection
    let invalid_service = RedisSharedStateService::new("redis://invalid-host:1234");
    if invalid_service.is_ok() {
        let invalid_service = Arc::new(invalid_service.unwrap());
        let health_result = time::timeout(
            Duration::from_millis(500),
            invalid_service.health_check()
        ).await;
        
        // Either timeout or return error, but not success
        match health_result {
            Ok(Ok(status)) => {
                assert!(!status, "Health check with invalid Redis connection should not succeed");
            },
            _ => {
                // Expected behavior - either timeout or error
            }
        }
    }
} 
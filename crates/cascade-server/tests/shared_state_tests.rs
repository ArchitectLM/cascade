use cascade_server::shared_state::{SharedStateService, InMemorySharedStateService};
use serde_json::json;
use tokio::test;
use std::time::Duration;
use tokio::time;
use std::sync::Arc;

#[test]
async fn test_in_memory_shared_state_basic_operations() {
    // Setup
    let service = InMemorySharedStateService::new();
    
    // Test empty state
    let result = service.get_state("test-scope", "test-key").await.unwrap();
    assert!(result.is_none(), "Initial state should be empty");
    
    // Test setting a value
    let value = json!({"name": "test", "value": 42});
    service.set_state("test-scope", "test-key", value.clone()).await.unwrap();
    
    // Test getting the value back
    let result = service.get_state("test-scope", "test-key").await.unwrap();
    assert!(result.is_some(), "Value should exist after setting");
    assert_eq!(result.unwrap(), value, "Retrieved value should match set value");
    
    // Test listing keys
    let keys = service.list_keys("test-scope").await.unwrap();
    assert_eq!(keys.len(), 1, "Should have one key in the scope");
    assert_eq!(keys[0], "test-key", "Key should match the one we set");
    
    // Test deleting the value
    service.delete_state("test-scope", "test-key").await.unwrap();
    let result = service.get_state("test-scope", "test-key").await.unwrap();
    assert!(result.is_none(), "Value should be gone after deletion");
    
    // Test multiple scopes
    service.set_state("scope1", "key1", json!("value1")).await.unwrap();
    service.set_state("scope2", "key1", json!("value2")).await.unwrap();
    
    // Each scope should have its own independent state
    let result1 = service.get_state("scope1", "key1").await.unwrap().unwrap();
    let result2 = service.get_state("scope2", "key1").await.unwrap().unwrap();
    
    assert_eq!(result1, json!("value1"), "Scope 1 should have its own value");
    assert_eq!(result2, json!("value2"), "Scope 2 should have its own value");
}

#[test]
async fn test_shared_state_concurrent_access() {
    // Setup
    let service = Arc::new(InMemorySharedStateService::new());
    let scope = "concurrent-test";
    let key = "counter";
    
    // Initialize counter to 0
    service.set_state(scope, key, json!(0)).await.unwrap();
    
    // Create multiple tasks that increment the counter
    let tasks: Vec<_> = (0..10).map(|_| {
        let service = service.clone();
        tokio::spawn(async move {
            for _ in 0..100 {
                // Get current value
                let current = service.get_state(scope, key).await.unwrap().unwrap();
                let current_val = current.as_i64().unwrap();
                
                // Increment value
                let new_val = current_val + 1;
                service.set_state(scope, key, json!(new_val)).await.unwrap();
                
                // Small delay to increase chances of race conditions if the implementation is faulty
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            }
        })
    }).collect();
    
    // Wait for all tasks to complete
    for task in tasks {
        task.await.unwrap();
    }
    
    // Check final counter value
    let final_value = service.get_state(scope, key).await.unwrap().unwrap();
    assert_eq!(final_value, json!(1000), "Final counter should reflect all increments");
}

#[test]
async fn test_shared_state_health_check() {
    // Memory implementation should always be healthy
    let service = InMemorySharedStateService::new();
    let result = service.health_check().await.unwrap();
    assert!(result, "In-memory service should report healthy");
}

#[test]
async fn test_in_memory_state_basic() {
    // Create shared state service
    let service = InMemorySharedStateService::new();
    
    // Set a value
    let scope = "test-scope";
    let key = "test-key";
    let value = json!({ "test": "value" });
    
    service.set_state(scope, key, value.clone()).await.unwrap();
    
    // Get the value
    let retrieved = service.get_state(scope, key).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap(), value);
    
    // Get a non-existent value
    let missing = service.get_state(scope, "missing-key").await.unwrap();
    assert!(missing.is_none());
    
    // Delete the value
    service.delete_state(scope, key).await.unwrap();
    
    // Verify it's gone
    let deleted = service.get_state(scope, key).await.unwrap();
    assert!(deleted.is_none());
}

#[test]
async fn test_in_memory_state_list_keys() {
    // Create shared state service
    let service = InMemorySharedStateService::new();
    
    // Set some values in the same scope
    let scope = "test-scope";
    service.set_state(scope, "key1", json!("value1")).await.unwrap();
    service.set_state(scope, "key2", json!("value2")).await.unwrap();
    service.set_state(scope, "key3", json!("value3")).await.unwrap();
    
    // List keys in the scope
    let keys = service.list_keys(scope).await.unwrap();
    assert_eq!(keys.len(), 3);
    assert!(keys.contains(&"key1".to_string()));
    assert!(keys.contains(&"key2".to_string()));
    assert!(keys.contains(&"key3".to_string()));
    
    // List keys in a non-existent scope
    let empty_keys = service.list_keys("missing-scope").await.unwrap();
    assert!(empty_keys.is_empty());
}

#[test]
async fn test_in_memory_state_ttl() {
    // Create shared state service
    let service = InMemorySharedStateService::new();
    
    // Set a value with a short TTL (100ms)
    let scope = "ttl-scope";
    let key = "ttl-key";
    let value = json!({ "expiring": true });
    
    service.set_state_with_ttl(scope, key, value.clone(), 100).await.unwrap();
    
    // Verify the value is there immediately
    let retrieved = service.get_state(scope, key).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap(), value);
    
    // Wait for TTL to expire
    time::sleep(Duration::from_millis(150)).await;
    
    // Verify the value is gone
    let expired = service.get_state(scope, key).await.unwrap();
    assert!(expired.is_none());
}

#[test]
async fn test_in_memory_state_mixed_ttl() {
    // Create shared state service
    let service = InMemorySharedStateService::new();
    
    // Set values with different TTLs
    let scope = "mixed-ttl-scope";
    
    // Permanent value (no TTL)
    service.set_state(scope, "permanent", json!("forever")).await.unwrap();
    
    // Short TTL (100ms)
    service.set_state_with_ttl(scope, "short-ttl", json!("expires-quickly"), 100).await.unwrap();
    
    // Longer TTL (300ms)
    service.set_state_with_ttl(scope, "long-ttl", json!("expires-later"), 300).await.unwrap();
    
    // Verify all values are initially present
    let keys = service.list_keys(scope).await.unwrap();
    assert_eq!(keys.len(), 3);
    
    // Wait for short TTL to expire
    time::sleep(Duration::from_millis(150)).await;
    
    // Verify only permanent and long-ttl remain
    let keys_after_short = service.list_keys(scope).await.unwrap();
    assert_eq!(keys_after_short.len(), 2);
    assert!(keys_after_short.contains(&"permanent".to_string()));
    assert!(keys_after_short.contains(&"long-ttl".to_string()));
    assert!(!keys_after_short.contains(&"short-ttl".to_string()));
    
    // Wait for long TTL to expire
    time::sleep(Duration::from_millis(200)).await;
    
    // Verify only permanent remains
    let keys_after_long = service.list_keys(scope).await.unwrap();
    assert_eq!(keys_after_long.len(), 1);
    assert!(keys_after_long.contains(&"permanent".to_string()));
}

#[test]
async fn test_in_memory_state_cleanup() {
    // Create shared state service
    let service = InMemorySharedStateService::new();
    
    // Add many values with short TTLs to test cleanup
    let scope = "cleanup-scope";
    for i in 0..100 {
        service.set_state_with_ttl(
            scope, 
            &format!("key-{}", i), 
            json!(i), 
            50 // Very short TTL
        ).await.unwrap();
    }
    
    // Add another 50 values in a different scope
    let scope2 = "cleanup-scope-2";
    for i in 0..50 {
        service.set_state_with_ttl(
            scope2, 
            &format!("key-{}", i), 
            json!(i), 
            50 // Very short TTL
        ).await.unwrap();
    }
    
    // Verify keys are initially present
    assert_eq!(service.list_keys(scope).await.unwrap().len(), 100);
    assert_eq!(service.list_keys(scope2).await.unwrap().len(), 50);
    
    // Wait for TTLs to expire
    time::sleep(Duration::from_millis(100)).await;
    
    // Keys should be reported as expired now
    assert_eq!(service.list_keys(scope).await.unwrap().len(), 0);
    assert_eq!(service.list_keys(scope2).await.unwrap().len(), 0);
    
    // Get metrics to verify cleanup status
    let metrics = service.get_metrics().await.unwrap();
    
    // The background cleanup might not have run yet, so we should expect
    // expired_keys to be non-zero until cleanup runs
    println!("Metrics: {}", serde_json::to_string_pretty(&metrics).unwrap());
    
    // Wait a bit longer to ensure cleanup task runs
    time::sleep(Duration::from_secs(1)).await;
    
    // Get metrics again
    let metrics_after_cleanup = service.get_metrics().await.unwrap();
    println!("Metrics after waiting: {}", serde_json::to_string_pretty(&metrics_after_cleanup).unwrap());
    
    // Check that cleanup is running
    assert_eq!(metrics_after_cleanup["is_cleanup_running"], json!(true));
}

#[test]
async fn test_in_memory_state_metrics() {
    // Create shared state service
    let service = InMemorySharedStateService::new();
    
    // Set various values
    service.set_state("scope1", "key1", json!("value1")).await.unwrap();
    service.set_state("scope1", "key2", json!("value2")).await.unwrap();
    service.set_state("scope2", "key1", json!("value3")).await.unwrap();
    service.set_state_with_ttl("scope3", "expiring1", json!("value4"), 500).await.unwrap();
    service.set_state_with_ttl("scope3", "expiring2", json!("value5"), 5000).await.unwrap();
    
    // Get metrics
    let metrics = service.get_metrics().await.unwrap();
    
    // Check general structure and counts
    assert_eq!(metrics["type"], json!("in_memory"));
    assert_eq!(metrics["scopes"], json!(3));
    assert_eq!(metrics["total_keys"], json!(5));
    assert_eq!(metrics["active_keys"], json!(5)); // All are active
    assert_eq!(metrics["expired_keys"], json!(0)); // None expired yet
    
    // Wait for one key to expire
    time::sleep(Duration::from_millis(600)).await;
    
    // Get metrics again
    let updated_metrics = service.get_metrics().await.unwrap();
    
    // Now we should have one expired key
    assert_eq!(updated_metrics["active_keys"], json!(4)); // One less active
    assert_eq!(updated_metrics["expired_keys"], json!(1)); // One expired
    
    // The cleanup task runs every 30 seconds by default, but we don't want to wait that long
    // in tests, so we just verify it works with the metrics
}

// The Redis tests are only compiled when the "redis" feature is enabled
#[cfg(feature = "redis")]
mod redis_tests {
    use super::*;
    use cascade_server::shared_state::redis::RedisSharedStateService;
    
    // This test requires a running Redis instance - only run it when specifically enabled
    #[test]
    #[ignore] // Add an ignore attribute to prevent this from running in normal test runs
    async fn test_redis_shared_state_basic_operations() {
        // You'd typically get this from config in a real app
        let redis_url = "redis://localhost:6379";
        
        // Create Redis service
        let service = RedisSharedStateService::new(redis_url).unwrap();
        
        // Run the same basic operations test as for in-memory
        // First, clean up any existing test data
        service.delete_state("test-scope", "test-key").await.unwrap();
        
        // Test empty state
        let result = service.get_state("test-scope", "test-key").await.unwrap();
        assert!(result.is_none(), "Initial state should be empty");
        
        // Test setting a value
        let value = json!({"name": "test", "value": 42});
        service.set_state("test-scope", "test-key", value.clone()).await.unwrap();
        
        // Test getting the value back
        let result = service.get_state("test-scope", "test-key").await.unwrap();
        assert!(result.is_some(), "Value should exist after setting");
        assert_eq!(result.unwrap(), value, "Retrieved value should match set value");
        
        // Test listing keys
        let keys = service.list_keys("test-scope").await.unwrap();
        assert!(keys.contains(&"test-key".to_string()), "Key should be in the list");
        
        // Test deleting the value
        service.delete_state("test-scope", "test-key").await.unwrap();
        let result = service.get_state("test-scope", "test-key").await.unwrap();
        assert!(result.is_none(), "Value should be gone after deletion");
    }

    #[test]
    #[ignore] // Requires a Redis server, so we ignore by default
    async fn test_redis_state_ttl() {
        // This test requires a Redis server running at localhost:6379
        let redis_url = "redis://127.0.0.1:6379/";
        let service = RedisSharedStateService::new(redis_url).unwrap();
        
        // Set a value with TTL
        let scope = "redis-ttl-test";
        let key = "ttl-key";
        let value = json!({ "redis_ttl": true });
        
        service.set_state_with_ttl(scope, key, value.clone(), 1000).await.unwrap();
        
        // Verify it's there
        let retrieved = service.get_state(scope, key).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), value);
        
        // Wait for TTL to expire
        time::sleep(Duration::from_millis(1100)).await;
        
        // Verify it's gone
        let expired = service.get_state(scope, key).await.unwrap();
        assert!(expired.is_none());
    }
} 
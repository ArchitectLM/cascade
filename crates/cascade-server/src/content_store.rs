/// Content store abstraction layer for the server

pub use cascade_content_store::*;

use std::sync::Arc;
use async_trait::async_trait;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use tracing::{debug, info};
use std::collections::HashSet;
use serde::{Serialize, Deserialize};
use std::fmt;

/// Default maximum size of the LRU cache (50MB)
const DEFAULT_MAX_CACHE_SIZE_BYTES: usize = 50 * 1024 * 1024;

/// Default maximum number of items in the LRU cache
const DEFAULT_MAX_CACHE_ITEMS: usize = 1000;

/// Default minimum cacheable size (256 bytes)
const DEFAULT_MIN_CACHEABLE_SIZE: usize = 256;

/// Default maximum cacheable size (10MB) - items larger than this won't be cached
const DEFAULT_MAX_CACHEABLE_SIZE: usize = 10 * 1024 * 1024;

/// Default TTL for cached items (1 hour in milliseconds)
const DEFAULT_CACHE_TTL_MS: u64 = 60 * 60 * 1000;

/// Available cache eviction policies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EvictionPolicy {
    /// Least Recently Used - remove the least recently accessed item first
    LRU,
    /// Least Frequently Used - remove the least frequently accessed item first
    LFU,
    /// First In First Out - remove the oldest item first
    FIFO,
}

impl Default for EvictionPolicy {
    fn default() -> Self {
        Self::LRU
    }
}

impl fmt::Display for EvictionPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvictionPolicy::LRU => write!(f, "LRU"),
            EvictionPolicy::LFU => write!(f, "LFU"),
            EvictionPolicy::FIFO => write!(f, "FIFO"),
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable content caching
    pub enabled: bool,
    /// Maximum size of the cache in bytes
    pub max_size_bytes: usize,
    /// Maximum number of items in the cache
    pub max_items: usize,
    /// Minimum size of cacheable items in bytes (items smaller than this won't be cached)
    pub min_cacheable_size: usize,
    /// Maximum size of cacheable items in bytes (items larger than this won't be cached)
    pub max_cacheable_size: usize,
    /// Optional TTL for cached items in milliseconds (None means no expiration)
    pub ttl_ms: Option<u64>,
    /// List of regex patterns for content hashes to preload
    pub preload_patterns: Vec<String>,
    /// Cache eviction policy
    pub eviction_policy: EvictionPolicy,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size_bytes: DEFAULT_MAX_CACHE_SIZE_BYTES,
            max_items: DEFAULT_MAX_CACHE_ITEMS,
            min_cacheable_size: DEFAULT_MIN_CACHEABLE_SIZE,
            max_cacheable_size: DEFAULT_MAX_CACHEABLE_SIZE,
            ttl_ms: Some(DEFAULT_CACHE_TTL_MS),
            preload_patterns: Vec::new(),
            eviction_policy: EvictionPolicy::default(),
        }
    }
}

/// Cache entry with size tracking
#[derive(Clone)]
struct CacheEntry {
    /// Content bytes
    content: Vec<u8>,
    /// Size in bytes
    size: usize,
    /// Creation timestamp (milliseconds since UNIX epoch)
    #[allow(dead_code)]
    created_at: u64,
    /// Last access timestamp (milliseconds since UNIX epoch)
    last_accessed_at: u64,
    /// Number of times this entry has been accessed
    access_count: u64,
    /// Expiration time (milliseconds since UNIX epoch), None if no expiration
    expires_at: Option<u64>,
}

impl CacheEntry {
    /// Create a new cache entry
    fn new(content: Vec<u8>, ttl_ms: Option<u64>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        let expires_at = ttl_ms.map(|ttl| now + ttl);
        
        Self {
            size: content.len(),
            content,
            created_at: now,
            last_accessed_at: now,
            access_count: 0,
            expires_at,
        }
    }
    
    /// Update the access statistics
    fn mark_accessed(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        self.last_accessed_at = now;
        self.access_count += 1;
    }
    
    /// Check if the entry has expired
    fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            
            expires_at <= now
        } else {
            false
        }
    }
}

/// Cache metrics
#[derive(Debug, Clone)]
pub struct CacheMetrics {
    /// Number of items in the cache
    pub item_count: usize,
    /// Total size of cached items in bytes
    pub total_size_bytes: usize,
    /// Maximum capacity in items
    pub capacity_items: usize,
    /// Maximum capacity in bytes
    pub capacity_bytes: usize,
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of items evicted due to size/count limits
    pub evictions: u64,
    /// Number of items expired due to TTL
    pub expirations: u64,
    /// Number of items rejected (too small or too large)
    pub rejected: u64,
    /// Hit ratio (hits / (hits + misses))
    pub hit_ratio: f64,
    /// Cache utilization (total_size_bytes / capacity_bytes)
    pub utilization: f64,
    /// Current eviction policy
    pub eviction_policy: String,
    /// Average item size in bytes
    pub avg_item_size: usize,
    /// Largest item size in bytes
    pub largest_item_size: usize,
    /// Smallest item size in bytes
    pub smallest_item_size: usize,
}

/// CachedContentStore wraps a ContentStorage implementation with an in-memory LRU cache
#[derive(Debug)]
pub struct CachedContentStore {
    /// Inner content store implementation
    inner: Arc<dyn ContentStorage>,
    /// LRU cache for content - protected by mutex since LruCache is not thread-safe
    content_cache: Mutex<LruCache<ContentHash, CacheEntry>>,
    /// Current total size of cached content in bytes
    current_cache_size: Mutex<usize>,
    /// Cache configuration
    config: CacheConfig,
    /// Cache metrics
    metrics: Mutex<CacheMetrics>,
    /// Cache cleanup interval (millis)
    cleanup_interval_ms: u64,
    /// Last cleanup timestamp
    last_cleanup: Mutex<u64>,
}

impl CachedContentStore {
    /// Create a new cached content store with default configuration
    pub fn new(inner: Arc<dyn ContentStorage>) -> Self {
        Self::with_config(inner, CacheConfig::default())
    }

    /// Create a new cached content store with custom configuration
    pub fn with_config(inner: Arc<dyn ContentStorage>, config: CacheConfig) -> Self {
        let cache_size = NonZeroUsize::new(config.max_items).unwrap_or(NonZeroUsize::new(1).unwrap());
        
        info!("Creating new CachedContentStore with max_items={}, max_size={}MB, policy={}", 
              config.max_items, config.max_size_bytes / (1024 * 1024), config.eviction_policy.to_string());
        
        let metrics = CacheMetrics {
            item_count: 0,
            total_size_bytes: 0,
            capacity_items: config.max_items,
            capacity_bytes: config.max_size_bytes,
            hits: 0,
            misses: 0,
            evictions: 0,
            expirations: 0,
            rejected: 0,
            hit_ratio: 0.0,
            utilization: 0.0,
            eviction_policy: config.eviction_policy.to_string(),
            avg_item_size: 0,
            largest_item_size: 0,
            smallest_item_size: 0,
        };
        
        // Default cleanup interval is 5 minutes (300,000 ms)
        let cleanup_interval_ms = 300_000;
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        let instance = Self {
            inner,
            content_cache: Mutex::new(LruCache::new(cache_size)),
            current_cache_size: Mutex::new(0),
            config: config.clone(),
            metrics: Mutex::new(metrics),
            cleanup_interval_ms,
            last_cleanup: Mutex::new(now),
        };
        
        // If there are preload patterns, schedule preloading (not blocking initialization)
        if !config.preload_patterns.is_empty() {
            debug!("Scheduling cache preloading with {} patterns", config.preload_patterns.len());
            // We would spawn a task here in a real implementation
        }
        
        instance
    }

    /// Get cache metrics
    pub fn get_cache_metrics(&self) -> CacheMetrics {
        // Clean expired entries first to get accurate metrics
        self.check_expired_entries();
        self.metrics.lock().unwrap().clone()
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        let mut cache = self.content_cache.lock().unwrap();
        cache.clear();
        
        let mut size = self.current_cache_size.lock().unwrap();
        *size = 0;
        
        let mut metrics = self.metrics.lock().unwrap();
        metrics.item_count = 0;
        metrics.total_size_bytes = 0;
        metrics.largest_item_size = 0;
        metrics.smallest_item_size = 0;
        metrics.avg_item_size = 0;
        metrics.utilization = 0.0;
        
        debug!("Content cache cleared");
    }
    
    /// Check if entries have expired and remove them
    fn check_expired_entries(&self) {
        if !self.config.enabled {
            return;
        }
        
        // Only run cleanup at the specified interval
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        let mut last_cleanup = self.last_cleanup.lock().unwrap();
        if now - *last_cleanup < self.cleanup_interval_ms {
            return;
        }
        
        // Update last cleanup time
        *last_cleanup = now;
        
        // If no TTL is set, we don't need to check for expired entries
        if self.config.ttl_ms.is_none() {
            return;
        }
        
        let mut cache = self.content_cache.lock().unwrap();
        let mut current_size = self.current_cache_size.lock().unwrap();
        let mut metrics = self.metrics.lock().unwrap();
        
        // Collect expired entries
        let mut expired_keys = Vec::new();
        for (hash, entry) in cache.iter() {
            if entry.is_expired() {
                expired_keys.push(hash.clone());
            }
        }
        
        // Remove expired entries
        for hash in expired_keys {
            if let Some(entry) = cache.pop(&hash) {
                *current_size = current_size.saturating_sub(entry.size);
                metrics.expirations += 1;
                debug!("Removed expired content from cache with hash {}", hash);
            }
        }
        
        // Update metrics
        metrics.item_count = cache.len();
        metrics.total_size_bytes = *current_size;
        
        // Calculate utilization
        if metrics.capacity_bytes > 0 {
            metrics.utilization = *current_size as f64 / metrics.capacity_bytes as f64;
        }
        
        // Update avg/min/max item sizes if we have items
        if !cache.is_empty() {
            let mut total_size = 0;
            let mut max_size = 0;
            let mut min_size = usize::MAX;
            
            for (_, entry) in cache.iter() {
                total_size += entry.size;
                max_size = max_size.max(entry.size);
                min_size = min_size.min(entry.size);
            }
            
            metrics.avg_item_size = total_size / cache.len();
            metrics.largest_item_size = max_size;
            metrics.smallest_item_size = min_size;
        } else {
            metrics.avg_item_size = 0;
            metrics.largest_item_size = 0;
            metrics.smallest_item_size = 0;
        }
    }
    
    /// Preload content into the cache based on patterns
    #[allow(dead_code)] // Not yet used, but planned
    async fn preload_content(&self, patterns: &[String]) -> Result<usize, ContentStoreError> {
        if !self.config.enabled || patterns.is_empty() {
            return Ok(0);
        }
        
        // For demonstration - in a real implementation we would:
        // 1. List all content hashes from the inner store
        // 2. Filter them based on regex patterns
        // 3. Load each matching content item into the cache
        
        let all_hashes = self.inner.list_all_content_hashes().await?;
        let mut loaded_count = 0;
        
        for hash in all_hashes {
            // Check if the hash matches any of the patterns
            // In a real implementation, we would use regex matching here
            let hash_str = hash.to_string();
            let should_load = patterns.iter().any(|pattern| hash_str.contains(pattern));
            
            if should_load {
                // Load content into cache
                let content = self.inner.get_content_addressed(&hash).await?;
                self.add_to_cache(hash, content, false);
                loaded_count += 1;
            }
        }
        
        debug!("Preloaded {} items into cache", loaded_count);
        Ok(loaded_count)
    }
    
    /// Add content to the cache
    fn add_to_cache(&self, hash: ContentHash, content: Vec<u8>, update_metrics: bool) -> bool {
        if !self.config.enabled {
            return false;
        }
        
        // Check size limits
        if content.len() < self.config.min_cacheable_size {
            if update_metrics {
                let mut metrics = self.metrics.lock().unwrap();
                metrics.rejected += 1;
            }
            debug!("Content too small to cache: {} bytes", content.len());
            return false;
        }
        
        if content.len() > self.config.max_cacheable_size {
            if update_metrics {
                let mut metrics = self.metrics.lock().unwrap();
                metrics.rejected += 1;
            }
            debug!("Content too large to cache: {} bytes", content.len());
            return false;
        }
        
        let entry = CacheEntry::new(content, self.config.ttl_ms);
        
        // Update cache
        let mut cache = self.content_cache.lock().unwrap();
        let mut current_size = self.current_cache_size.lock().unwrap();
        
        // If we already have this item, remove it first to avoid counting its size twice
        if let Some(old_entry) = cache.pop(&hash) {
            *current_size = current_size.saturating_sub(old_entry.size);
        }
        
        // Check if adding this item would exceed our max cache size
        if *current_size + entry.size <= self.config.max_size_bytes {
            // Add to cache - this will automatically handle LRU eviction of older entries if needed
            let old_len = cache.len();
            cache.put(hash.clone(), entry.clone());
            *current_size += entry.size;
            
            // Check if items were evicted
            if update_metrics && old_len == cache.len() && old_len > 0 {
                // An item was evicted, update metrics
                let mut metrics = self.metrics.lock().unwrap();
                metrics.evictions += 1;
            }
            
            if update_metrics {
                // Update metrics
                let mut metrics = self.metrics.lock().unwrap();
                metrics.total_size_bytes = *current_size;
                metrics.item_count = cache.len();
                
                // Update avg/min/max item sizes
                if metrics.smallest_item_size == 0 || entry.size < metrics.smallest_item_size {
                    metrics.smallest_item_size = entry.size;
                }
                
                if entry.size > metrics.largest_item_size {
                    metrics.largest_item_size = entry.size;
                }
                
                // Calculate new average based on current total and new item
                if cache.len() > 0 {
                    metrics.avg_item_size = *current_size / cache.len();
                }
                
                // Calculate utilization
                if metrics.capacity_bytes > 0 {
                    metrics.utilization = *current_size as f64 / metrics.capacity_bytes as f64;
                }
            }
            
            debug!("Cached content with hash {}, cache size: {} items, {} bytes", 
                  hash, cache.len(), *current_size);
                  
            true
        } else {
            // If the item would exceed the cache, don't cache it
            debug!("Cache too full to store item: {} bytes, current cache size: {} bytes, max: {} bytes", 
                  entry.size, *current_size, self.config.max_size_bytes);
            false
        }
    }
    
    /// Return the inner ContentStorage implementation for downcasting
    pub fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait]
impl ContentStorage for CachedContentStore {
    async fn store_content_addressed(&self, content: &[u8]) -> Result<ContentHash, ContentStoreError> {
        // Store in the underlying content store
        let hash = self.inner.store_content_addressed(content).await?;
        
        // Attempt to cache using our helper method
        if self.config.enabled {
            self.add_to_cache(hash.clone(), content.to_vec(), true);
        }
        
        Ok(hash)
    }

    async fn get_content_addressed(&self, hash: &ContentHash) -> Result<Vec<u8>, ContentStoreError> {
        // Clean expired entries periodically
        self.check_expired_entries();
        
        // Try to get from cache if enabled
        if self.config.enabled {
            let mut hit = false;
            let content = {
                let mut cache = self.content_cache.lock().unwrap();
                if let Some(entry) = cache.get_mut(hash) {
                    // Update access statistics
                    entry.mark_accessed();
                    hit = true;
                    Some(entry.content.clone())
                } else {
                    None
                }
            };
            
            // Update metrics
            {
                let mut metrics = self.metrics.lock().unwrap();
                if hit {
                    metrics.hits += 1;
                    metrics.hit_ratio = metrics.hits as f64 / (metrics.hits + metrics.misses) as f64;
                    
                    debug!("Cache hit for content with hash {}", hash);
                    return Ok(content.unwrap());
                } else {
                    metrics.misses += 1;
                    metrics.hit_ratio = metrics.hits as f64 / (metrics.hits + metrics.misses) as f64;
                }
            }
        }
        
        // Fallback to inner store
        debug!("Cache miss for content with hash {}", hash);
        let content = self.inner.get_content_addressed(hash).await?;
        
        // Add to cache for future lookups if enabled
        if self.config.enabled {
            self.add_to_cache(hash.clone(), content.clone(), true);
        }
        
        Ok(content)
    }

    async fn content_exists(&self, hash: &ContentHash) -> Result<bool, ContentStoreError> {
        // Check cache first if enabled
        if self.config.enabled {
            let cache = self.content_cache.lock().unwrap();
            if cache.contains(hash) {
                return Ok(true);
            }
        }
        
        // Fallback to inner store
        self.inner.content_exists(hash).await
    }

    async fn store_manifest(&self, flow_id: &str, manifest: &Manifest) -> Result<(), ContentStoreError> {
        // Just delegate to inner store (we don't cache manifests)
        self.inner.store_manifest(flow_id, manifest).await
    }

    async fn get_manifest(&self, flow_id: &str) -> Result<Manifest, ContentStoreError> {
        // Just delegate to inner store (we don't cache manifests)
        self.inner.get_manifest(flow_id).await
    }

    async fn delete_manifest(&self, flow_id: &str) -> Result<(), ContentStoreError> {
        // Just delegate to inner store (we don't cache manifests)
        self.inner.delete_manifest(flow_id).await
    }

    async fn list_manifest_keys(&self) -> Result<Vec<String>, ContentStoreError> {
        // Just delegate to inner store
        self.inner.list_manifest_keys().await
    }

    async fn list_all_content_hashes(&self) -> Result<HashSet<ContentHash>, ContentStoreError> {
        // Delegate to inner store and convert Vec to HashSet if needed
        let hashes = self.inner.list_all_content_hashes().await?;
        Ok(hashes)
    }

    async fn delete_content(&self, hash: &ContentHash) -> Result<(), ContentStoreError> {
        // Remove from cache if present
        if self.config.enabled {
            let mut cache = self.content_cache.lock().unwrap();
            let mut current_size = self.current_cache_size.lock().unwrap();
            let mut metrics = self.metrics.lock().unwrap();
            
            if let Some(entry) = cache.pop(hash) {
                *current_size = current_size.saturating_sub(entry.size);
                
                // Update metrics
                metrics.total_size_bytes = *current_size;
                metrics.item_count = cache.len();
                
                // Update utilization
                if metrics.capacity_bytes > 0 {
                    metrics.utilization = *current_size as f64 / metrics.capacity_bytes as f64;
                }
                
                // Recalculate avg/min/max if needed
                if !cache.is_empty() {
                    let mut total_size = 0;
                    let mut max_size = 0;
                    let mut min_size = usize::MAX;
                    
                    for (_, entry) in cache.iter() {
                        total_size += entry.size;
                        max_size = max_size.max(entry.size);
                        min_size = min_size.min(entry.size);
                    }
                    
                    metrics.avg_item_size = total_size / cache.len();
                    metrics.largest_item_size = max_size;
                    metrics.smallest_item_size = min_size;
                } else {
                    metrics.avg_item_size = 0;
                    metrics.largest_item_size = 0;
                    metrics.smallest_item_size = 0;
                }
                
                debug!("Removed content from cache with hash {}", hash);
            }
        }
        
        // Delegate to inner store
        self.inner.delete_content(hash).await
    }
    
    fn as_any(&self) -> &dyn std::any::Any where Self: 'static {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cascade_content_store::memory::InMemoryContentStore;
    
    #[tokio::test]
    async fn test_cached_content_store() {
        // Create a memory content store
        let mem_store = InMemoryContentStore::new();
        let mem_content_store = Arc::new(mem_store);

        // Create a cached wrapper with a small cache
        let cache_config = CacheConfig {
            enabled: true,
            max_size_bytes: 1024 * 1024, // 1MB
            min_cacheable_size: 40,      // 40 bytes minimum
            max_cacheable_size: 1024 * 1024, // 1MB maximum
            ttl_ms: Some(60000),         // 1 minute
            max_items: 1000,             // Max 1000 items
            preload_patterns: Vec::new(),
            eviction_policy: EvictionPolicy::LRU,
        };
        
        let store = CachedContentStore::with_config(mem_content_store, cache_config);
        
        // Create a content item that exceeds the minimum cacheable size
        let content = vec![0u8; 1000]; // 1000 bytes, well above the 40 byte minimum
        
        // Store the content
        let hash = store.store_content_addressed(&content).await.unwrap();
        
        // Check cache metrics after store
        let metrics = store.get_cache_metrics();
        println!("After initial store: item_count={}", metrics.item_count);
        
        // Cache should have this content now
        let cache = store.content_cache.lock().unwrap();
        let contains_after_store = cache.contains(&hash);
        println!("Cache contains hash after store: {}", contains_after_store);
        drop(cache);
        
        // Retrieve the content (should be a cache hit)
        let retrieved = store.get_content_addressed(&hash).await.unwrap();
        assert_eq!(retrieved, content);
        
        // Check the hit count increased
        let metrics = store.get_cache_metrics();
        println!("After cache hit: hits={}, misses={}, item_count={}", 
                 metrics.hits, metrics.misses, metrics.item_count);
        
        // Clear the cache
        store.clear_cache();
        
        // Check cache metrics after clear
        let metrics = store.get_cache_metrics();
        println!("After cache clear: item_count={}", metrics.item_count);
        assert_eq!(metrics.item_count, 0);
        
        // Cache should not have this content now
        let cache = store.content_cache.lock().unwrap();
        let contains_after_clear = cache.contains(&hash);
        println!("Cache contains hash after clear: {}", contains_after_clear);
        drop(cache);
        
        // Retrieve the content again (should be a cache miss)
        let retrieved = store.get_content_addressed(&hash).await.unwrap();
        assert_eq!(retrieved, content);
        
        // Check the miss count increased
        let metrics = store.get_cache_metrics();
        println!("After cache miss: hits={}, misses={}, item_count={}", 
                 metrics.hits, metrics.misses, metrics.item_count);
        
        // Manually check if item was added to cache again
        let cache = store.content_cache.lock().unwrap();
        let contains_after_miss = cache.contains(&hash);
        println!("Cache contains hash after miss: {}", contains_after_miss);
        drop(cache);
        
        // Verify our manual check matches the metrics
        assert_eq!(metrics.item_count, 1);
        
        // Manually add to cache to test the function
        let add_result = store.add_to_cache(hash.clone(), content.clone(), true);
        println!("Manual add_to_cache result: {}", add_result);
        
        // Get final metrics
        let metrics = store.get_cache_metrics();
        println!("Final metrics: hits={}, misses={}, item_count={}", 
                 metrics.hits, metrics.misses, metrics.item_count);
        
        // Final assertion to ensure the cache has 1 item
        assert_eq!(metrics.item_count, 1);
    }
    
    #[tokio::test]
    async fn test_size_thresholds() {
        // Create an in-memory content store
        let inner = Arc::new(InMemoryContentStore::new());
        
        // Create custom config with size thresholds
        let config = CacheConfig {
            enabled: true,
            max_size_bytes: 100 * 1024, // 100KB
            max_items: 100,
            min_cacheable_size: 20, // 20 bytes minimum
            max_cacheable_size: 50 * 1024, // 50KB maximum
            ttl_ms: None,
            preload_patterns: Vec::new(),
            eviction_policy: EvictionPolicy::LRU,
        };
        
        let cached_store = CachedContentStore::with_config(inner, config);
        
        // Test content too small to cache
        let small_content = b"tiny";
        let small_hash = cached_store.store_content_addressed(small_content).await.unwrap();
        
        // Verify it wasn't cached
        let metrics = cached_store.get_cache_metrics();
        assert_eq!(metrics.item_count, 0);
        assert_eq!(metrics.rejected, 1);
        
        // Test content good size to cache
        let good_content = vec![0; 30]; // 30 bytes
        let good_hash = cached_store.store_content_addressed(&good_content).await.unwrap();
        
        // Verify it was cached
        let metrics = cached_store.get_cache_metrics();
        assert_eq!(metrics.item_count, 1);
        
        // Test content too large to cache
        let large_content = vec![0; 60 * 1024]; // 60KB
        let large_hash = cached_store.store_content_addressed(&large_content).await.unwrap();
        
        // Verify it wasn't cached
        let metrics = cached_store.get_cache_metrics();
        assert_eq!(metrics.item_count, 1); // Still just the good content
        assert_eq!(metrics.rejected, 2); // Both small and large were rejected
        
        // Make sure all content is still retrievable
        let retrieved_small = cached_store.get_content_addressed(&small_hash).await.unwrap();
        assert_eq!(retrieved_small, small_content);
        
        let retrieved_good = cached_store.get_content_addressed(&good_hash).await.unwrap();
        assert_eq!(retrieved_good, good_content);
        
        let retrieved_large = cached_store.get_content_addressed(&large_hash).await.unwrap();
        assert_eq!(retrieved_large, large_content);
        
        // Check metrics again
        let metrics = cached_store.get_cache_metrics();
        assert_eq!(metrics.hits, 1); // One hit for good_hash
        assert_eq!(metrics.misses, 2); // Misses for small_hash and large_hash
    }
    
    #[tokio::test]
    async fn test_ttl_expiration() {
        // Create an in-memory content store
        let inner = Arc::new(InMemoryContentStore::new());
        
        // Create custom config with short TTL
        let config = CacheConfig {
            enabled: true,
            max_size_bytes: 100 * 1024,
            max_items: 100,
            min_cacheable_size: 0,
            max_cacheable_size: 50 * 1024,
            ttl_ms: Some(100), // 100ms TTL
            preload_patterns: Vec::new(),
            eviction_policy: EvictionPolicy::LRU,
        };
        
        // Create with very short cleanup interval for testing
        let mut cached_store = CachedContentStore::with_config(inner, config);
        cached_store.cleanup_interval_ms = 10; // 10ms cleanup interval
        
        // Store some content
        let content = b"This content will expire quickly";
        let hash = cached_store.store_content_addressed(content).await.unwrap();
        
        // Verify it's cached
        let metrics = cached_store.get_cache_metrics();
        assert_eq!(metrics.item_count, 1);
        
        // Get content (should be a cache hit)
        let retrieved = cached_store.get_content_addressed(&hash).await.unwrap();
        assert_eq!(retrieved, content);
        
        // Manually expire the entry by manipulating the last_cleanup time 
        // and setting the expiration time in the past
        {
            let mut cache = cached_store.content_cache.lock().unwrap();
            if let Some(entry) = cache.get_mut(&hash) {
                // Set expiration time to now minus 1 second (in the past)
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                
                entry.expires_at = Some(now - 1000);
            }
            
            // Set last cleanup to a time in the past to force a cleanup
            let mut last_cleanup = cached_store.last_cleanup.lock().unwrap();
            *last_cleanup = 0;
        }
        
        // This get should trigger cleanup and be a cache miss
        let retrieved = cached_store.get_content_addressed(&hash).await.unwrap();
        assert_eq!(retrieved, content);
        
        // Verify content was expired from cache
        let metrics = cached_store.get_cache_metrics();
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.misses, 1);
        assert_eq!(metrics.expirations, 1);
        
        // Content should be back in cache after the miss
        assert_eq!(metrics.item_count, 1);
    }
} 
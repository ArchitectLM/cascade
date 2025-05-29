//! Configuration for the Cascade Server
//!
//! This module contains the configuration types and loading functionality.

use serde::{Deserialize, Serialize};
use std::env;
use tracing::{info, warn};

use crate::error::{ServerError, ServerResult};
use crate::content_store::{CacheConfig, EvictionPolicy};

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Port to listen on
    #[serde(default = "default_port")]
    pub port: u16,
    
    /// Host to bind to
    #[serde(default = "default_host")]
    pub bind_address: String,
    
    /// URL of the content store
    pub content_store_url: String,
    
    /// URL of the edge platform API
    pub edge_api_url: String,
    
    /// URL of the shared state service
    #[serde(default = "default_shared_state_url")]
    pub shared_state_url: String,
    
    /// Secret key for API auth
    #[serde(default)]
    pub admin_api_key: Option<String>,
    
    /// Secret for JWT signing
    #[serde(default)]
    pub edge_callback_jwt_secret: Option<String>,
    
    /// JWT issuer
    #[serde(default = "default_jwt_issuer")]
    pub edge_callback_jwt_issuer: String,
    
    /// JWT audience
    #[serde(default = "default_jwt_audience")]
    pub edge_callback_jwt_audience: String,
    
    /// JWT expiry in seconds
    #[serde(default = "default_jwt_expiry")]
    pub edge_callback_jwt_expiry_seconds: u64,
    
    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
    
    /// Content cache configuration
    #[serde(default)]
    pub content_cache_config: Option<CacheConfig>,
    
    /// Cloudflare API token (for Cloudflare KV and Workers)
    #[serde(default)]
    pub cloudflare_api_token: Option<String>,
}

fn default_port() -> u16 {
    8080
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_jwt_issuer() -> String {
    "cascade-server".to_string()
}

fn default_jwt_audience() -> String {
    "cascade-edge".to_string()
}

fn default_jwt_expiry() -> u64 {
    3600 // 1 hour
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_shared_state_url() -> String {
    "memory://local".to_string()
}

impl ServerConfig {
    /// Load configuration from environment variables and optional config file
    pub fn load() -> ServerResult<Self> {
        // Start with defaults
        let mut config = Self::default();
        
        // Override from environment variables
        if let Ok(port) = env::var("SERVER_PORT") {
            if let Ok(port) = port.parse::<u16>() {
                config.port = port;
            } else {
                warn!("Invalid SERVER_PORT value: {}", port);
            }
        }
        
        if let Ok(host) = env::var("SERVER_HOST") {
            config.bind_address = host;
        }
        
        if let Ok(content_store_url) = env::var("CONTENT_STORE_URL") {
            config.content_store_url = content_store_url;
        }
        
        if let Ok(edge_api_url) = env::var("EDGE_API_URL") {
            config.edge_api_url = edge_api_url;
        }
        
        if let Ok(shared_state_url) = env::var("SHARED_STATE_URL") {
            config.shared_state_url = shared_state_url;
        }
        
        if let Ok(admin_api_key) = env::var("ADMIN_API_KEY") {
            config.admin_api_key = Some(admin_api_key);
        }
        
        if let Ok(jwt_secret) = env::var("EDGE_CALLBACK_JWT_SECRET") {
            config.edge_callback_jwt_secret = Some(jwt_secret);
        }
        
        if let Ok(jwt_issuer) = env::var("EDGE_CALLBACK_JWT_ISSUER") {
            config.edge_callback_jwt_issuer = jwt_issuer;
        }
        
        if let Ok(jwt_audience) = env::var("EDGE_CALLBACK_JWT_AUDIENCE") {
            config.edge_callback_jwt_audience = jwt_audience;
        }
        
        if let Ok(jwt_expiry) = env::var("EDGE_CALLBACK_JWT_EXPIRY_SECONDS") {
            if let Ok(expiry) = jwt_expiry.parse::<u64>() {
                config.edge_callback_jwt_expiry_seconds = expiry;
            } else {
                warn!("Invalid EDGE_CALLBACK_JWT_EXPIRY_SECONDS value: {}", jwt_expiry);
            }
        }
        
        if let Ok(log_level) = env::var("LOG_LEVEL") {
            config.log_level = log_level;
        }
        
        if let Ok(cloudflare_api_token) = env::var("CLOUDFLARE_API_TOKEN") {
            config.cloudflare_api_token = Some(cloudflare_api_token);
        }
        
        // Content cache configuration from environment
        if let Ok(cache_enabled) = env::var("CONTENT_CACHE_ENABLED") {
            let enabled = cache_enabled.to_lowercase() == "true" || cache_enabled == "1";
            
            if !enabled {
                config.content_cache_config = None;
            } else {
                // Cache is enabled, check for size limits
                let mut cache_config = CacheConfig::default();
                
                if let Ok(max_items) = env::var("CONTENT_CACHE_MAX_ITEMS") {
                    if let Ok(items) = max_items.parse::<usize>() {
                        cache_config.max_items = items;
                    } else {
                        warn!("Invalid CONTENT_CACHE_MAX_ITEMS value: {}", max_items);
                    }
                }
                
                if let Ok(max_size_mb) = env::var("CONTENT_CACHE_MAX_SIZE_MB") {
                    if let Ok(size) = max_size_mb.parse::<usize>() {
                        cache_config.max_size_bytes = size * 1024 * 1024;
                    } else {
                        warn!("Invalid CONTENT_CACHE_MAX_SIZE_MB value: {}", max_size_mb);
                    }
                }
                
                // New configuration options
                if let Ok(min_size) = env::var("CONTENT_CACHE_MIN_SIZE") {
                    if let Ok(size) = min_size.parse::<usize>() {
                        cache_config.min_cacheable_size = size;
                    } else {
                        warn!("Invalid CONTENT_CACHE_MIN_SIZE value: {}", min_size);
                    }
                }
                
                if let Ok(max_size) = env::var("CONTENT_CACHE_MAX_CACHEABLE_SIZE_KB") {
                    if let Ok(size) = max_size.parse::<usize>() {
                        cache_config.max_cacheable_size = size * 1024;
                    } else {
                        warn!("Invalid CONTENT_CACHE_MAX_CACHEABLE_SIZE_KB value: {}", max_size);
                    }
                }
                
                if let Ok(ttl) = env::var("CONTENT_CACHE_TTL_MS") {
                    if ttl.to_lowercase() == "none" {
                        cache_config.ttl_ms = None;
                    } else if let Ok(ms) = ttl.parse::<u64>() {
                        cache_config.ttl_ms = Some(ms);
                    } else {
                        warn!("Invalid CONTENT_CACHE_TTL_MS value: {}", ttl);
                    }
                }
                
                if let Ok(policy) = env::var("CONTENT_CACHE_EVICTION_POLICY") {
                    let policy_str = policy.to_uppercase();
                    cache_config.eviction_policy = match policy_str.as_str() {
                        "LRU" => EvictionPolicy::LRU,
                        "LFU" => EvictionPolicy::LFU,
                        "FIFO" => EvictionPolicy::FIFO,
                        _ => {
                            warn!("Invalid CONTENT_CACHE_EVICTION_POLICY value: {}, using default LRU", policy);
                            EvictionPolicy::LRU
                        }
                    };
                }
                
                if let Ok(patterns) = env::var("CONTENT_CACHE_PRELOAD_PATTERNS") {
                    if !patterns.is_empty() {
                        cache_config.preload_patterns = patterns.split(',')
                            .map(|p| p.trim().to_string())
                            .filter(|p| !p.is_empty())
                            .collect();
                    }
                }
                
                config.content_cache_config = Some(cache_config);
            }
        }
        
        // Validate required fields
        if config.content_store_url.is_empty() {
            return Err(ServerError::ConfigError(
                "Content store URL is required".to_string()
            ));
        }
        
        if config.edge_api_url.is_empty() {
            return Err(ServerError::ConfigError(
                "Edge API URL is required".to_string()
            ));
        }
        
        // Add warnings for missing optional fields
        if config.admin_api_key.is_none() {
            warn!("No ADMIN_API_KEY provided - admin API will be unsecured!");
        }
        
        if config.edge_callback_jwt_secret.is_none() {
            warn!("No EDGE_CALLBACK_JWT_SECRET provided - edge callbacks will be unsecured!");
        }
        
        // Add cloudflare token warning when using cloudflare:// URL
        if config.content_store_url.starts_with("cloudflare://") && config.cloudflare_api_token.is_none() {
            warn!("Using Cloudflare KV store but no CLOUDFLARE_API_TOKEN provided in environment!");
        }
        
        info!("Loaded server configuration");
        Ok(config)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            bind_address: default_host(),
            content_store_url: String::new(),
            edge_api_url: String::new(),
            shared_state_url: default_shared_state_url(),
            admin_api_key: None,
            edge_callback_jwt_secret: None,
            edge_callback_jwt_issuer: default_jwt_issuer(),
            edge_callback_jwt_audience: default_jwt_audience(),
            edge_callback_jwt_expiry_seconds: default_jwt_expiry(),
            log_level: default_log_level(),
            content_cache_config: Some(CacheConfig::default()),
            cloudflare_api_token: None,
        }
    }
} 
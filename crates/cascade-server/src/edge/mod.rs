//! Edge platform integration
//!
//! This module contains the edge platform client and related functionality.

use async_trait::async_trait;
use serde_json::Value;
use std::fmt::Debug;

use crate::error::ServerResult;

/// Interface for edge platform operations
#[async_trait]
pub trait EdgePlatform: Send + Sync + Debug {
    /// Deploy a new worker
    async fn deploy_worker(&self, worker_id: &str, manifest: Value) -> ServerResult<String>;
    
    /// Update an existing worker
    async fn update_worker(&self, worker_id: &str, manifest: Value) -> ServerResult<String>;
    
    /// Delete a worker
    async fn delete_worker(&self, worker_id: &str) -> ServerResult<()>;
    
    /// Check if a worker exists
    async fn worker_exists(&self, worker_id: &str) -> ServerResult<bool>;
    
    /// Get the URL for a deployed worker
    async fn get_worker_url(&self, worker_id: &str) -> ServerResult<Option<String>>;
    
    /// Get health status
    async fn health_check(&self) -> ServerResult<bool>;
}

/// Re-export specific implementations
pub mod cloudflare;

// Alias to the chosen implementation for crate-internal usage
 
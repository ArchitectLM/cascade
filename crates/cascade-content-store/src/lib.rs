//! Cascade Content Store
//! 
//! Provides abstractions and implementations for content-addressed storage.
//! The ContentStorage trait defines a contract for storing/retrieving content-addressed blobs
//! and flow manifests.

use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display};
use std::time::{SystemTime};

/// Represents a content hash, typically "sha256:<hex_digest>"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash(String);

impl ContentHash {
    /// Constructor ensures format (e.g., validates prefix and hex length)
    pub fn new(hash_str: String) -> Result<Self, ContentStoreError> {
        // Basic validation example:
        if !hash_str.starts_with("sha256:") || hash_str.len() != 71 { // sha256: + 64 hex chars
             return Err(ContentStoreError::InvalidHashFormat(hash_str));
        }
        Ok(Self(hash_str))
    }

    /// Create a ContentHash directly from a string without validation
    /// Only use this when you're certain the hash is in the correct format
    #[allow(dead_code)]
    pub(crate) fn new_unchecked(hash_str: String) -> Self {
        Self(hash_str)
    }

    /// Get the string representation of the hash
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to owned String
    pub fn into_string(self) -> String {
        self.0
    }
}

// Implement Display for ContentHash
impl Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents the Manifest document structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub manifest_version: String, // e.g., "1.0"
    pub flow_id: String,
    pub dsl_hash: String, // sha256 hash of the source DSL content for this version
    pub timestamp: u64, // Unix timestamp (ms) of manifest creation
    pub entry_step_id: String,
    pub edge_steps: HashMap<String, EdgeStepInfo>, // Key: step_id
    pub server_steps: Vec<String>, // List of step_ids executed by server
    pub edge_callback_url: Option<String>, // URL for edge worker to call back to server
    pub required_bindings: Vec<String>, // e.g., ["KV_NAMESPACE", "API_TOKEN"] required by worker
    /// Last access time (Unix timestamp in ms)
    #[serde(default = "default_timestamp")]
    pub last_accessed: u64,
}

/// Default timestamp function for last_accessed field
#[inline]
pub fn default_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Information about steps to be executed on the edge
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeStepInfo {
    pub component_type: String, // Logical type, e.g., "StdLib:HttpCall"
    pub component_hash: ContentHash, // Hash of the executable component logic/config
    pub config_hash: Option<ContentHash>, // Hash of the specific config for this step (if distinct)
    pub run_after: Vec<String>, // step_ids this step depends on
    pub inputs_map: HashMap<String, String>, // Input name -> DataReference string
}

/// Garbage collection configuration options
#[derive(Debug, Clone)]
pub struct GarbageCollectionOptions {
    /// Age threshold for inactive manifests (ms)
    pub inactive_manifest_threshold_ms: u64,
    /// Dry run mode - report but don't delete
    pub dry_run: bool,
    /// Maximum number of items to collect in one run
    pub max_items: Option<usize>,
}

impl Default for GarbageCollectionOptions {
    fn default() -> Self {
        Self {
            // Default: 30 days
            inactive_manifest_threshold_ms: 30 * 24 * 60 * 60 * 1000,
            dry_run: false,
            max_items: None,
        }
    }
}

/// Result of a garbage collection operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GarbageCollectionResult {
    /// Number of unused content blobs removed
    pub content_blobs_removed: usize,
    /// Number of inactive manifests removed
    pub manifests_removed: usize,
    /// Total bytes reclaimed
    pub bytes_reclaimed: usize,
    /// Whether this was a dry run (no actual deletions)
    pub dry_run: bool,
    /// Duration of the collection process in milliseconds
    pub duration_ms: u64,
}

/// Errors that can occur during content store operations
#[derive(Error, Debug)]
pub enum ContentStoreError {
    #[error("Storage backend error: {0}")]
    BackendError(#[from] anyhow::Error), // Catch-all for backend-specific issues

    #[error("Content not found for hash: {0}")]
    NotFound(ContentHash),

    #[error("Manifest not found for flow ID: {0}")]
    ManifestNotFound(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error), // Or other specific serialization errors

    #[error("Invalid content hash format: {0}")]
    InvalidHashFormat(String),

    #[error("Operation timed out: {0}")]
    Timeout(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Unexpected error: {0}")]
    Unexpected(String),
    
    #[error("Garbage collection error: {0}")]
    GarbageCollectionError(String),
}

/// Result type for ContentStorage operations
pub type ContentStoreResult<T> = Result<T, ContentStoreError>;

/// Trait defining the contract for content storage implementations
#[async_trait]
pub trait ContentStorage: Send + Sync + std::fmt::Debug {
    /// Stores content and returns a content-addressed hash (sha-256)
    async fn store_content_addressed(&self, content: &[u8]) -> ContentStoreResult<ContentHash>;
    
    /// Retrieve content by its content-addressed hash (sha-256)
    async fn get_content_addressed(&self, hash: &ContentHash) -> ContentStoreResult<Vec<u8>>;
    
    /// Check if content exists by hash
    async fn content_exists(&self, hash: &ContentHash) -> ContentStoreResult<bool>;
    
    /// Store a manifest for a flow
    async fn store_manifest(&self, flow_id: &str, manifest: &Manifest) -> ContentStoreResult<()>;
    
    /// Get a manifest for a flow
    async fn get_manifest(&self, flow_id: &str) -> ContentStoreResult<Manifest>;
    
    /// Delete a manifest for a flow
    async fn delete_manifest(&self, flow_id: &str) -> ContentStoreResult<()>;
    
    /// List all manifest keys (usually flow IDs)
    async fn list_manifest_keys(&self) -> ContentStoreResult<Vec<String>>;
    
    /// List all content hashes
    async fn list_all_content_hashes(&self) -> ContentStoreResult<HashSet<ContentHash>>;
    
    /// Delete content by hash
    async fn delete_content(&self, hash: &ContentHash) -> ContentStoreResult<()>;
    
    /// Runs garbage collection to remove unused content and inactive manifests.
    async fn run_garbage_collection(
        &self,
        _options: GarbageCollectionOptions
    ) -> ContentStoreResult<GarbageCollectionResult> {
        Err(ContentStoreError::Unexpected(
            "run_garbage_collection not implemented for this storage backend".to_string()
        ))
    }
    
    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any where Self: 'static;
}

// Re-export modules so they can be used from other crates
pub mod memory;
pub mod cloudflare; 
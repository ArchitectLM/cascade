//! Main Cascade Server implementation
//!
//! This module contains the CascadeServer implementation.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tracing::{info, warn, error, debug, info_span, Instrument};
use serde::{Serialize, Deserialize};
use dashmap::DashMap;
use chrono::Utc;
use std::fmt::Debug;

use cascade_content_store::{ContentStorage, Manifest, ContentHash, EdgeStepInfo, GarbageCollectionOptions, GarbageCollectionResult, ContentStoreError};
use cascade_core::{ComponentRuntimeApi, DataPacket, ComponentRuntimeApiBase};

use crate::api::admin::FlowSummary;
use crate::api::admin::DeploymentResponse;
use crate::config::ServerConfig;
use crate::edge::EdgePlatform;
use crate::error::{ServerError, ServerResult};
use crate::edge_manager::EdgeManager;

/// JWT claims for edge callbacks
#[derive(Debug, Serialize, Deserialize)]
pub struct EdgeJwtClaims {
    /// Flow ID
    sub: String,
    /// Server ID (issuer)
    iss: String,
    /// Edge worker ID (audience)
    aud: String,
    /// Expiration time
    exp: usize,
    /// Issued at time
    iat: usize,
    /// Flow instance ID if specific to an execution
    #[serde(skip_serializing_if = "Option::is_none")]
    fid: Option<String>,
    /// Allowed steps
    #[serde(skip_serializing_if = "Option::is_none")]
    steps: Option<Vec<String>>,
}

/// Main server implementation
#[derive(Clone)]
pub struct CascadeServer {
    /// Configuration
    pub config: ServerConfig,
    
    /// Content store client
    content_store: Arc<dyn ContentStorage>,
    
    /// Edge platform client
    edge_platform: Arc<dyn EdgePlatform>,
    
    /// Edge manager
    edge_manager: Arc<EdgeManager>,
    
    /// Core runtime API
    core_runtime: Arc<dyn ComponentRuntimeApi>,
    
    /// Server address (might be different from configured if port is 0)
    address: Option<SocketAddr>,
    
    /// Cache for deployed flows - flow_id -> timestamp
    flow_cache: DashMap<String, String>,
    
    /// Cache for content existence checks - hash -> exists
    content_exists_cache: DashMap<String, bool>,
    
    /// Shared state service
    shared_state: Arc<dyn crate::shared_state::SharedStateService>,
}

/// Manual Debug implementation that doesn't try to debug the trait objects
impl std::fmt::Debug for CascadeServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CascadeServer")
            .field("config", &self.config)
            .field("flow_cache_size", &self.flow_cache.len())
            .field("content_cache_size", &self.content_exists_cache.len())
            .finish()
    }
}

impl CascadeServer {
    /// Create a new CascadeServer
    pub fn new(
        config: ServerConfig,
        content_store: Arc<dyn ContentStorage>,
        edge_platform: Arc<dyn EdgePlatform>,
        edge_manager: EdgeManager,
        core_runtime: Arc<dyn ComponentRuntimeApi>,
        shared_state: Arc<dyn crate::shared_state::SharedStateService>,
    ) -> Self {
        Self {
            config,
            content_store,
            edge_platform,
            edge_manager: Arc::new(edge_manager),
            core_runtime,
            address: None,
            flow_cache: DashMap::new(),
            content_exists_cache: DashMap::new(),
            shared_state,
        }
    }
    
    /// For testing: Create a new server instance with mocked dependencies
    #[cfg(test)]
    pub async fn new_with_mocks(
        config: ServerConfig,
        mock_edge_api: Arc<dyn EdgePlatform>,
        mock_content_store: Arc<dyn ContentStorage>,
    ) -> ServerResult<Self> {
        // Create mock shared state service
        let shared_state = Arc::new(crate::shared_state::InMemorySharedStateService::new());
        
        // Create mock core runtime with shared state
        let core_runtime = crate::create_core_runtime(Some(shared_state.clone()))?;
        
        // Create edge manager with mocks
        let edge_manager = EdgeManager::new(
            mock_edge_api.clone(),
            mock_content_store.clone(),
            format!("http://localhost:{}/api/v1/edge/callback", config.port),
            Some("test-jwt-secret".to_string()),
            Some("cascade-server-test".to_string()),
            Some("cascade-edge-test".to_string()),
            3600,
        );
        
        Ok(Self::new(
            config,
            mock_content_store,
            mock_edge_api,
            edge_manager,
            core_runtime,
            shared_state,
        ))
    }
    
    /// Run the server
    pub async fn run(mut self) -> ServerResult<()> {
        info!("Starting Cascade Server");
        
        // Build the API router
        let app = crate::api::build_router(Arc::new(self.clone()));
        
        // Create and bind the TCP listener
        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.port));
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        
        // Store the actual bound address
        self.address = Some(addr);
        info!("Listening on {}", addr);
        
        // Run the server
        axum::serve(listener, app).await?;
        
        Ok(())
    }
    
    /// Get the server's bound address
    pub fn address(&self) -> SocketAddr {
        self.address.unwrap_or_else(|| {
            SocketAddr::from(([127, 0, 0, 1], self.config.port))
        })
    }
    
    /// List all deployed flows
    pub async fn list_flows(&self) -> ServerResult<Vec<FlowSummary>> {
        // Check cache for quick response if we have data
        if !self.flow_cache.is_empty() {
            debug!("Using flow cache for listing flows");
            let mut flows = Vec::with_capacity(self.flow_cache.len());
            
            for entry in self.flow_cache.iter() {
                let flow_id = entry.key().clone();
                let manifest_timestamp = entry.value().clone();
                
                // Fetch edge worker URL (not cached as it could change externally)
                let edge_worker_url = self.edge_platform.get_worker_url(&flow_id).await.ok().and_then(|x| x);
                
                flows.push(FlowSummary {
                    flow_id,
                    manifest_timestamp,
                    edge_worker_url,
                });
            }
            
            return Ok(flows);
        }
        
        // Cache is empty, fetch from content store
        let flow_ids = self.content_store.list_manifest_keys().await
            .map_err(|e| ServerError::ContentStoreError(format!("Failed to list manifests: {}", e)))?;
        
        // For each flow ID, get more details
        let mut flows = Vec::new();
        for flow_id in flow_ids {
            // Get the manifest
            match self.content_store.get_manifest(&flow_id).await {
                Ok(manifest) => {
                    // Update cache
                    self.flow_cache.insert(flow_id.clone(), manifest.timestamp.to_string());
                    
                    // Try to get the edge worker URL if applicable
                    let edge_worker_url = self.edge_platform.get_worker_url(&flow_id).await.ok().and_then(|x| x);
                    
                    flows.push(FlowSummary {
                        flow_id,
                        manifest_timestamp: manifest.timestamp.to_string(),
                        edge_worker_url,
                    });
                },
                Err(err) => {
                    warn!(%flow_id, ?err, "Failed to get manifest while listing flows");
                }
            }
        }
        
        Ok(flows)
    }
    
    /// Get a flow by ID
    pub async fn get_flow(&self, flow_id: &str) -> ServerResult<Value> {
        // Get the manifest
        let manifest = self.content_store.get_manifest(flow_id).await
            .map_err(|err| match err {
                cascade_content_store::ContentStoreError::ManifestNotFound(_) => 
                    ServerError::NotFound(format!("Flow {}", flow_id)),
                _ => ServerError::ContentStoreError(format!("Failed to get manifest: {}", err)),
            })?;
        
        // Update cache with manifest timestamp
        self.flow_cache.insert(flow_id.to_string(), manifest.timestamp.to_string());
        
        // Convert to API response
        let edge_worker_url = self.edge_platform.get_worker_url(flow_id).await.ok().and_then(|x| x);
        
        let response = json!({
            "flow_id": flow_id,
            "manifest_timestamp": manifest.timestamp.to_string(),
            "manifest_version": manifest.manifest_version,
            "edge_steps": manifest.edge_steps.keys().collect::<Vec<_>>(),
            "server_steps": manifest.server_steps,
            "edge_worker_url": edge_worker_url,
        });
        
        Ok(response)
    }
    
    /// Deploy a flow
    pub async fn deploy_flow(&self, flow_id: &str, yaml: &str) -> ServerResult<(DeploymentResponse, bool)> {
        let span = info_span!("deploy_flow", %flow_id);
        async move {
            info!("Deploying flow");
            
            // Optimized check - look up in cache first
            let is_new = if self.flow_cache.get(flow_id).is_some() {
                debug!(%flow_id, "Flow exists in cache");
                false
            } else {
                // Not in cache, check content store directly
                !self.flow_exists(flow_id).await?
            };
            
            // Parse and validate the DSL before making any more expensive operations
            let flow_def = self.parse_and_validate_dsl(yaml)?;
            
            // Create a manifest for the flow
            let manifest = self.create_manifest(flow_id, yaml, flow_def).await?;
            
            // Store the manifest
            info!("Storing manifest");
            self.content_store.store_manifest(flow_id, &manifest).await
                .map_err(|e| ServerError::ContentStoreError(format!("Failed to store manifest: {}", e)))?;
            
            // Update cache with manifest timestamp
            self.flow_cache.insert(flow_id.to_string(), manifest.timestamp.to_string());
            
            // Deploy to edge platform using EdgeManager
            let worker_url = match self.edge_manager.deploy_worker(flow_id, &manifest).await {
                Ok(url) => {
                    info!("Flow deployed to edge platform");
                    Some(url)
                },
                Err(err) => {
                    error!(?err, "Failed to deploy to edge platform");
                    None
                }
            };
            
            // Prepare response
            let response = DeploymentResponse {
                flow_id: flow_id.to_string(),
                status: if is_new { "DEPLOYED".to_string() } else { "UPDATED".to_string() },
                manifest_hash: "sha256:placeholder".to_string(), // A real implementation would hash the manifest
                manifest_timestamp: manifest.timestamp.to_string(),
                edge_worker_url: worker_url,
            };
            
            info!("Flow deployed successfully");
            Ok((response, is_new))
        }.instrument(span).await
    }
    
    /// Undeploy a flow
    pub async fn undeploy_flow(&self, flow_id: &str) -> ServerResult<()> {
        let span = info_span!("undeploy_flow", %flow_id);
        async move {
            info!("Undeploying flow");
            
            // Check if flow exists
            let exists = self.flow_exists(flow_id).await?;
            if !exists {
                return Err(ServerError::NotFound(format!("Flow {} not found", flow_id)));
            }
            
            // Undeploy from edge platform using EdgeManager
            match self.edge_manager.undeploy_worker(flow_id).await {
                Ok(_) => {
                    info!("Flow undeployed from edge platform");
                },
                Err(err) => {
                    warn!(?err, "Failed to undeploy from edge platform");
                }
            }
            
            // Delete manifest from content store
            info!("Deleting manifest");
            self.content_store.delete_manifest(flow_id).await
                .map_err(|e| ServerError::ContentStoreError(format!("Failed to delete manifest: {}", e)))?;
            
            // Remove from cache
            self.flow_cache.remove(flow_id);
            
            info!("Flow undeployed successfully");
            Ok(())
        }.instrument(span).await
    }
    
    /// Get a flow's manifest
    pub async fn get_flow_manifest(&self, flow_id: &str) -> ServerResult<Value> {
        // Get the manifest
        let manifest = self.content_store.get_manifest(flow_id).await
            .map_err(|err| match err {
                cascade_content_store::ContentStoreError::ManifestNotFound(_) => 
                    ServerError::NotFound(format!("Flow {}", flow_id)),
                _ => ServerError::ContentStoreError(format!("Failed to get manifest: {}", err)),
            })?;
        
        // Convert to JSON Value
        let manifest_json = serde_json::to_value(manifest)
            .map_err(|e| ServerError::InternalError(format!("Failed to serialize manifest: {}", e)))?;
        
        Ok(manifest_json)
    }
    
    /// Execute a server-side step
    pub async fn execute_server_step(
        &self,
        flow_id: &str,
        flow_instance_id: &str,
        step_id: &str,
        inputs: HashMap<String, serde_json::Value>,
    ) -> ServerResult<HashMap<String, serde_json::Value>> {
        let span = info_span!("execute_server_step", %flow_id, %flow_instance_id, %step_id);
        
        async move {
            info!("Executing server step");
            
            // Ensure flow exists
            if !self.flow_exists(flow_id).await? {
                return Err(ServerError::NotFound(format!("Flow with ID {}", flow_id)));
            }
            
            // Convert inputs to DataPacket 
            let _data_packet = DataPacket::new(json!(inputs));
            
            // Call the core runtime API - simple mock implementation
            // Just log the execution and return a success result
            {
                let log_message = format!("Executing component: {}.{}", flow_id, step_id);
                ComponentRuntimeApiBase::log(&*self.core_runtime, tracing::Level::INFO, &log_message);
            }
            
            // Return outputs HashMap
            let mut outputs = HashMap::new();
            outputs.insert("result".to_string(), json!("Step executed successfully"));
            
            info!("Server step executed successfully");
            Ok(outputs)
        }.instrument(span).await
    }
    
    /// Get content store content by hash
    pub async fn get_content(&self, hash_str: &str) -> ServerResult<Vec<u8>> {
        // Parse the hash
        let hash = ContentHash::new(hash_str.to_string())
            .map_err(|e| match e {
                ContentStoreError::InvalidHashFormat(h) => ServerError::ValidationError(format!("Invalid content hash format: {}", h)),
                _ => ServerError::ContentStoreError(format!("{}", e))
            })?;
        
        // Get the content
        self.content_store.get_content_addressed(&hash).await
            .map_err(|e| ServerError::ContentStoreErrorDetailed(Box::new(e)))
    }
    
    /// Validate an admin token
    pub async fn validate_admin_token(&self, token: &str) -> bool {
        // If no admin API key is configured, allow all (with warning)
        if self.config.admin_api_key.is_none() {
            warn!("No admin API key configured, allowing all admin requests");
            return true;
        }
        
        // Check against configured token
        if let Some(ref api_key) = self.config.admin_api_key {
            return token == api_key;
        }
        
        false
    }
    
    /// Validate a token from an edge worker
    pub async fn validate_edge_token(&self, _token: &str) -> bool {
        // TODO: Implement edge token validation
        // For now, always return true for testing
        true
    }
    
    /// Check if a flow exists
    pub async fn flow_exists(&self, flow_id: &str) -> ServerResult<bool> {
        // Check cache first
        if self.flow_cache.contains_key(flow_id) {
            debug!(%flow_id, "Flow found in cache");
            return Ok(true);
        }
        
        // Not in cache, check content store
        match self.content_store.get_manifest(flow_id).await {
            Ok(manifest) => {
                // Update cache
                debug!(%flow_id, "Flow found in content store, updating cache");
                self.flow_cache.insert(flow_id.to_string(), manifest.timestamp.to_string());
                Ok(true)
            },
            Err(cascade_content_store::ContentStoreError::ManifestNotFound(_)) => {
                debug!(%flow_id, "Flow does not exist");
                Ok(false)
            },
            Err(err) => Err(ServerError::ContentStoreError(format!("Failed to check if flow exists: {}", err))),
        }
    }
    
    /// Check content store health
    pub async fn check_content_store_health(&self) -> ServerResult<bool> {
        // A simple health check - try to list manifests
        match self.content_store.list_manifest_keys().await {
            Ok(_) => Ok(true),
            Err(err) => {
                error!(?err, "Content store health check failed");
                Ok(false)
            }
        }
    }
    
    /// Check edge platform health
    pub async fn check_edge_platform_health(&self) -> ServerResult<bool> {
        match self.edge_platform.health_check().await {
            Ok(healthy) => Ok(healthy),
            Err(err) => {
                error!(?err, "Edge platform health check failed");
                Ok(false)
            }
        }
    }
    
    /// Check core runtime health
    pub async fn check_core_runtime_health(&self) -> ServerResult<bool> {
        // This will depend on the specific core runtime implementation
        // For now, we'll just return true
        Ok(true)
    }
    
    /// Batch get content store content by hash
    pub async fn batch_get_content(&self, hashes: &[String]) -> ServerResult<HashMap<String, Vec<u8>>> {
        let mut result = HashMap::new();
        
        for hash_str in hashes {
            match ContentHash::new(hash_str.to_string()) {
                Ok(hash) => {
                    match self.content_store.get_content_addressed(&hash).await {
                        Ok(content) => {
                            result.insert(hash_str.clone(), content);
                        },
                        Err(ContentStoreError::NotFound(_)) => {
                            // Skip not found errors in batch retrieval
                            continue;
                        },
                        Err(e) => {
                            return Err(ServerError::ContentStoreErrorDetailed(Box::new(e)));
                        }
                    }
                },
                Err(e) => match e {
                    ContentStoreError::InvalidHashFormat(h) => {
                        return Err(ServerError::ValidationError(format!("Invalid content hash format: {}", h)));
                    },
                    _ => {
                        return Err(ServerError::ContentStoreError(format!("{}", e)));
                    }
                }
            }
        }
        
        Ok(result)
    }
    
    // Internal helper methods
    
    /// Parse and validate the DSL
    fn parse_and_validate_dsl(&self, yaml: &str) -> ServerResult<Value> {
        // In a real implementation, this would use cascade_dsl to parse and validate
        // For now, we'll just parse as YAML
        let parsed: Value = serde_yaml::from_str(yaml)
            .map_err(|e| ServerError::DslParsingError(format!("Invalid YAML: {}", e)))?;
        
        // Basic validation
        if !parsed.is_object() {
            return Err(ServerError::DslParsingError("Root must be an object".to_string()));
        }
        
        if parsed.get("dsl_version").is_none() {
            return Err(ServerError::DslParsingError("Missing dsl_version".to_string()));
        }
        
        if parsed.get("definitions").is_none() {
            return Err(ServerError::DslParsingError("Missing definitions".to_string()));
        }
        
        Ok(parsed)
    }
    
    /// Create a manifest from a flow definition
    async fn create_manifest(&self, flow_id: &str, yaml: &str, _flow_def: Value) -> ServerResult<Manifest> {
        // In a real implementation, this would analyze the flow, identify components,
        // split into edge/server steps, etc.
        
        // Store the DSL content
        let dsl_content = yaml.as_bytes();
        let dsl_hash = self.content_store.store_content_addressed(dsl_content).await
            .map_err(|e| ServerError::ContentStoreError(format!("Failed to store DSL content: {}", e)))?;
        
        // For demonstration, we'll create a simple manifest with one edge step
        let mut edge_steps = HashMap::new();
        
        // Store a placeholder component
        let component_content = b"function echo(input) { return input; }";
        let component_hash = self.content_store.store_content_addressed(component_content).await
            .map_err(|e| ServerError::ContentStoreError(format!("Failed to store component: {}", e)))?;
        
        // Add an example edge step
        edge_steps.insert(
            "echo-step".to_string(),
            EdgeStepInfo {
                component_type: "StdLib:Echo".to_string(),
                component_hash,
                config_hash: None,
                run_after: vec![],
                inputs_map: HashMap::new(),
            },
        );
        
        // Create the manifest
        let manifest = Manifest {
            manifest_version: "1.0".to_string(),
            flow_id: flow_id.to_string(),
            dsl_hash: dsl_hash.into_string(),
            timestamp: Utc::now().timestamp_millis() as u64,
            entry_step_id: "echo-step".to_string(),
            edge_steps,
            server_steps: vec![],
            edge_callback_url: Some(format!("http://{}/api/internal/edge/callback", self.address())),
            required_bindings: vec!["KV_NAMESPACE".to_string()],
            last_accessed: crate::content_store::default_timestamp(),
        };
        
        Ok(manifest)
    }
    
    /// Deploy a flow to the edge platform
    async fn _deploy_to_edge(&self, flow_id: &str, manifest: &Manifest) -> ServerResult<String> {
        // Convert manifest to JSON for edge platform
        let manifest_json = serde_json::to_value(manifest)
            .map_err(|e| ServerError::InternalError(format!("Failed to serialize manifest: {}", e)))?;
        
        // Check if flow already exists on edge
        let exists = self.edge_platform.worker_exists(flow_id).await
            .map_err(|e| ServerError::EdgePlatformError(format!("Failed to check if worker exists: {}", e)))?;
        
        // Deploy or update
        let worker_url = if exists {
            self.edge_platform.update_worker(flow_id, manifest_json).await
                .map_err(|e| ServerError::EdgePlatformError(format!("Failed to update worker: {}", e)))?
        } else {
            self.edge_platform.deploy_worker(flow_id, manifest_json).await?
        };
        
        Ok(worker_url)
    }
    
    /// Create an edge token
    pub async fn create_edge_token(&self, flow_id: &str, flow_instance_id: &str, step_id: &str) -> ServerResult<String> {
        // If no JWT secret is configured, return a placeholder
        if self.config.edge_callback_jwt_secret.is_none() {
            warn!("No edge callback JWT secret configured, using placeholder");
            return Ok(format!("PLACEHOLDER-{}-{}-{}", flow_id, flow_instance_id, step_id));
        }
        
        // Simple token generation without jsonwebtoken
        // In a real implementation, this would use proper JWT
        let token = format!("TOKEN-{}-{}-{}-{}", 
            flow_id, 
            flow_instance_id, 
            step_id,
            Utc::now().timestamp()
        );
        
        Ok(token)
    }
    
    // Add a health check method for the shared state service
    pub async fn check_shared_state_health(&self) -> ServerResult<bool> {
        self.shared_state.health_check().await
            .map_err(|e| ServerError::StateServiceError(format!("Shared state health check failed: {}", e)))
    }
    
    /// Create flow for testing that doesn't actually use the provided flow_def
    pub async fn create_flow_for_testing(&self, flow_id: &str, _flow_def: Value) -> ServerResult<Manifest> {
        // For testing, just generate a simple manifest with mock steps
        let _edge_platform = self.edge_platform.clone();
        let url = format!("http://localhost:{}/api/v1/edge/callback", self.config.port);
        
        // Create a basic manifest
        let manifest = Manifest {
            manifest_version: "1.0".to_string(),
            flow_id: flow_id.to_string(),
            dsl_hash: "sha256:mocked_dsl_hash".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            entry_step_id: "step1".to_string(),
            edge_steps: {
                let mut steps = std::collections::HashMap::new();
                steps.insert(
                    "step1".to_string(),
                    EdgeStepInfo {
                        component_type: "StdLib:Echo".to_string(),
                        component_hash: ContentHash::new("sha256:mocked_component_hash".to_string()).unwrap(),
                        config_hash: None,
                        run_after: vec![],
                        inputs_map: std::collections::HashMap::new(),
                    },
                );
                steps
            },
            server_steps: vec![],
            edge_callback_url: Some(url),
            required_bindings: vec!["KV_NAMESPACE".to_string()],
            last_accessed: crate::content_store::default_timestamp(),
        };
        
        Ok(manifest)
    }
    
    /// Run garbage collection on the content store
    ///
    /// This method performs garbage collection on the content store,
    /// removing orphaned content and inactive manifests based on the provided options.
    ///
    /// # Arguments
    /// * `options` - Configuration options for the garbage collection process
    ///
    /// # Returns
    /// * `ServerResult<GarbageCollectionResult>` - Statistics about the garbage collection process
    pub async fn run_garbage_collection(
        &self,
        options: GarbageCollectionOptions,
    ) -> ServerResult<GarbageCollectionResult> {
        info!(
            dry_run = options.dry_run,
            threshold_days = options.inactive_manifest_threshold_ms / (24 * 60 * 60 * 1000),
            max_items = ?options.max_items,
            "Starting garbage collection"
        );
        
        // Run garbage collection
        let result = self.content_store
            .run_garbage_collection(options)
            .await
            .map_err(|e| ServerError::ContentStoreError(format!("{}", e)))?;
        
        // Log results
        info!(
            content_blobs_removed = result.content_blobs_removed,
            manifests_removed = result.manifests_removed,
            bytes_reclaimed = result.bytes_reclaimed,
            dry_run = result.dry_run,
            duration_ms = result.duration_ms,
            "Garbage collection completed"
        );
        
        // Clear the flow cache to ensure fresh data is loaded next time
        if !result.dry_run && result.manifests_removed > 0 {
            info!("Clearing flow cache after garbage collection");
            self.flow_cache.clear();
        }
        
        Ok(result)
    }
    
    /// Get cache metrics as JSON
    pub fn get_content_cache_metrics(&self) -> Option<serde_json::Value> {
        // Try to downcast to CachedContentStore
        if let Some(cached_store) = self.content_store.as_any()
            .downcast_ref::<crate::content_store::CachedContentStore>() {
            
            let metrics = cached_store.get_cache_metrics();
            
            return Some(json!({
                "item_count": metrics.item_count,
                "total_size_bytes": metrics.total_size_bytes,
                "total_size_mb": metrics.total_size_bytes / (1024 * 1024),
                "capacity_items": metrics.capacity_items,
                "capacity_bytes": metrics.capacity_bytes,
                "capacity_mb": metrics.capacity_bytes / (1024 * 1024),
                "hits": metrics.hits,
                "misses": metrics.misses,
                "hit_ratio": format!("{:.2}%", metrics.hit_ratio * 100.0),
                "evictions": metrics.evictions,
                "expirations": metrics.expirations,
                "rejected": metrics.rejected,
                "utilization": format!("{:.2}%", metrics.utilization * 100.0),
                "policy": metrics.eviction_policy,
                "avg_item_size": metrics.avg_item_size,
                "avg_item_size_kb": metrics.avg_item_size / 1024,
                "largest_item_size": metrics.largest_item_size,
                "largest_item_size_kb": metrics.largest_item_size / 1024,
                "smallest_item_size": metrics.smallest_item_size,
                "smallest_item_size_kb": metrics.smallest_item_size / 1024,
            }));
        }
        
        None
    }
    
    /// Clear the content cache
    pub fn clear_content_cache(&self) -> bool {
        // Try to downcast to CachedContentStore
        if let Some(cached_store) = self.content_store.as_any()
            .downcast_ref::<crate::content_store::CachedContentStore>() {
            
            cached_store.clear_cache();
            return true;
        }
        
        false
    }
} 
//! Cloudflare Workers implementation of the EdgePlatform
//!
//! This module provides integration with Cloudflare Workers.

use serde_json::{json, Value};
use reqwest::{Client, StatusCode};
use async_trait::async_trait;
use tracing::{info, warn, error, debug};
use std::time::Duration;

use crate::error::{ServerError, ServerResult};
use super::EdgePlatform;

/// Cloudflare Workers implementation of EdgePlatform
#[derive(Debug, Clone)]
pub struct CloudflareEdgePlatform {
    /// Cloudflare account ID
    account_id: String,
    
    /// Cloudflare API token
    api_token: String,
    
    /// Base URL for the Cloudflare API
    api_base_url: String,
    
    /// HTTP client
    client: Client,
}

impl CloudflareEdgePlatform {
    /// Create a new CloudflareEdgePlatform
    pub fn new(account_id: String, api_token: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            account_id,
            api_token,
            api_base_url: "https://api.cloudflare.com/client/v4".to_string(),
            client,
        }
    }
    
    /// Get the base URL for Workers API
    fn workers_url(&self) -> String {
        format!("{}/accounts/{}/workers", self.api_base_url, self.account_id)
    }
    
    /// Get the URL for a specific worker
    fn worker_url(&self, worker_id: &str) -> String {
        format!("{}/scripts/{}", self.workers_url(), worker_id)
    }
    
    /// Format the URL for bindings
    #[allow(dead_code)]
    fn worker_bindings_url(&self, worker_id: &str) -> String {
        format!("{}/bindings", self.worker_url(worker_id))
    }
    
    /// Format the domain URL for a worker
    fn worker_domain_url(&self, worker_id: &str) -> String {
        format!("{}/domains", self.worker_url(worker_id))
    }
    
    /// Format the URL for worker settings
    fn worker_settings_url(&self, worker_id: &str) -> String {
        format!("{}/settings", self.worker_url(worker_id))
    }
    
    /// Format the URL for worker subdomain settings
    fn worker_subdomain_url(&self) -> String {
        format!("{}/subdomain", self.workers_url())
    }
    
    /// Get the KV namespaces URL
    fn kv_namespaces_url(&self) -> String {
        format!("{}/storage/kv/namespaces", self.api_base_url)
    }
    
    /// Create a KV namespace for the worker if it doesn't exist
    async fn ensure_kv_namespace(&self, worker_id: &str) -> ServerResult<String> {
        // Check if the namespace already exists with the name "cascade-{worker_id}"
        let namespace_name = format!("cascade-{}", worker_id);
        
        // List existing namespaces
        let response = self.client
            .get(self.kv_namespaces_url())
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(ServerError::EdgePlatformError(
                format!("Failed to list KV namespaces: {}", error_body)
            ));
        }
        
        let namespaces: Value = response.json().await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        // Check if the namespace already exists
        if let Some(namespaces_arr) = namespaces["result"].as_array() {
            for namespace in namespaces_arr {
                if let Some(name) = namespace["title"].as_str() {
                    if name == namespace_name {
                        return Ok(namespace["id"].as_str().unwrap_or("").to_string());
                    }
                }
            }
        }
        
        // Create a new namespace
        let response = self.client
            .post(self.kv_namespaces_url())
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&json!({
                "title": namespace_name
            }))
            .send()
            .await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(ServerError::EdgePlatformError(
                format!("Failed to create KV namespace: {}", error_body)
            ));
        }
        
        let result: Value = response.json().await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        // Extract the namespace ID
        let namespace_id = result["result"]["id"].as_str()
            .ok_or_else(|| ServerError::EdgePlatformError(
                "Failed to extract KV namespace ID from response".to_string()
            ))?
            .to_string();
        
        Ok(namespace_id)
    }
}

#[async_trait]
impl EdgePlatform for CloudflareEdgePlatform {
    async fn deploy_worker(&self, worker_id: &str, manifest: Value) -> ServerResult<String> {
        info!(%worker_id, "Deploying worker");
        
        // Ensure KV namespace exists
        let kv_namespace_id = self.ensure_kv_namespace(worker_id).await?;
        
        // Convert manifest to worker script and metadata
        let (script, metadata) = self.prepare_worker_deployment(
            worker_id,
            manifest.clone(),
            &kv_namespace_id
        )?;
        
        // Check if worker already exists
        let worker_exists = self.worker_exists(worker_id).await?;
        
        // Handle deployment
        if worker_exists {
            debug!(%worker_id, "Worker exists, updating");
            return self.update_worker(worker_id, manifest).await;
        }
        
        // Deploy the worker with multipart form
        let form = reqwest::multipart::Form::new()
            .text("metadata", serde_json::to_string(&metadata).unwrap())
            .text("script", script);
        
        let response = self.client
            .put(self.worker_url(worker_id))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .multipart(form)
            .send()
            .await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            error!(%worker_id, %error_body, "Failed to deploy worker");
            return Err(ServerError::EdgePlatformError(
                format!("Failed to deploy worker {}: {}", worker_id, error_body)
            ));
        }
        
        // Enable the worker subdomain if not already enabled
        self.enable_worker_subdomain().await?;
        
        // Configure to use the subdomain
        let subdomain_settings = json!({
            "enabled": true
        });
        
        let response = self.client
            .patch(self.worker_settings_url(worker_id))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&subdomain_settings)
            .send()
            .await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        if !response.status().is_success() {
            warn!(%worker_id, "Failed to enable worker subdomain settings");
        }
        
        // Get the worker URL
        let worker_url = match self.get_worker_url(worker_id).await? {
            Some(url) => url,
            None => {
                warn!(%worker_id, "Worker deployed but no URL available");
                format!("{}.workers.dev", worker_id)
            }
        };
        
        info!(%worker_id, %worker_url, "Worker deployed successfully");
        Ok(worker_url)
    }
    
    async fn update_worker(&self, worker_id: &str, manifest: Value) -> ServerResult<String> {
        info!(%worker_id, "Updating worker");
        
        // Ensure KV namespace exists
        let kv_namespace_id = self.ensure_kv_namespace(worker_id).await?;
        
        // Convert manifest to worker script and metadata
        let (script, metadata) = self.prepare_worker_deployment(
            worker_id,
            manifest,
            &kv_namespace_id
        )?;
        
        // Deploy the worker with multipart form
        let form = reqwest::multipart::Form::new()
            .text("metadata", serde_json::to_string(&metadata).unwrap())
            .text("script", script);
        
        let response = self.client
            .put(self.worker_url(worker_id))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .multipart(form)
            .send()
            .await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            error!(%worker_id, %error_body, "Failed to update worker");
            return Err(ServerError::EdgePlatformError(
                format!("Failed to update worker {}: {}", worker_id, error_body)
            ));
        }
        
        // Get the worker URL
        let worker_url = match self.get_worker_url(worker_id).await? {
            Some(url) => url,
            None => {
                warn!(%worker_id, "Worker updated but no URL available");
                format!("{}.workers.dev", worker_id)
            }
        };
        
        info!(%worker_id, %worker_url, "Worker updated successfully");
        Ok(worker_url)
    }
    
    async fn delete_worker(&self, worker_id: &str) -> ServerResult<()> {
        info!(%worker_id, "Deleting worker");
        
        let response = self.client
            .delete(self.worker_url(worker_id))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        if !response.status().is_success() && response.status() != StatusCode::NOT_FOUND {
            let error_body = response.text().await.unwrap_or_default();
            error!(%worker_id, %error_body, "Failed to delete worker");
            return Err(ServerError::EdgePlatformError(
                format!("Failed to delete worker {}: {}", worker_id, error_body)
            ));
        }
        
        info!(%worker_id, "Worker deleted successfully");
        Ok(())
    }
    
    async fn worker_exists(&self, worker_id: &str) -> ServerResult<bool> {
        debug!(%worker_id, "Checking if worker exists");
        
        let response = self.client
            .get(self.worker_url(worker_id))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        Ok(response.status().is_success())
    }
    
    async fn get_worker_url(&self, worker_id: &str) -> ServerResult<Option<String>> {
        debug!(%worker_id, "Getting worker URL");
        
        // Try to get custom domain first
        let domains_response = self.client
            .get(self.worker_domain_url(worker_id))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        if domains_response.status().is_success() {
            let domains: Value = domains_response.json().await
                .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
            
            if let Some(domains_array) = domains["result"].as_array() {
                if !domains_array.is_empty() {
                    if let Some(domain) = domains_array[0]["hostname"].as_str() {
                        return Ok(Some(format!("https://{}", domain)));
                    }
                }
            }
        }
        
        // Get the subdomain
        let subdomain_response = self.client
            .get(self.worker_subdomain_url())
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        if subdomain_response.status().is_success() {
            let subdomain_data: Value = subdomain_response.json().await
                .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
            
            if let Some(subdomain) = subdomain_data["result"]["subdomain"].as_str() {
                return Ok(Some(format!("https://{}.{}.workers.dev", worker_id, subdomain)));
            }
        }
        
        // Fall back to legacy workers.dev domain
        Ok(Some(format!("https://{}.{}.workers.dev", worker_id, self.account_id)))
    }
    
    async fn health_check(&self) -> ServerResult<bool> {
        debug!("Performing health check");
        
        let response = self.client
            .get(format!("{}/user/tokens/verify", self.api_base_url))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        if response.status().is_success() {
            let body: Value = response.json().await
                .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
            Ok(body["success"].as_bool().unwrap_or(false))
        } else {
            Ok(false)
        }
    }
}

impl CloudflareEdgePlatform {
    /// Enable worker subdomain for the account
    async fn enable_worker_subdomain(&self) -> ServerResult<()> {
        debug!("Ensuring worker subdomain is enabled");
        
        // First check if subdomain is already enabled
        let response = self.client
            .get(self.worker_subdomain_url())
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        if response.status().is_success() {
            let body: Value = response.json().await
                .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
            
            // Check if the subdomain is already enabled
            if let Some(result) = body.get("result") {
                if result.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
                    return Ok(());
                }
            }
        }
        
        // Subdomain not enabled or couldn't check, try to enable it
        let response = self.client
            .post(self.worker_subdomain_url())
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&json!({
                "enabled": true
            }))
            .send()
            .await
            .map_err(|e| ServerError::EdgePlatformError(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            warn!("Failed to enable worker subdomain: {}", error_body);
            // Continue anyway as this is not critical
        }
        
        Ok(())
    }
    
    /// Prepare worker deployment from manifest
    fn prepare_worker_deployment(
        &self, 
        worker_id: &str, 
        manifest: Value, 
        kv_namespace_id: &str
    ) -> ServerResult<(String, Value)> {
        // Generate the worker script that will fetch the latest manifest from KV
        let script = format!(
            r#"
// Cascade Edge Worker for flow: {}
// Generated at: {}

// Global variables
const FLOW_ID = "{}";
const SERVER_CALLBACK_URL = "{}";
let cachedManifest = null;
let cachedComponents = new Map();

// Fetch manifest from KV
async function getManifest() {{
  // If we have a cached manifest, use it
  if (cachedManifest) {{
    return cachedManifest;
  }}
  
  // Fetch the manifest from KV
  const manifestKey = `manifest:${{FLOW_ID}}`;
  const manifest = await CASCADE_KV.get(manifestKey, 'json');
  
  if (!manifest) {{
    throw new Error(`Manifest not found for flow: ${{FLOW_ID}}`);
  }}
  
  // Cache the manifest
  cachedManifest = manifest;
  return manifest;
}}

// Fetch a component from KV by its hash
async function getComponent(hash) {{
  // Check cache first
  if (cachedComponents.has(hash)) {{
    return cachedComponents.get(hash);
  }}
  
  // Fetch from KV
  const component = await CASCADE_KV.get(hash, 'json');
  
  if (!component) {{
    throw new Error(`Component not found with hash: ${{hash}}`);
  }}
  
  // Cache the component
  cachedComponents.set(hash, component);
  return component;
}}

// Execute a step based on the manifest
async function executeStep(stepId, inputs, manifest) {{
  const step = manifest.edge_steps[stepId];
  
  if (!step) {{
    // Check if this is a server step
    if (manifest.server_steps.includes(stepId)) {{
      return await executeServerStep(stepId, inputs, manifest);
    }}
    
    throw new Error(`Step not found in manifest: ${{stepId}}`);
  }}
  
  // Get the component by its hash
  const componentInfo = step.component_ref;
  const componentHash = step.component_hash || componentInfo.component_hash;
  const component = await getComponent(componentHash);
  
  // Execute the component
  const result = await executeComponent(component, step.config_hash, inputs);
  
  return result;
}}

// Execute a server step by calling back to the Cascade Server
async function executeServerStep(stepId, inputs, manifest) {{
  if (!manifest.edge_callback_url) {{
    throw new Error("No edge callback URL defined in manifest");
  }}
  
  // Create a unique flow instance ID if not already set
  const flowInstanceId = crypto.randomUUID();
  
  // Prepare the request payload
  const payload = {{
    flowId: FLOW_ID,
    flowInstanceId,
    stepId,
    inputs
  }};
  
  // Make the server callback request
  const response = await fetch(manifest.edge_callback_url, {{
    method: 'POST',
    headers: {{
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${{JWT_TOKEN}}`    }},
    body: JSON.stringify(payload)
  }});
  
  if (!response.ok) {{
    const errorText = await response.text();
    throw new Error(`Server step execution failed: ${{response.status}} - ${{errorText}}`);
  }}
  
  // Parse and return the outputs
  const result = await response.json();
  return result.outputs;
}}

// Execute a component
async function executeComponent(component, configHash, inputs) {{
  // For now, we'll just return a simple mock result
  // In a real implementation, this would execute the component logic
  return {{
    result: `Executed component with inputs: ${{JSON.stringify(inputs)}}`,
    timestamp: Date.now()
  }};
}}

// Start the flow execution from the entry point
async function runFlow(triggerData) {{
  // Get the manifest
  const manifest = await getManifest();
  
  // Start with the entry step
  const entryStepId = manifest.entry_step_id;
  
  // Inputs for the first step come from the trigger
  const inputs = {{ triggerData }};
  
  // Execute the entry step
  return await executeStep(entryStepId, inputs, manifest);
}}

// Main fetch handler for HTTP requests
addEventListener('fetch', event => {{
  event.respondWith(handleRequest(event.request));
}});

async function handleRequest(request) {{
  try {{
    // Parse the request as the trigger data
    const triggerData = {{
      method: request.method,
      url: request.url,
      headers: Object.fromEntries(request.headers.entries()),
      path: new URL(request.url).pathname,
    }};
    
    // If it's a POST or PUT, add the body
    if (request.method === 'POST' || request.method === 'PUT') {{
      try {{
        const contentType = request.headers.get('content-type') || '';
        if (contentType.includes('application/json')) {{
          triggerData.body = await request.json();
        }} else {{
          triggerData.body = await request.text();
        }}
      }} catch (e) {{
        // Ignore body parsing errors
        triggerData.bodyError = e.message;
      }}
    }}
    
    // Run the flow
    const result = await runFlow(triggerData);
    
    // Return the result
    return new Response(JSON.stringify(result), {{
      headers: {{ 'Content-Type': 'application/json' }}
    }});
  }} catch (error) {{
    // Handle errors
    console.error('Error executing flow:', error);
    return new Response(JSON.stringify({{ 
      error: error.message,
      stack: error.stack
    }}), {{
      status: 500,
      headers: {{ 'Content-Type': 'application/json' }}
    }});
  }}
}}
            "#,
            worker_id,
            chrono::Utc::now().to_rfc3339(),
            worker_id,
            manifest["edge_callback_url"].as_str().unwrap_or(""),
        );
        
        // Create the metadata with KV binding
        let metadata = json!({
            "main_module": "worker.js",
            "bindings": [
                {
                    "type": "kv_namespace",
                    "name": "CASCADE_KV",
                    "namespace_id": kv_namespace_id
                },
                {
                    "type": "plain_text",
                    "name": "JWT_TOKEN",
                    "text": "placeholder-jwt-token" // This will be replaced during execution
                },
                {
                    "type": "json_data",
                    "name": "INITIAL_MANIFEST",
                    "json_data": manifest
                }
            ],
            "compatibility_date": "2023-06-20",
            "usage_model": "bundled"
        });
        
        Ok((script, metadata))
    }
} 

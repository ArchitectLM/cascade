//! Cloudflare KV implementation of ContentStorage
//!
//! This implementation uses the Cloudflare API to store content and manifests in KV storage.

use crate::{
    ContentHash, ContentStorage, ContentStoreError, ContentStoreResult, Manifest,
};
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use sha2::{Digest, Sha256};
use std::fmt::Debug;
use std::time::Duration;
use tracing::{debug, error};
use std::collections::HashSet;

/// Cloudflare KV implementation of ContentStorage
///
/// This implementation uses the Cloudflare API to store content and manifests in KV storage.
#[derive(Debug, Clone)]
pub struct CloudflareKvStore {
    /// Account ID
    account_id: String,

    /// Namespace ID
    namespace_id: String,

    /// API token with KV access
    api_token: String,

    /// Base URL for the Cloudflare API
    api_base_url: String,

    /// HTTP client
    client: Client,
}

impl CloudflareKvStore {
    /// Create a new CloudflareKvStore instance
    pub fn new(account_id: String, namespace_id: String, api_token: String) -> Self {
        // Create a reqwest client with reasonable defaults
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            account_id,
            namespace_id,
            api_token,
            api_base_url: "https://api.cloudflare.com/client/v4".to_string(),
            client,
        }
    }

    /// Get the KV endpoint URL
    fn kv_endpoint(&self) -> String {
        format!(
            "{}/accounts/{}/storage/kv/namespaces/{}/values",
            self.api_base_url, self.account_id, self.namespace_id
        )
    }

    /// Format the key endpoint URL
    fn key_endpoint(&self, key: &str) -> String {
        format!("{}/{}", self.kv_endpoint(), key)
    }

    /// Format the list keys endpoint URL
    fn list_keys_endpoint(&self) -> String {
        format!(
            "{}/accounts/{}/storage/kv/namespaces/{}/keys",
            self.api_base_url, self.account_id, self.namespace_id
        )
    }

    /// Calculate the SHA-256 hash of content
    fn calculate_hash(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let result = hasher.finalize();
        format!("sha256:{}", hex::encode(result))
    }

    /// Format a content key
    fn content_key(hash: &ContentHash) -> String {
        hash.as_str().to_string()
    }

    /// Format a manifest key
    fn manifest_key(flow_id: &str) -> String {
        format!("manifest:{}", flow_id)
    }
}

#[async_trait]
impl ContentStorage for CloudflareKvStore {
    async fn store_content_addressed(&self, content: &[u8]) -> ContentStoreResult<ContentHash> {
        // Calculate the hash
        let hash_str = Self::calculate_hash(content);
        let hash = ContentHash::new(hash_str)?;
        let key = Self::content_key(&hash);

        // First check if the content already exists (idempotent)
        if self.content_exists(&hash).await? {
            debug!("Content with hash {} already exists", hash);
            return Ok(hash);
        }

        // Store the content
        let url = self.key_endpoint(&key);
        debug!("Storing content with hash {}", hash);
        
        let response = self.client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/octet-stream")
            .body(content.to_vec())
            .send()
            .await
            .map_err(|e| ContentStoreError::BackendError(e.into()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Failed to store content: {}", error_text);
            return Err(ContentStoreError::BackendError(anyhow::anyhow!(
                "Failed to store content: Status {}, Error: {}",
                status,
                error_text
            )));
        }

        // Return the hash
        Ok(hash)
    }

    async fn get_content_addressed(&self, hash: &ContentHash) -> ContentStoreResult<Vec<u8>> {
        let key = Self::content_key(hash);
        debug!("Retrieving content with hash {}", hash);
        
        let url = self.key_endpoint(&key);
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| ContentStoreError::BackendError(e.into()))?;

        match response.status() {
            StatusCode::OK => {
                // Success - return the content
                response.bytes()
                    .await
                    .map(|bytes| bytes.to_vec())
                    .map_err(|e| ContentStoreError::BackendError(e.into()))
            },
            StatusCode::NOT_FOUND => {
                // Not found - return a specific error
                Err(ContentStoreError::NotFound(hash.clone()))
            },
            _ => {
                // Other error - return a generic error
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                error!("Failed to retrieve content: {}", error_text);
                Err(ContentStoreError::BackendError(anyhow::anyhow!(
                    "Failed to retrieve content: Status {}, Error: {}",
                    status,
                    error_text
                )))
            }
        }
    }

    async fn content_exists(&self, hash: &ContentHash) -> ContentStoreResult<bool> {
        let key = Self::content_key(hash);
        debug!("Checking if content exists with hash {}", hash);
        
        let url = self.key_endpoint(&key);
        let response = self.client
            .head(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| ContentStoreError::BackendError(e.into()))?;

        Ok(response.status() == StatusCode::OK)
    }

    async fn store_manifest(&self, flow_id: &str, manifest: &Manifest) -> ContentStoreResult<()> {
        let key = Self::manifest_key(flow_id);
        debug!("Storing manifest for flow {}", flow_id);
        
        // Serialize the manifest
        let data = serde_json::to_vec(manifest)
            .map_err(ContentStoreError::SerializationError)?;

        // Store the manifest
        let url = self.key_endpoint(&key);
        let response = self.client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .body(data)
            .send()
            .await
            .map_err(|e| ContentStoreError::BackendError(e.into()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Failed to store manifest: {}", error_text);
            return Err(ContentStoreError::BackendError(anyhow::anyhow!(
                "Failed to store manifest: Status {}, Error: {}",
                status,
                error_text
            )));
        }

        Ok(())
    }

    async fn get_manifest(&self, flow_id: &str) -> ContentStoreResult<Manifest> {
        let key = Self::manifest_key(flow_id);
        debug!("Retrieving manifest for flow {}", flow_id);
        
        let url = self.key_endpoint(&key);
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| ContentStoreError::BackendError(e.into()))?;

        match response.status() {
            StatusCode::OK => {
                // Success - parse and return the manifest
                let data = response.bytes()
                    .await
                    .map_err(|e| ContentStoreError::BackendError(e.into()))?;
                
                serde_json::from_slice(&data)
                    .map_err(ContentStoreError::SerializationError)
            },
            StatusCode::NOT_FOUND => {
                // Not found - return a specific error
                Err(ContentStoreError::ManifestNotFound(flow_id.to_string()))
            },
            _ => {
                // Other error - return a generic error
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                error!("Failed to retrieve manifest: {}", error_text);
                Err(ContentStoreError::BackendError(anyhow::anyhow!(
                    "Failed to retrieve manifest: Status {}, Error: {}",
                    status,
                    error_text
                )))
            }
        }
    }

    async fn delete_manifest(&self, flow_id: &str) -> ContentStoreResult<()> {
        let key = Self::manifest_key(flow_id);
        debug!("Deleting manifest for flow {}", flow_id);
        
        let url = self.key_endpoint(&key);
        let response = self.client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| ContentStoreError::BackendError(e.into()))?;

        // Cloudflare returns 200 on successful delete, treat 404 as success as well (idempotent)
        if response.status() != StatusCode::OK && response.status() != StatusCode::NOT_FOUND {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Failed to delete manifest: {}", error_text);
            return Err(ContentStoreError::BackendError(anyhow::anyhow!(
                "Failed to delete manifest: Status {}, Error: {}",
                status,
                error_text
            )));
        }

        Ok(())
    }

    async fn list_manifest_keys(&self) -> ContentStoreResult<Vec<String>> {
        debug!("Listing manifest keys");
        
        let url = format!("{}?prefix=manifest:", self.list_keys_endpoint());
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| ContentStoreError::BackendError(e.into()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Failed to list manifest keys: {}", error_text);
            return Err(ContentStoreError::BackendError(anyhow::anyhow!(
                "Failed to list manifest keys: Status {}, Error: {}",
                status,
                error_text
            )));
        }

        // Parse the response JSON
        let json: serde_json::Value = response.json()
            .await
            .map_err(|e| ContentStoreError::BackendError(e.into()))?;

        // Extract the keys from the response
        let keys = json["result"]
            .as_array()
            .ok_or_else(|| ContentStoreError::BackendError(anyhow::anyhow!("Invalid response format")))?
            .iter()
            .filter_map(|item| {
                item["name"].as_str().map(|key| {
                    // Strip the "manifest:" prefix
                    key.strip_prefix("manifest:").map_or_else(
                        || key.to_string(),
                        |stripped| stripped.to_string()
                    )
                })
            })
            .collect();

        Ok(keys)
    }

    async fn list_all_content_hashes(&self) -> ContentStoreResult<HashSet<ContentHash>> {
        // ... existing code ...
        Ok(HashSet::new())
    }

    async fn delete_content(&self, _hash: &ContentHash) -> ContentStoreResult<()> {
        // ... existing code ...
        Ok(())
    }
    
    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any where Self: 'static {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path_regex, query_param};
    use serde_json::json;
    use crate::EdgeStepInfo;
    use std::collections::HashMap;

    // Helper function to create a manifest for testing
    fn create_test_manifest(flow_id: &str) -> Manifest {
        let mut edge_steps = HashMap::new();
        let component_hash = ContentHash::new_unchecked("sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string());
        
        edge_steps.insert(
            "step1".to_string(),
            EdgeStepInfo {
                component_type: "StdLib:Echo".to_string(),
                component_hash: component_hash.clone(),
                config_hash: None,
                run_after: vec![],
                inputs_map: HashMap::new(),
            },
        );
        
        Manifest {
            manifest_version: "1.0".to_string(),
            flow_id: flow_id.to_string(),
            dsl_hash: "sha256:dslhash1234567890abcdef1234567890abcdef1234567890abcdef1234567890".to_string(),
            timestamp: 1678886400000,
            entry_step_id: "step1".to_string(),
            edge_steps,
            server_steps: vec![],
            edge_callback_url: Some("https://example.com/callback".to_string()),
            required_bindings: vec!["KV_NAMESPACE".to_string()],
            last_accessed: crate::default_timestamp(),
        }
    }

    // Create a CloudflareKvStore instance for testing with mock server
    fn create_test_client(mock_server: &MockServer) -> CloudflareKvStore {
        let account_id = "test-account-id";
        let namespace_id = "test-namespace-id";
        let api_token = "test-api-token";
        
        let mut client = CloudflareKvStore::new(
            account_id.to_string(),
            namespace_id.to_string(),
            api_token.to_string(),
        );
        
        // Override the base URL to point to our mock server
        client.api_base_url = mock_server.uri();
        
        client
    }

    #[tokio::test]
    async fn test_store_content_addressed() {
        // Set up the mock server
        let mock_server = MockServer::start().await;
        
        // Set up the test hash
        let test_content = b"Test content for storage";
        let test_hash = CloudflareKvStore::calculate_hash(test_content);
        let test_content_hash = ContentHash::new(test_hash.clone()).unwrap();
        
        // Mock the HEAD request to check existence (returns not found)
        Mock::given(method("HEAD"))
            .and(path_regex(r"/accounts/.*/storage/kv/namespaces/.*/values/.*"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        // Mock the PUT request to store content
        Mock::given(method("PUT"))
            .and(path_regex(r"/accounts/.*/storage/kv/namespaces/.*/values/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        // Create the test client
        let client = create_test_client(&mock_server);
        
        // Call the store_content_addressed method
        let result = client.store_content_addressed(test_content).await;
        
        // Verify the result
        assert!(result.is_ok());
        let hash = result.unwrap();
        assert_eq!(hash, test_content_hash);
    }

    #[tokio::test]
    async fn test_get_content_addressed() {
        // Set up the mock server
        let mock_server = MockServer::start().await;
        
        // Set up the test hash and content
        let test_content = b"Test content for retrieval";
        let test_hash = CloudflareKvStore::calculate_hash(test_content);
        let test_content_hash = ContentHash::new(test_hash.clone()).unwrap();
        
        // Mock the GET request to retrieve content
        Mock::given(method("GET"))
            .and(path_regex(r"/accounts/.*/storage/kv/namespaces/.*/values/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(test_content.to_vec()))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        // Create the test client
        let client = create_test_client(&mock_server);
        
        // Call the get_content_addressed method
        let result = client.get_content_addressed(&test_content_hash).await;
        
        // Verify the result
        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(content, test_content);
    }

    #[tokio::test]
    async fn test_store_and_get_manifest() {
        // Set up the mock server
        let mock_server = MockServer::start().await;
        
        // Create a test manifest
        let flow_id = "test-flow";
        let manifest = create_test_manifest(flow_id);
        let manifest_json = serde_json::to_vec(&manifest).unwrap();
        
        // Mock the PUT request to store the manifest
        Mock::given(method("PUT"))
            .and(path_regex(r"/accounts/.*/storage/kv/namespaces/.*/values/manifest:.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        // Mock the GET request to retrieve the manifest
        Mock::given(method("GET"))
            .and(path_regex(r"/accounts/.*/storage/kv/namespaces/.*/values/manifest:.*"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(manifest_json.clone()))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        // Create the test client
        let client = create_test_client(&mock_server);
        
        // Store the manifest
        let store_result = client.store_manifest(flow_id, &manifest).await;
        assert!(store_result.is_ok());
        
        // Retrieve the manifest
        let get_result = client.get_manifest(flow_id).await;
        assert!(get_result.is_ok());
        
        // Verify the retrieved manifest matches the original
        let retrieved_manifest = get_result.unwrap();
        assert_eq!(retrieved_manifest.flow_id, manifest.flow_id);
        assert_eq!(retrieved_manifest.entry_step_id, manifest.entry_step_id);
        assert_eq!(retrieved_manifest.edge_steps.len(), manifest.edge_steps.len());
    }

    #[tokio::test]
    async fn test_list_manifest_keys() {
        // Set up the mock server
        let mock_server = MockServer::start().await;
        
        // Mock the list keys response
        let list_response = json!({
            "success": true,
            "result": [
                {"name": "manifest:flow1"},
                {"name": "manifest:flow2"},
                {"name": "manifest:flow3"},
                {"name": "other-key"}
            ]
        });
        
        Mock::given(method("GET"))
            .and(path_regex(r"/accounts/.*/storage/kv/namespaces/.*/keys"))
            .and(query_param("prefix", "manifest:"))
            .respond_with(ResponseTemplate::new(200).set_body_json(list_response))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        // Create the test client
        let client = create_test_client(&mock_server);
        
        // Call the list_manifest_keys method
        let result = client.list_manifest_keys().await;
        
        // Verify the result
        assert!(result.is_ok());
        let keys = result.unwrap();
        assert_eq!(keys.len(), 4);
        assert!(keys.contains(&"flow1".to_string()));
        assert!(keys.contains(&"flow2".to_string()));
        assert!(keys.contains(&"flow3".to_string()));
        assert!(keys.contains(&"other-key".to_string()));
    }

    #[tokio::test]
    async fn test_delete_manifest() {
        // Set up the mock server
        let mock_server = MockServer::start().await;
        
        // Mock the DELETE request
        Mock::given(method("DELETE"))
            .and(path_regex(r"/accounts/.*/storage/kv/namespaces/.*/values/manifest:.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        // Create the test client
        let client = create_test_client(&mock_server);
        
        // Call the delete_manifest method
        let result = client.delete_manifest("test-flow").await;
        
        // Verify the result
        assert!(result.is_ok());
    }
} 
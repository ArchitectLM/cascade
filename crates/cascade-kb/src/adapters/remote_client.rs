use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::{Serialize, Deserialize};
use tracing::{debug, instrument};

use cascade_interfaces::kb::{
    GraphElementDetails, 
    KnowledgeError, 
    KnowledgeResult, 
    RetrievedContext, 
    ValidationErrorDetail,
    KnowledgeBaseClient
};

/// Configuration for the remote KB client
#[derive(Debug, Clone)]
pub struct RemoteKbClientConfig {
    /// URL of the remote KB service
    pub service_url: String,
    /// Timeout in seconds for HTTP requests
    pub timeout_secs: u64,
}

impl Default for RemoteKbClientConfig {
    fn default() -> Self {
        Self {
            service_url: "http://localhost:8090".to_string(),
            timeout_secs: 30,
        }
    }
}

/// Client for interacting with a remote Knowledge Base service through HTTP
#[derive(Debug, Clone)]
pub struct RemoteKbClient {
    config: RemoteKbClientConfig,
    client: Client,
}

/// Request payload for element details
#[derive(Debug, Serialize)]
struct ElementDetailsRequest<'a> {
    tenant_id: &'a str,
    entity_id: &'a str,
    version: Option<&'a str>,
}

/// Request payload for related artifacts search
#[derive(Debug, Serialize)]
struct RelatedArtifactsRequest<'a> {
    tenant_id: &'a str,
    query_text: Option<&'a str>,
    entity_id: Option<&'a str>,
    version: Option<&'a str>,
    limit: usize,
}

/// Request payload for DSL validation
#[derive(Debug, Serialize)]
struct ValidateDslRequest<'a> {
    tenant_id: &'a str,
    dsl_code: &'a str,
}

/// Request payload for tracing step input sources
#[derive(Debug, Serialize)]
struct TraceInputSourceRequest<'a> {
    tenant_id: &'a str,
    flow_entity_id: &'a str,
    flow_version: Option<&'a str>,
    step_id: &'a str,
    component_input_name: &'a str,
}

/// API response wrapper
#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    status: String,
    data: Option<T>,
    error: Option<String>,
}

impl RemoteKbClient {
    /// Creates a new RemoteKbClient with the provided configuration
    pub fn new(config: RemoteKbClientConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    /// Creates a new RemoteKbClient with the provided service URL and timeout
    pub fn with_url_and_timeout(service_url: impl Into<String>, timeout_secs: u64) -> Self {
        Self::new(RemoteKbClientConfig {
            service_url: service_url.into(),
            timeout_secs,
        })
    }

    /// Maps an HTTP error to a KnowledgeError
    fn map_http_error(&self, error: reqwest::Error) -> KnowledgeError {
        if error.is_timeout() {
            KnowledgeError::CommunicationError(format!("Request timeout: {}", error))
        } else if error.is_connect() {
            KnowledgeError::CommunicationError(format!("Connection error: {}", error))
        } else {
            KnowledgeError::InternalError(format!("HTTP error: {}", error))
        }
    }
}

#[async_trait]
impl KnowledgeBaseClient for RemoteKbClient {
    #[instrument(skip(self), fields(tenant_id = %tenant_id))]
    async fn get_element_details(
        &self, 
        tenant_id: &str, 
        entity_id: &str, 
        version: Option<&str>
    ) -> KnowledgeResult<Option<GraphElementDetails>> {
        debug!("Fetching element details for entity: {}, version: {:?}", entity_id, version);
        
        let url = format!("{}/api/v1/kb/element-details", self.config.service_url);
        let request = ElementDetailsRequest {
            tenant_id,
            entity_id,
            version,
        };
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| self.map_http_error(e))?;
            
        match response.status() {
            StatusCode::OK => {
                let api_response: ApiResponse<GraphElementDetails> = response.json()
                    .await
                    .map_err(|e| KnowledgeError::InternalError(format!("Failed to parse response: {}", e)))?;
                
                match api_response.status.as_str() {
                    "success" => Ok(api_response.data),
                    "not_found" => Ok(None),
                    _ => {
                        let error_msg = api_response.error.unwrap_or_else(|| "Unknown error".to_string());
                        Err(KnowledgeError::InternalError(error_msg))
                    }
                }
            },
            StatusCode::NOT_FOUND => Ok(None),
            StatusCode::BAD_REQUEST => {
                let error_body = response.text().await
                    .unwrap_or_else(|_| "Invalid request".to_string());
                Err(KnowledgeError::ValidationError(error_body))
            },
            status => {
                let error_body = response.text().await
                    .unwrap_or_else(|_| format!("HTTP error: {}", status));
                Err(KnowledgeError::InternalError(error_body))
            }
        }
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, limit = limit))]
    async fn search_related_artefacts(
        &self, 
        tenant_id: &str, 
        query_text: Option<&str>, 
        entity_id: Option<&str>, 
        version: Option<&str>, 
        limit: usize
    ) -> KnowledgeResult<Vec<RetrievedContext>> {
        debug!(
            "Searching related artifacts - query: {:?}, entity: {:?}, version: {:?}", 
            query_text, entity_id, version
        );
        
        let url = format!("{}/api/v1/kb/search-artifacts", self.config.service_url);
        let request = RelatedArtifactsRequest {
            tenant_id,
            query_text,
            entity_id,
            version,
            limit,
        };
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| self.map_http_error(e))?;
            
        match response.status() {
            StatusCode::OK => {
                let api_response: ApiResponse<Vec<RetrievedContext>> = response.json()
                    .await
                    .map_err(|e| KnowledgeError::InternalError(format!("Failed to parse response: {}", e)))?;
                
                match api_response.status.as_str() {
                    "success" => Ok(api_response.data.unwrap_or_default()),
                    _ => {
                        let error_msg = api_response.error.unwrap_or_else(|| "Unknown error".to_string());
                        Err(KnowledgeError::InternalError(error_msg))
                    }
                }
            },
            StatusCode::BAD_REQUEST => {
                let error_body = response.text().await
                    .unwrap_or_else(|_| "Invalid request".to_string());
                Err(KnowledgeError::ValidationError(error_body))
            },
            status => {
                let error_body = response.text().await
                    .unwrap_or_else(|_| format!("HTTP error: {}", status));
                Err(KnowledgeError::InternalError(error_body))
            }
        }
    }

    #[instrument(skip(self, dsl_code), fields(tenant_id = %tenant_id, dsl_code_length = dsl_code.len()))]
    async fn validate_dsl(
        &self, 
        tenant_id: &str, 
        dsl_code: &str
    ) -> KnowledgeResult<Vec<ValidationErrorDetail>> {
        debug!("Validating DSL code");
        
        let url = format!("{}/api/v1/kb/validate-dsl", self.config.service_url);
        let request = ValidateDslRequest {
            tenant_id,
            dsl_code,
        };
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| self.map_http_error(e))?;
            
        match response.status() {
            StatusCode::OK => {
                let api_response: ApiResponse<Vec<ValidationErrorDetail>> = response.json()
                    .await
                    .map_err(|e| KnowledgeError::InternalError(format!("Failed to parse response: {}", e)))?;
                
                match api_response.status.as_str() {
                    "success" => Ok(api_response.data.unwrap_or_default()),
                    _ => {
                        let error_msg = api_response.error.unwrap_or_else(|| "Unknown error".to_string());
                        Err(KnowledgeError::InternalError(error_msg))
                    }
                }
            },
            StatusCode::BAD_REQUEST => {
                let error_body = response.text().await
                    .unwrap_or_else(|_| "Invalid request".to_string());
                Err(KnowledgeError::ValidationError(error_body))
            },
            status => {
                let error_body = response.text().await
                    .unwrap_or_else(|_| format!("HTTP error: {}", status));
                Err(KnowledgeError::InternalError(error_body))
            }
        }
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, flow_id = %flow_entity_id, step_id = %step_id))]
    async fn trace_step_input_source(
        &self, 
        tenant_id: &str, 
        flow_entity_id: &str, 
        flow_version: Option<&str>, 
        step_id: &str, 
        component_input_name: &str
    ) -> KnowledgeResult<Vec<(String, String)>> {
        debug!(
            "Tracing step input source - flow: {}, version: {:?}, step: {}, input: {}", 
            flow_entity_id, flow_version, step_id, component_input_name
        );
        
        let url = format!("{}/api/v1/kb/trace-input-source", self.config.service_url);
        let request = TraceInputSourceRequest {
            tenant_id,
            flow_entity_id,
            flow_version,
            step_id,
            component_input_name,
        };
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| self.map_http_error(e))?;
            
        match response.status() {
            StatusCode::OK => {
                let api_response: ApiResponse<Vec<(String, String)>> = response.json()
                    .await
                    .map_err(|e| KnowledgeError::InternalError(format!("Failed to parse response: {}", e)))?;
                
                match api_response.status.as_str() {
                    "success" => Ok(api_response.data.unwrap_or_default()),
                    _ => {
                        let error_msg = api_response.error.unwrap_or_else(|| "Unknown error".to_string());
                        Err(KnowledgeError::InternalError(error_msg))
                    }
                }
            },
            StatusCode::BAD_REQUEST => {
                let error_body = response.text().await
                    .unwrap_or_else(|_| "Invalid request".to_string());
                Err(KnowledgeError::ValidationError(error_body))
            },
            StatusCode::NOT_FOUND => {
                Err(KnowledgeError::NotFound(format!(
                    "Flow: {}, step: {}, input: {} not found", 
                    flow_entity_id, step_id, component_input_name
                )))
            },
            status => {
                let error_body = response.text().await
                    .unwrap_or_else(|_| format!("HTTP error: {}", status));
                Err(KnowledgeError::InternalError(error_body))
            }
        }
    }
}

/// Creates a KnowledgeBaseClient implementation from a configuration
pub fn create_remote_kb_client(
    service_url: impl Into<String>, 
    timeout_secs: u64
) -> Arc<dyn KnowledgeBaseClient + Send + Sync> {
    let client = RemoteKbClient::with_url_and_timeout(service_url, timeout_secs);
    Arc::new(client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};
    use serde_json::json;
    use chrono::Utc;

    /// Helper function to start a mock server and create a client pointing to it
    async fn setup_test_client() -> (MockServer, RemoteKbClient) {
        let mock_server = MockServer::start().await;
        let client = RemoteKbClient::with_url_and_timeout(
            mock_server.uri(), 
            5
        );
        (mock_server, client)
    }

    #[tokio::test]
    async fn test_get_element_details_success() {
        let (mock_server, client) = setup_test_client().await;
        
        let expected_details = GraphElementDetails {
            id: "component-123".to_string(),
            name: "TestComponent".to_string(),
            description: Some("A test component".to_string()),
            element_type: "action".to_string(),
            version: Some("1.0".to_string()),
            framework: Some("cascade".to_string()),
            schema: None,
            inputs: None,
            outputs: None,
            steps: None,
            tags: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: None,
        };
        
        Mock::given(method("POST"))
            .and(path("/api/v1/kb/element-details"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({
                    "status": "success",
                    "data": {
                        "id": "component-123",
                        "name": "TestComponent",
                        "description": "A test component",
                        "element_type": "action",
                        "version": "1.0",
                        "framework": "cascade",
                        "schema": null,
                        "inputs": null,
                        "outputs": null,
                        "steps": null,
                        "tags": null,
                        "created_at": expected_details.created_at,
                        "updated_at": expected_details.updated_at,
                        "metadata": null
                    }
                }))
            )
            .mount(&mock_server)
            .await;
            
        let result = client.get_element_details("test-tenant", "component-123", None).await;
        
        assert!(result.is_ok(), "Expected Ok result, got {:?}", result);
        let element = result.unwrap();
        assert!(element.is_some(), "Expected Some element");
        
        let element = element.unwrap();
        assert_eq!(element.id, expected_details.id);
        assert_eq!(element.name, expected_details.name);
        assert_eq!(element.description, expected_details.description);
    }

    #[tokio::test]
    async fn test_get_element_details_not_found() {
        let (mock_server, client) = setup_test_client().await;
        
        Mock::given(method("POST"))
            .and(path("/api/v1/kb/element-details"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({
                    "status": "not_found",
                    "data": null
                }))
            )
            .mount(&mock_server)
            .await;
            
        let result = client.get_element_details("test-tenant", "nonexistent", None).await;
        
        assert!(result.is_ok(), "Expected Ok result, got {:?}", result);
        assert!(result.unwrap().is_none(), "Expected None for nonexistent element");
    }

    #[tokio::test]
    async fn test_search_related_artefacts() {
        let (mock_server, client) = setup_test_client().await;
        
        Mock::given(method("POST"))
            .and(path("/api/v1/kb/search-artifacts"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({
                    "status": "success",
                    "data": [
                        {
                            "id": "doc-123",
                            "content": "Test documentation",
                            "context_type": "documentation",
                            "relevance_score": 0.95,
                            "title": "Test Doc",
                            "metadata": null,
                            "related_element_id": null
                        },
                        {
                            "id": "example-456",
                            "content": "Example code snippet",
                            "context_type": "example",
                            "relevance_score": 0.85,
                            "title": "Example Usage",
                            "metadata": null,
                            "related_element_id": null
                        }
                    ]
                }))
            )
            .mount(&mock_server)
            .await;
            
        let result = client.search_related_artefacts(
            "test-tenant", 
            Some("test query"), 
            None, 
            None, 
            5
        ).await;
        
        assert!(result.is_ok(), "Expected Ok result, got {:?}", result);
        let artifacts = result.unwrap();
        assert_eq!(artifacts.len(), 2, "Expected 2 artifacts");
        
        assert_eq!(artifacts[0].id, "doc-123");
        assert_eq!(artifacts[0].context_type, "documentation");
        assert_eq!(artifacts[1].id, "example-456");
        assert_eq!(artifacts[1].context_type, "example");
    }

    // Additional tests would be implemented for validate_dsl and trace_step_input_source
    // following a similar pattern
} 
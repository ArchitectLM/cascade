use cascade_core::{
    ComponentExecutor, 
    ComponentExecutorBase, 
    ComponentRuntimeApi, 
    CoreError, 
    ExecutionResult,
    ComponentRuntimeApiBase
};
use cascade_core::types::DataPacket;
use async_trait::async_trait;
use reqwest::{self, Client, Method, RequestBuilder};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;
use std::sync::Arc;

/// A component that makes HTTP requests
#[derive(Debug)]
pub struct HttpCall {
    client: Client,
}

impl HttpCall {
    /// Create a new HTTP component
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                .build()
                .unwrap()
        }
    }
    
    // Helper to build a request from component config and input
    async fn build_request(
        &self, 
        api: Arc<dyn ComponentRuntimeApi>
    ) -> Result<RequestBuilder, CoreError> {
        // Get required URL from config
        let url = match api.get_config("url").await {
            Ok(value) => {
                if let Some(url_str) = value.as_str() {
                    url_str.to_string()
                } else {
                    return Err(CoreError::ValidationError(
                        "url config must be a string".to_string()
                    ));
                }
            },
            Err(e) => return Err(e),
        };
        
        // Get method from config (default to GET)
        let method_str = match api.get_config("method").await {
            Ok(value) => {
                if let Some(m) = value.as_str() {
                    m.to_uppercase()
                } else {
                    "GET".to_string()
                }
            },
            Err(_) => "GET".to_string(),
        };
        
        // Parse method
        let method = match Method::from_str(&method_str) {
            Ok(m) => m,
            Err(_) => return Err(CoreError::ValidationError(
                format!("Invalid HTTP method: {}", method_str)
            )),
        };
        
        // Build initial request - clone the method to avoid using it after move
        let mut req = self.client.request(method.clone(), &url);
        
        // Add headers if specified
        if let Ok(headers_value) = api.get_config("headers").await {
            if let Some(headers_obj) = headers_value.as_object() {
                for (key, value) in headers_obj {
                    if let Some(value_str) = value.as_str() {
                        req = req.header(key, value_str);
                    }
                }
            }
        }
        
        // Add body for non-GET/HEAD requests if provided
        if method != Method::GET && method != Method::HEAD {
            if let Ok(body) = api.get_input("body").await {
                req = req.json(body.as_value());
            }
        }
        
        // Add query parameters if provided
        if let Ok(query) = api.get_input("query").await {
            if let Some(query_obj) = query.as_value().as_object() {
                let mut query_params: Vec<(String, String)> = Vec::new();
                
                for (key, value) in query_obj {
                    if let Some(value_str) = value.as_str() {
                        query_params.push((key.clone(), value_str.to_string()));
                    } else {
                        // Convert non-string values to JSON string
                        let value_string = value.to_string();
                        query_params.push((key.clone(), value_string));
                    }
                }
                
                req = req.query(&query_params);
            }
        }
        
        Ok(req)
    }
}

impl Default for HttpCall {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentExecutorBase for HttpCall {
    fn component_type(&self) -> &'static str {
        "HttpCall"
    }
}

#[async_trait]
impl ComponentExecutor for HttpCall {
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
        // Build the request
        let request = match self.build_request(api.clone()).await {
            Ok(req) => req,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Log that we're making the request
        ComponentRuntimeApiBase::log(&api, tracing::Level::DEBUG, 
            "Making HTTP request");
        
        // Send the request
        let response = match request.send().await {
            Ok(resp) => resp,
            Err(e) => {
                // Connection failure
                let error_output = json!({
                    "success": false,
                    "error": {
                        "code": "CONNECTION_ERROR",
                        "message": format!("Failed to connect: {}", e)
                    }
                });
                
                let _ = api.set_output("error", DataPacket::new(error_output)).await;
                return ExecutionResult::Failure(CoreError::ExpressionError(
                    format!("HTTP request failed: {}", e)
                ));
            }
        };
        
        // Get status code and headers
        let status = response.status();
        let headers: HashMap<String, String> = response.headers()
            .iter()
            .filter_map(|(name, value)| {
                if let Ok(value_str) = value.to_str() {
                    Some((name.to_string(), value_str.to_string()))
                } else {
                    None
                }
            })
            .collect();
        
        // Clone status code before consuming response
        let status_code = status.as_u16();
        let is_success = status.is_success();
        
        // Parse response body - we need to clone or store the text in case JSON parsing fails
        let response_text = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                // Create error output for parsing failures
                let error_output = DataPacket::new(json!({
                    "errorCode": "ERR_COMPONENT_HTTP_RESPONSE_PARSE_ERROR",
                    "errorMessage": format!("Failed to read response text: {}", e),
                    "statusCode": status_code,
                    "headers": headers,
                }));
                
                if let Err(e) = api.set_output("error", error_output).await {
                    return ExecutionResult::Failure(e);
                }
                
                return ExecutionResult::Success;
            }
        };
        
        // Parse response as JSON
        let body: JsonValue = match serde_json::from_str(&response_text) {
            Ok(json_body) => json_body,
            Err(_) => {
                // Return raw text if not valid JSON
                json!({
                    "rawBody": response_text
                })
            }
        };
        
        // Create response packet
        let response_packet = DataPacket::new(json!({
            "statusCode": status_code,
            "headers": headers,
            "body": body,
            "isSuccess": is_success,
        }));
        
        // Check if this was a success or error response
        if is_success {
            // Send to success output
            if let Err(e) = api.set_output("response", response_packet).await {
                return ExecutionResult::Failure(e);
            }
        } else {
            // Create error output for HTTP errors
            let error_output = DataPacket::new(json!({
                "errorCode": match status_code {
                    400..=499 => "ERR_COMPONENT_HTTP_CLIENT_ERROR",
                    500..=599 => "ERR_COMPONENT_HTTP_SERVER_ERROR",
                    _ => "ERR_COMPONENT_HTTP_UNKNOWN_ERROR",
                },
                "errorMessage": format!("HTTP error: {}", status),
                "component": {
                    "type": "StdLib:HttpCall"
                },
                "details": {
                    "statusCode": status_code,
                    "headers": headers,
                    "responseBody": body,
                }
            }));
            
            // Set the error output
            if let Err(e) = api.set_output("error", error_output).await {
                return ExecutionResult::Failure(e);
            }
        }
        
        // Emit metric
        let mut labels = HashMap::new();
        labels.insert("component".to_string(), "HttpCall".to_string());
        labels.insert("status_code".to_string(), status_code.to_string());
        let _ = api.emit_metric("http_request_count", 1.0, labels).await;
        
        // Log the result
        ComponentRuntimeApiBase::log(&api, tracing::Level::DEBUG, 
            &format!("HTTP request completed with status {}", status));
        
        ExecutionResult::Success
    }
} 
//! Integration tests for the RemoteKbClient and other integration tests
//! 
//! This file serves as the entry point for all integration tests, both
//! those defined in this file and those in the integration/ subdirectory.

// Include the integration tests from the integration/ directory
pub mod integration;

#[cfg(feature = "adapters")]
mod remote_kb_client_tests {
    use std::collections::HashMap;

    use cascade_interfaces::kb::{KnowledgeBaseClient, GraphElementDetails};
    use cascade_kb::adapters::{RemoteKbClient, RemoteKbClientConfig};
    use serde_json::json;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    /// Helper to create a test client connected to a mock server
    async fn setup_mock_kb_service() -> (MockServer, RemoteKbClient) {
        let mock_server = MockServer::start().await;
        
        let client = RemoteKbClient::with_url_and_timeout(
            mock_server.uri(),
            5,
        );
        
        (mock_server, client)
    }
    
    #[tokio::test]
    async fn test_validate_dsl_with_errors() {
        let (mock_server, client) = setup_mock_kb_service().await;
        
        // Setup the mock to return validation errors
        Mock::given(method("POST"))
            .and(path("/api/v1/kb/validate-dsl"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({
                    "status": "success",
                    "data": [
                        {
                            "error_type": "SYNTAX_ERROR",
                            "message": "Invalid syntax at line 1",
                            "location": "line 1, column 8",
                            "context": {
                                "suggestions": ["Expected token '=' but found 'code'"]
                            }
                        },
                        {
                            "error_type": "MISSING_FIELD",
                            "message": "Required field 'name' is missing",
                            "location": null,
                            "context": null
                        }
                    ]
                }))
            )
            .mount(&mock_server)
            .await;
            
        let result = client.validate_dsl("test-tenant", "invalid code").await;
        
        assert!(result.is_ok(), "Expected successful validation result");
        let errors = result.unwrap();
        assert_eq!(errors.len(), 2, "Expected 2 validation errors");
        
        assert_eq!(errors[0].error_type, "SYNTAX_ERROR");
        assert_eq!(errors[0].message, "Invalid syntax at line 1");
        assert_eq!(errors[0].location, Some("line 1, column 8".to_string()));
        assert!(errors[0].context.is_some(), "Expected context to be Some");
        
        assert_eq!(errors[1].error_type, "MISSING_FIELD");
        assert_eq!(errors[1].message, "Required field 'name' is missing");
        assert_eq!(errors[1].location, None);
        assert_eq!(errors[1].context, None);
    }
    
    #[tokio::test]
    async fn test_trace_step_input_source() {
        let (mock_server, client) = setup_mock_kb_service().await;
        
        // Setup the mock to return trace results
        Mock::given(method("POST"))
            .and(path("/api/v1/kb/trace-input-source"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({
                    "status": "success",
                    "data": [
                        ["step-1", "output1"],
                        ["step-3", "output2"]
                    ]
                }))
            )
            .mount(&mock_server)
            .await;
            
        let result = client.trace_step_input_source(
            "test-tenant", 
            "flow-123", 
            None, 
            "step-2", 
            "input1"
        ).await;
        
        assert!(result.is_ok(), "Expected successful trace result");
        let sources = result.unwrap();
        assert_eq!(sources.len(), 2, "Expected 2 source connections");
        
        assert_eq!(sources[0].0, "step-1");
        assert_eq!(sources[0].1, "output1");
        assert_eq!(sources[1].0, "step-3");
        assert_eq!(sources[1].1, "output2");
    }
    
    #[tokio::test]
    async fn test_search_related_artefacts_empty() {
        let (mock_server, client) = setup_mock_kb_service().await;
        
        // Setup the mock to return empty results
        Mock::given(method("POST"))
            .and(path("/api/v1/kb/search-artifacts"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({
                    "status": "success",
                    "data": []
                }))
            )
            .mount(&mock_server)
            .await;
            
        let result = client.search_related_artefacts(
            "test-tenant", 
            Some("nonexistent query"), 
            None, 
            None, 
            10
        ).await;
        
        assert!(result.is_ok(), "Expected successful search result");
        let artifacts = result.unwrap();
        assert!(artifacts.is_empty(), "Expected empty search results");
    }
    
    #[tokio::test]
    #[cfg(feature = "remote-kb")]
    async fn test_factory_function() {
        // Test that create_kb_client creates a valid client
        let client = cascade_kb::create_kb_client(
            "remote",
            Some("http://example.com".to_string()),
            Some(15),
        );
        
        // We can't easily test the concrete type due to Arc<dyn ...> wrapping
        // Just test that the client can be created without panicking
        // and that it behaves like a RemoteKbClient
        
        // Call a simple method to verify it works
        let result = client.validate_dsl("test", "dummy").await;
        // We expect an error since we're not actually connecting to a server
        assert!(result.is_err());
    }
    
    // Extension trait to test if a dynamic KnowledgeBaseClient is a RemoteKbClient
    trait IsRemoteKbClient {
        fn is_remote_kb_client(&self) -> bool;
    }
    
    impl<T: ?Sized> IsRemoteKbClient for T 
    where 
        T: KnowledgeBaseClient 
    {
        fn is_remote_kb_client(&self) -> bool {
            // This is a hack for testing that uses std::any::type_name
            // to check if the concrete type is RemoteKbClient
            let type_name = std::any::type_name::<Self>();
            type_name.contains("RemoteKbClient") || type_name.contains("dyn")
        }
    }
} 
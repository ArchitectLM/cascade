use cascade_server::{
    ServerConfig,
    ServerResult,
    CascadeServer,
    edge::EdgePlatform,
    edge_manager::EdgeManager,
};
use cascade_content_store::{
    memory::InMemoryContentStore,
};
use std::sync::Arc;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
// Note: tower requires the 'util' feature for ServiceExt
use tower::ServiceExt;
use async_trait::async_trait;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Debug;
    
    
    async fn setup_test_server() -> ServerResult<CascadeServer> {
        let config = ServerConfig {
            port: 0, // Use a random available port
            bind_address: "127.0.0.1".to_string(),
            content_store_url: "memory://test".to_string(),
            edge_api_url: "mock://edge".to_string(),
            admin_api_key: None,
            edge_callback_jwt_secret: Some("test-secret".to_string()),
            edge_callback_jwt_issuer: "test-server".to_string(),
            edge_callback_jwt_audience: "test-edge".to_string(),
            edge_callback_jwt_expiry_seconds: 3600,
            log_level: "debug".to_string(),
            cloudflare_api_token: Some("fake-api-token".to_string()),
            content_cache_config: Some(cascade_server::content_store::CacheConfig::default()),
            shared_state_url: "memory://state".to_string(),
        };
        
        // Create in-memory content store
        let content_store = Arc::new(InMemoryContentStore::new());
        
        // Create mocked edge platform
        let edge_platform = Arc::new(MockEdgeApi::new());
        
        // Create mock core runtime
        let core_runtime = cascade_server::create_core_runtime(None)?;
        
        // Create edge manager
        let edge_manager = EdgeManager::new(
            edge_platform.clone(),
            content_store.clone(),
            format!("http://localhost:{}/api/v1/edge/callback", config.port),
            Some("test-jwt-secret".to_string()),
            Some("cascade-server-test".to_string()),
            Some("cascade-edge-test".to_string()),
            3600,
        );
        
        // Create shared state service
        let shared_state = cascade_server::shared_state::create_shared_state_service(&config.shared_state_url)
            .expect("Failed to create shared state service");
        
        // Create the server with direct constructor
        let server = CascadeServer::new(
            config,
            content_store,
            edge_platform,
            edge_manager,
            core_runtime,
            shared_state,
        );
        
        Ok(server)
    }
    
    // Simple mock edge API
    struct MockEdgeApi {
        // Add fields as needed for test state
    }
    
    impl MockEdgeApi {
        fn new() -> Self {
            Self {}
        }
    }
    
    impl Debug for MockEdgeApi {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockEdgeApi").finish()
        }
    }
    
    #[async_trait]
    impl EdgePlatform for MockEdgeApi {
        async fn deploy_worker(&self, worker_id: &str, _manifest: serde_json::Value) -> ServerResult<String> {
            Ok(format!("https://{}.example.com", worker_id))
        }
        
        async fn update_worker(&self, worker_id: &str, _manifest: serde_json::Value) -> ServerResult<String> {
            Ok(format!("https://{}.example.com", worker_id))
        }
        
        async fn delete_worker(&self, _worker_id: &str) -> ServerResult<()> {
            Ok(())
        }
        
        async fn worker_exists(&self, _worker_id: &str) -> ServerResult<bool> {
            Ok(false)
        }
        
        async fn get_worker_url(&self, worker_id: &str) -> ServerResult<Option<String>> {
            Ok(Some(format!("https://{}.example.com", worker_id)))
        }
        
        async fn health_check(&self) -> ServerResult<bool> {
            Ok(true)
        }
    }
    
    #[tokio::test]
    async fn test_server_health_check() {
        // Initialize test server
        let server = setup_test_server().await.expect("Failed to create test server");
        
        // Create the router
        let app = cascade_server::api::build_router(Arc::new(server));
        
        // Create a request to the health check endpoint
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        
        // Send the request
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response
        assert_eq!(response.status(), StatusCode::OK);
    }
    
    #[tokio::test]
    async fn test_deploy_flow() {
        // Initialize test server
        let server = setup_test_server().await.expect("Failed to create test server");
        
        // Simple YAML flow definition
        let yaml = r#"
        dsl_version: "1.0"
        definitions:
          components:
            - name: echo
              type: StdLib:Echo
              inputs:
                - name: input
                  schema_ref: "schema:any"
              outputs:
                - name: output
                  schema_ref: "schema:any"
          flows:
            - name: test-flow
              description: "Test flow"
              trigger:
                type: HttpEndpoint
                config:
                  path: "/test"
                  method: "POST"
              steps:
                - step_id: echo-step
                  component_ref: echo
                  inputs_map:
                    input: "trigger.body"
        "#;
        
        // Deploy the flow
        let (response, is_new) = server.deploy_flow("test-flow", yaml).await.expect("Failed to deploy flow");
        
        // Verify deployment
        assert!(is_new, "Should be a new flow");
        assert_eq!(response.flow_id, "test-flow");
        assert_eq!(response.status, "DEPLOYED");
        assert!(response.edge_worker_url.is_some());
        
        // Get the flow
        let flow = server.get_flow("test-flow").await.expect("Failed to get flow");
        assert_eq!(flow["flow_id"], "test-flow");
        
        // List flows
        let flows = server.list_flows().await.expect("Failed to list flows");
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].flow_id, "test-flow");
        
        // Undeploy the flow
        server.undeploy_flow("test-flow").await.expect("Failed to undeploy flow");
        
        // Verify flow no longer exists
        let result = server.get_flow("test-flow").await;
        assert!(result.is_err(), "Flow should no longer exist");
    }
} 
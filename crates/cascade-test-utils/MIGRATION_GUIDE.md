# Migrating Tests to Use Cascade Test Utils

This guide provides steps and examples for migrating existing Cascade Platform tests to use the new `cascade-test-utils` crate.

## Migration Benefits

- **Standardized testing utilities** across all Cascade projects
- **Reduced duplication** of test infrastructure code
- **Improved maintainability** with centralized test utilities
- **Better test isolation** with proper mocking
- **Enhanced readability** with consistent test patterns

## Migration Steps

### 1. Add Dependency

Add `cascade-test-utils` to your project's `Cargo.toml`:

```toml
[dev-dependencies]
cascade-test-utils = { version = "0.1.0", path = "../cascade-test-utils" }
```

### 2. Replace Custom Mocks with Standard Mocks

Before:
```rust
// Custom mock implementation
struct MockContentStorage {
    manifests: RwLock<HashMap<String, serde_json::Value>>,
}

impl MockContentStorage {
    fn new() -> Self {
        Self {
            manifests: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl ContentStorage for MockContentStorage {
    async fn store_manifest(&self, key: &str, manifest: serde_json::Value) -> Result<(), StorageError> {
        self.manifests.write().unwrap().insert(key.to_string(), manifest);
        Ok(())
    }
    
    // ... other implementations
}
```

After:
```rust
use cascade_test_utils::mocks::create_mock_content_storage;

#[tokio::test]
async fn test_with_mock_storage() {
    // Create a mock with default behavior
    let mut mock_storage = create_mock_content_storage();
    
    // Configure custom behavior
    mock_storage.expect_store_manifest()
        .returning(|key, manifest| {
            println!("Storing manifest for key: {}", key);
            Ok(())
        });
        
    // Use the mock in your test
    let result = mock_storage.store_manifest("test-key", serde_json::json!({})).await;
    assert!(result.is_ok());
}
```

### 3. Use In-Memory Implementations Instead of Simple Mocks

Before:
```rust
// Simple in-memory implementation
let mut content_store = HashMap::new();

// Test code that manipulates the HashMap directly
content_store.insert("key1".to_string(), json!({"data": "value"}));
assert!(content_store.contains_key("key1"));
```

After:
```rust
use cascade_test_utils::implementations::InMemoryContentStore;

#[tokio::test]
async fn test_with_in_memory_store() {
    // Thread-safe in-memory implementation
    let content_store = InMemoryContentStore::new();
    
    // Use the proper interface
    content_store.store_manifest("key1", json!({"data": "value"})).await.unwrap();
    let exists = content_store.get_manifest("key1").await.unwrap().is_some();
    assert!(exists);
}
```

### 4. Replace Custom Test Server with TestServerBuilder

Before:
```rust
// Custom test setup code
async fn setup_test_server() -> TestServer {
    let content_storage = Arc::new(MockContentStorage::new());
    let core_runtime = Arc::new(MockCoreRuntime::new());
    let edge_platform = Arc::new(MockEdgePlatform::new());
    
    let app = Router::new()
        .route("/health", get(health_handler))
        .layer(Extension(content_storage.clone()))
        .layer(Extension(core_runtime.clone()))
        .layer(Extension(edge_platform.clone()));
    
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let server = axum::serve(listener, app).handle();
    
    TestServer {
        server,
        addr,
        content_storage,
        core_runtime,
        edge_platform,
    }
}
```

After:
```rust
use cascade_test_utils::builders::TestServerBuilder;

#[tokio::test]
async fn test_with_test_server() {
    // Use the builder pattern for test server setup
    let server = TestServerBuilder::new()
        .with_content_store(InMemoryContentStore::new())
        .with_core_runtime(create_mock_core_runtime_api())
        .with_edge_platform(create_mock_edge_platform())
        .build()
        .await
        .unwrap();
    
    // Use the server in your test
    let response = server.client
        .get(&format!("{}/health", server.base_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
}
```

### 5. Use Assertion Helpers for Complex Validations

Before:
```rust
// Manual validation logic
let manifest = get_manifest().await.unwrap();
assert!(manifest.get("flow").is_some(), "Missing flow field");
assert_eq!(
    manifest.get("flow").unwrap().get("id").unwrap().as_str().unwrap(),
    "test-flow",
    "Incorrect flow ID"
);
```

After:
```rust
use cascade_test_utils::assertions::manifest::assert_manifest_has_flow_id;

#[tokio::test]
async fn test_with_assertion_helpers() {
    let manifest = get_manifest().await.unwrap();
    
    // Use specialized assertion helpers
    assert_manifest_has_flow_id(&manifest, "test-flow").unwrap();
}
```

### 6. Generate Test Data with Data Generators

Before:
```rust
// Hardcoded test data
let flow_dsl = r#"
flow:
  id: test-flow
  components:
    - id: http-component
      type: HttpCall
      config:
        url: "https://example.com/api"
        method: GET
  steps:
    - id: http-step
      component: http-component
      deployment_location: server
"#;
```

After:
```rust
use cascade_test_utils::data_generators::{create_http_flow_dsl, create_http_component_config};

#[tokio::test]
async fn test_with_generated_data() {
    // Generate test data programmatically
    let flow_dsl = create_http_flow_dsl(
        "test-flow",
        "https://example.com/api",
        "GET"
    );
    
    // Generate component config
    let component_config = create_http_component_config(
        "https://example.com/api",
        "GET"
    );
}
```

## Advanced Migration: Component Behavior Simulation

Before:
```rust
// Custom component executor with hardcoded behavior
struct MockHttpComponent;

#[async_trait]
impl ComponentExecutor for MockHttpComponent {
    async fn execute(&self, runtime_api: Arc<dyn ComponentRuntimeAPI>) -> Result<(), ComponentError> {
        // Set outputs to simulate HTTP response
        runtime_api.set_output(
            "statusCode",
            DataPacket::new(json!(200)),
        ).await?;
        
        runtime_api.set_output(
            "body",
            DataPacket::new(json!({"success": true})),
        ).await?;
        
        runtime_api.set_output(
            "headers",
            DataPacket::new(json!({})),
        ).await?;
        
        Ok(())
    }
}
```

After:
```rust
use cascade_test_utils::mocks::component_behaviors::{
    HttpResponseData,
    simulate_http_response,
};
use cascade_test_utils::mocks::component::{
    create_mock_component_executor,
    create_mock_component_runtime_api,
};

#[tokio::test]
async fn test_with_component_behavior_simulation() {
    // Create a mock component executor
    let mut mock_executor = create_mock_component_executor();
    
    // Configure the executor to use the HTTP simulation
    mock_executor
        .expect_execute()
        .returning(|runtime_api| {
            Box::pin(async move {
                let response_data = HttpResponseData::new(
                    200,
                    json!({"success": true})
                );
                
                simulate_http_response(runtime_api, response_data).await
            })
        });
    
    // Use the mock executor in your test
    let runtime_api = Arc::new(create_mock_component_runtime_api("http-test"));
    let result = mock_executor.execute(runtime_api).await;
    assert!(result.is_ok());
}
```

## Complete Example

Here's a complete migration example for a test that deploys and triggers a flow:

Before:
```rust
#[tokio::test]
async fn test_flow_deployment_and_execution() {
    // Custom setup code
    let content_storage = Arc::new(MockContentStorage::new());
    let core_runtime = Arc::new(MockCoreRuntime::new());
    let test_server = setup_test_server(content_storage.clone(), core_runtime.clone()).await;
    
    // Test flow DSL
    let flow_dsl = r#"
    flow:
      id: test-flow
      components:
        - id: http-component
          type: HttpCall
          config:
            url: "https://example.com/api"
            method: GET
      steps:
        - id: http-step
          component: http-component
          deployment_location: server
    "#;
    
    // Deploy flow
    let deploy_result = core_runtime.deploy_dsl("test-flow", flow_dsl).await;
    assert!(deploy_result.is_ok());
    
    // Trigger flow
    let instance_id = core_runtime.trigger_flow(
        "test-flow",
        json!({"input": "value"})
    ).await.unwrap();
    
    // Get flow state
    let instance = core_runtime.get_flow_state(&instance_id).await.unwrap().unwrap();
    
    // Manual assertions
    assert_eq!(instance.flow_id, "test-flow");
    assert!(matches!(instance.state, FlowState::Completed));
}
```

After:
```rust
use cascade_test_utils::builders::TestServerBuilder;
use cascade_test_utils::data_generators::create_http_flow_dsl;
use cascade_test_utils::mocks::{FlowState, FlowInstance};
use cascade_test_utils::assertions::flow_state::assert_flow_completed_with_output;

#[tokio::test]
async fn test_flow_deployment_and_execution() {
    // Use TestServerBuilder for setup
    let server = TestServerBuilder::new()
        .build()
        .await
        .unwrap();
    
    // Generate flow DSL
    let flow_dsl = create_http_flow_dsl(
        "test-flow",
        "https://example.com/api",
        "GET"
    );
    
    // Deploy flow
    let deploy_result = server.core_runtime.deploy_dsl("test-flow", &flow_dsl).await;
    assert!(deploy_result.is_ok());
    
    // Trigger flow
    let instance_id = server.core_runtime.trigger_flow(
        "test-flow",
        json!({"input": "value"})
    ).await.unwrap();
    
    // Get flow state
    let instance = server.core_runtime.get_flow_state(&instance_id)
        .await
        .unwrap()
        .unwrap();
    
    // Use assertion helpers
    assert_flow_completed_with_output(
        &instance,
        json!({"result": "success"})
    ).unwrap();
}
```

## Migration Checklist

- [ ] Add `cascade-test-utils` to your project's dev-dependencies
- [ ] Replace custom mock implementations with standard mocks
- [ ] Replace simple in-memory implementations with thread-safe implementations
- [ ] Replace custom test server setup with TestServerBuilder
- [ ] Use assertion helpers for complex validations
- [ ] Use data generators for test data
- [ ] Use component behavior simulators for component testing

## Best Practices

1. **Incremental Migration**: Migrate one test file at a time to ensure nothing breaks
2. **Keep Both Versions Initially**: Keep both original and migrated versions until you confirm the migrated version works correctly
3. **Run Tests Often**: Run tests frequently during migration to catch issues early
4. **Use Type Annotations**: Add explicit type annotations when using the new utilities to help with understanding
5. **Document Migration Decisions**: Document any decisions made during migration for future reference

Once the migration is complete, you should have more maintainable, readable, and robust tests that follow consistent patterns across the Cascade Platform.

## Need Help?

See the [README.md](./README.md) for more detailed information on the available utilities and their usage. 
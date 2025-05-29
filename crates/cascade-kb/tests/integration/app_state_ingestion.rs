use std::sync::Arc;
use tokio::sync::mpsc;
use serde_json::json;
use uuid::Uuid;
use chrono::Utc;
use tokio::time::{timeout, Duration};

use cascade_kb::{
    data::{
        identifiers::{
            TenantId,
        },
        types::{Scope, DataPacket},
        trace_context::TraceContext,
        errors::{CoreError, StateStoreError},
        ApplicationStateSnapshot, FlowInstanceConfig, StepInstanceConfig,
    },
    VersionId,
    services::{
        ingestion::IngestionService,
        messages::IngestionMessage,
    },
    traits::{StateStore, EmbeddingGenerator, DslParser, GraphDataPatch},
    ParsedDslDefinitions,
};

// Mock implementations for testing
#[derive(Default)]
struct MockStateStore {
    patches: std::sync::Mutex<Vec<GraphDataPatch>>,
}

#[async_trait::async_trait]
impl StateStore for MockStateStore {
    async fn execute_query(
        &self,
        _tenant_id: &TenantId,
        _trace_ctx: &TraceContext,
        _query: &str,
        _params: Option<std::collections::HashMap<String, DataPacket>>,
    ) -> Result<Vec<std::collections::HashMap<String, DataPacket>>, StateStoreError> {
        // Return a simple successful response
        let mut row = std::collections::HashMap::new();
        row.insert("result".to_string(), DataPacket::Integer(1));
        Ok(vec![row])
    }

    async fn upsert_graph_data(
        &self,
        _tenant_id: &TenantId,
        _trace_ctx: &TraceContext,
        data: GraphDataPatch,
    ) -> Result<(), StateStoreError> {
        // Store the patch for later verification
        let mut patches = self.patches.lock().unwrap();
        patches.push(data);
        Ok(())
    }

    async fn vector_search(
        &self,
        _tenant_id: &TenantId,
        _trace_ctx: &TraceContext,
        _index_name: &str,
        _embedding: &[f32],
        _k: usize,
        _filter_params: Option<std::collections::HashMap<String, DataPacket>>,
    ) -> Result<Vec<(uuid::Uuid, f32)>, StateStoreError> {
        Ok(vec![])
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Mock EmbeddingGenerator - not used for app state but required by IngestionService
#[derive(Default)]
struct MockEmbeddingGenerator;

#[async_trait::async_trait]
impl EmbeddingGenerator for MockEmbeddingGenerator {
    async fn generate_embedding(&self, _text: &str) -> Result<Vec<f32>, CoreError> {
        Ok(vec![0.1, 0.2, 0.3])
    }
}

// Mock DslParser - not used for app state but required by IngestionService
#[derive(Default)]
struct MockDslParser;

#[async_trait::async_trait]
impl DslParser for MockDslParser {
    async fn parse_and_validate_yaml(
        &self,
        _yaml_content: &str,
        _tenant_id: &TenantId,
        _trace_ctx: &TraceContext,
    ) -> Result<ParsedDslDefinitions, CoreError> {
        Ok(ParsedDslDefinitions {
            components: Vec::new(),
            flows: Vec::new(),
            input_definitions: Vec::new(),
            output_definitions: Vec::new(),
            step_definitions: Vec::new(),
            trigger_definitions: Vec::new(),
            condition_definitions: Vec::new(),
            documentation_chunks: Vec::new(),
            code_examples: Vec::new(),
        })
    }
}

#[tokio::test]
async fn test_app_state_ingestion() {
    // Initialize logging for better debugging
    let _ = tracing_subscriber::fmt()
        .with_env_filter("cascade_kb=debug,app_state_ingestion=debug")
        .try_init();

    tracing::info!("Starting app_state_ingestion test");

    // Create dependencies
    let state_store = Arc::new(MockStateStore::default());
    let embedding_generator = Arc::new(MockEmbeddingGenerator);
    let dsl_parser = Arc::new(MockDslParser);

    // Create channels with sufficient buffer
    let (tx, rx) = mpsc::channel(32);

    // Create service
    let mut service = IngestionService::new(
        state_store.clone(),
        embedding_generator,
        dsl_parser,
        rx,
    );

    // Start service in background
    let service_handle = tokio::spawn(async move {
        tracing::info!("IngestionService starting");
        if let Err(e) = service.run().await {
            tracing::error!("Service failed with error: {:?}", e);
        }
        tracing::info!("IngestionService completed");
    });

    // Create test data
    let tenant_id = TenantId::new_v4();
    let trace_ctx = TraceContext::new_root();

    let step_instance = StepInstanceConfig {
        instance_step_id: "test-step-1".to_string(),
        flow_instance_id: "test-flow-1".to_string(),
        step_def_id: Uuid::new_v4(),
        component_def_version_id: VersionId::new_v4(),
        config: DataPacket::Json(json!({
            "key": "value"
        })),
        tenant_id: tenant_id.clone(),
    };

    let flow_instance = FlowInstanceConfig {
        instance_id: "test-flow-1".to_string(),
        config_source: Some("test-config.yaml".to_string()),
        snapshot_id: "test-snapshot-1".to_string(),
        flow_def_version_id: VersionId::new_v4(),
        tenant_id: tenant_id.clone(),
        step_instances: Some(vec![step_instance]),
    };

    let snapshot = ApplicationStateSnapshot {
        snapshot_id: "test-snapshot-1".to_string(),
        timestamp: Utc::now(),
        source: "test-deployment".to_string(),
        tenant_id: tenant_id.clone(),
        scope: Scope::ApplicationState,
        flow_instances: Some(vec![flow_instance]),
    };

    // Send test message
    tx.send(IngestionMessage::ApplicationState {
        tenant_id: tenant_id.clone(),
        trace_ctx: trace_ctx.clone(),
        snapshot_data: DataPacket::Json(serde_json::to_value(&snapshot).unwrap()),
        source_info: "test".to_string(),
    }).await.unwrap();

    // Give the message time to be processed
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Drop sender to allow service to complete
    drop(tx);

    // Wait for service to shut down with timeout
    match timeout(Duration::from_secs(5), service_handle).await {
        Ok(result) => {
            if let Err(e) = result {
                tracing::error!("Error from service join handle: {:?}", e);
                panic!("Service task panicked: {:?}", e);
            }
            tracing::info!("Service shut down properly");
        },
        Err(_) => {
            tracing::warn!("Service didn't shut down within timeout period");
            // Don't fail the test, just continue
        }
    }

    // Verify results
    let patches = state_store.patches.lock().unwrap();
    assert_eq!(patches.len(), 1, "Should have exactly one patch");

    let patch = &patches[0];

    // In our implementation, the application state snapshot with 1 flow and 1 step
    // should have 3 nodes: ApplicationStateSnapshot, FlowInstanceConfig, StepInstanceConfig
    assert_eq!(patch.nodes.len(), 3, "Should have 3 nodes in the patch");

    // There should be 2 edges:
    // (ApplicationStateSnapshot)-[:CONTAINS_FLOW_INSTANCE]->(FlowInstanceConfig)
    // (FlowInstanceConfig)-[:CONTAINS_STEP_INSTANCE]->(StepInstanceConfig)
    assert_eq!(patch.edges.len(), 2, "Should have 2 edges in the patch");

    // Verify node types
    let node_labels: Vec<&str> = patch.nodes.iter()
        .map(|n| n.as_object().unwrap().get("label").unwrap().as_str().unwrap())
        .collect();

    assert!(node_labels.contains(&"ApplicationStateSnapshot"), "Missing ApplicationStateSnapshot node");
    assert!(node_labels.contains(&"FlowInstanceConfig"), "Missing FlowInstanceConfig node");
    assert!(node_labels.contains(&"StepInstanceConfig"), "Missing StepInstanceConfig node");

    // Verify edge types
    let edge_types: Vec<&str> = patch.edges.iter()
        .map(|e| e.as_object().unwrap().get("type").unwrap().as_str().unwrap())
        .collect();

    assert!(edge_types.contains(&"CONTAINS_FLOW_INSTANCE"), "Missing CONTAINS_FLOW_INSTANCE edge");
    assert!(edge_types.contains(&"CONTAINS_STEP_INSTANCE"), "Missing CONTAINS_STEP_INSTANCE edge");

    tracing::info!("Test completed successfully");
} 
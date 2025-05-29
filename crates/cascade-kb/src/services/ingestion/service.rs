use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error, instrument};

use crate::{
    data::{
        TraceContext,
        CoreError,
        TenantId,
        DataPacket,
        SourceType,
        Scope,
        DocumentationChunk,
        ComponentDefinition,
        identifiers::{ComponentDefinitionId, VersionId},
        entities::FlowDefinition,
    },
    services::messages::IngestionMessage,
    traits::{StateStore, EmbeddingGenerator, DslParser, ParsedDslDefinitions},
};

use super::mapping::{app_state, yaml_defs};

/// Service responsible for ingesting and processing data into the Knowledge Base.
/// Processes messages received via channel, transforms them into graph model representation,
/// and persists them via the StateStore.
pub struct IngestionService {
    state_store: Arc<dyn StateStore>,
    embedding_generator: Arc<dyn EmbeddingGenerator>,
    dsl_parser: Arc<dyn DslParser>,
    ingestion_rx: mpsc::Receiver<IngestionMessage>,
}

impl IngestionService {
    /// Creates a new IngestionService instance with the provided dependencies and message channel.
    pub fn new(
        state_store: Arc<dyn StateStore>,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
        dsl_parser: Arc<dyn DslParser>,
        ingestion_rx: mpsc::Receiver<IngestionMessage>,
    ) -> Self {
        Self {
            state_store,
            embedding_generator,
            dsl_parser,
            ingestion_rx,
        }
    }

    /// Runs the service, processing messages from the channel.
    /// Each message is processed in a separate task to avoid blocking the channel.
    pub async fn run(&mut self) -> Result<(), CoreError> {
        info!("IngestionService started");
        while let Some(msg) = self.ingestion_rx.recv().await {
            // Clone Arc dependencies for the task
            let state_store = Arc::clone(&self.state_store);
            let embedding_generator = Arc::clone(&self.embedding_generator);
            let dsl_parser = Arc::clone(&self.dsl_parser);

            // Spawn a task to process the message
            let _handle = tokio::spawn(async move {
                match msg {
                    IngestionMessage::YamlDefinitions { tenant_id, trace_ctx, yaml_content, scope, source_info } => {
                        info!(
                            tenant_id = ?tenant_id,
                            trace_id = ?trace_ctx.trace_id,
                            "Processing YAML definitions"
                        );

                        // Process YAML definitions
                        if let Err(e) = process_yaml_definitions(
                            &dsl_parser, 
                            &embedding_generator, 
                            &state_store, 
                            &yaml_content, 
                            &tenant_id, 
                            &trace_ctx, 
                            scope, 
                            source_info
                        ).await {
                            error!(
                                tenant_id = ?tenant_id,
                                trace_id = ?trace_ctx.trace_id,
                                error = %e,
                                "Failed to process YAML definitions"
                            );
                        }
                    },
                    IngestionMessage::ApplicationState { tenant_id, trace_ctx, snapshot_data, source_info } => {
                        info!(
                            tenant_id = ?tenant_id,
                            trace_id = ?trace_ctx.trace_id,
                            "Processing application state"
                        );

                        // Process application state
                        if let Err(e) = process_application_state(
                            &state_store, 
                            snapshot_data, 
                            &tenant_id, 
                            &trace_ctx, 
                            source_info
                        ).await {
                            error!(
                                tenant_id = ?tenant_id,
                                trace_id = ?trace_ctx.trace_id,
                                error = %e,
                                "Failed to process application state"
                            );
                        }
                    },
                    // Handle the new message types with stub implementations
                    IngestionMessage::DocumentChunk { tenant_id, trace_ctx, chunk, linked_entity_type, linked_entity_id } => {
                        info!(
                            tenant_id = ?tenant_id,
                            trace_id = ?trace_ctx.trace_id,
                            "Processing document chunk"
                        );
                        
                        // Process document chunk and persist to KB
                        if let Err(e) = process_document_chunk(
                            &embedding_generator,
                            &state_store,
                            chunk,
                            linked_entity_type,
                            linked_entity_id,
                            &tenant_id,
                            &trace_ctx,
                        ).await {
                            error!(
                                tenant_id = ?tenant_id,
                                trace_id = ?trace_ctx.trace_id,
                                error = %e,
                                "Failed to process document chunk"
                            );
                        }
                    },
                    IngestionMessage::CreateVersionSet { tenant_id, trace_context, .. } => {
                        info!(
                            tenant_id = ?tenant_id,
                            trace_id = ?trace_context.trace_id,
                            "Processing version set creation (not implemented yet)"
                        );
                        // TODO: Implement version set creation
                    },
                    IngestionMessage::CreateVersion { tenant_id, trace_context, .. } => {
                        info!(
                            tenant_id = ?tenant_id,
                            trace_id = ?trace_context.trace_id,
                            "Processing version creation (not implemented yet)"
                        );
                        // TODO: Implement version creation
                    }
                }
            });
        }

        // Channel has been closed
        info!("IngestionService channel closed, shutting down");
        Ok(())
    }

    /// Processes YAML definitions and persists them to the knowledge base.
    /// This is a public method that can be called directly without going through the message channel.
    #[instrument(skip(self, yaml_content), fields(tenant_id = ?tenant_id, trace_id = ?trace_ctx.trace_id))]
    pub async fn process_yaml_definitions(
        &self,
        yaml_content: &str,
        tenant_id: &TenantId,
        trace_ctx: &TraceContext,
        scope: crate::data::Scope,
        source_info: Option<String>,
    ) -> Result<(), CoreError> {
        // We'll delegate to the standalone function
        process_yaml_definitions(
            &self.dsl_parser, 
            &self.embedding_generator, 
            &self.state_store, 
            yaml_content, 
            tenant_id, 
            trace_ctx, 
            scope, 
            source_info
        ).await
    }

    /// Processes application state and persists it to the knowledge base.
    /// This is a public method that can be called directly without going through the message channel.
    #[instrument(skip(self, snapshot_data), fields(tenant_id = ?tenant_id, trace_id = ?trace_ctx.trace_id))]
    pub async fn process_application_state(
        &self,
        snapshot_data: DataPacket,
        tenant_id: &TenantId,
        trace_ctx: &TraceContext,
        source_info: String,
    ) -> Result<(), CoreError> {
        // We'll delegate to the standalone function
        process_application_state(
            &self.state_store, 
            snapshot_data, 
            tenant_id, 
            trace_ctx, 
            source_info
        ).await
    }
}

/// Standalone function to process YAML definitions.
/// This allows both the service's run method and public API method to use the same logic.
#[instrument(skip(dsl_parser, embedding_generator, state_store, yaml_content), fields(tenant_id = ?tenant_id, trace_id = ?trace_ctx.trace_id))]
async fn process_yaml_definitions(
    dsl_parser: &Arc<dyn DslParser>,
    embedding_generator: &Arc<dyn EmbeddingGenerator>,
    state_store: &Arc<dyn StateStore>,
    yaml_content: &str,
    tenant_id: &TenantId,
    trace_ctx: &TraceContext,
    scope: crate::data::Scope,
    source_info: Option<String>, 
) -> Result<(), CoreError> {
    // Parse YAML content
    let parsed_defs = dsl_parser.parse_and_validate_yaml(yaml_content, tenant_id, trace_ctx)
        .await
        .map_err(|e| CoreError::ingestion_error_with_context(
            "Failed to parse YAML content",
            Some(*tenant_id),
            Some(trace_ctx),
            Some(e),
        ))?;

    // Process parsed definitions
    let mut processed_defs = parsed_defs;
    
    // Generate embeddings for documentation chunks
    processed_defs = generate_embeddings_for_docs(embedding_generator, processed_defs, tenant_id, trace_ctx).await?;
    
    // Transform parsed definitions into GraphPatch
    let patch = yaml_defs::transform_to_graph_patch(&processed_defs, tenant_id, scope)
        .map_err(|e| CoreError::ingestion_error_with_context(
            "Failed to transform definitions to graph patch",
            Some(*tenant_id),
            Some(trace_ctx),
            Some(e),
        ))?;
    
    // Persist the graph patch
    info!(
        tenant_id = ?tenant_id,
        trace_id = ?trace_ctx.trace_id,
        nodes = patch.nodes.len(),
        edges = patch.edges.len(),
        "Upserting graph data"
    );

    state_store.upsert_graph_data(tenant_id, trace_ctx, patch).await
        .map_err(|e| CoreError::db_error_with_context(
            "Failed to upsert graph data",
            Some(*tenant_id),
            Some(trace_ctx),
            Some(e),
        ))?;

    info!(
        tenant_id = ?tenant_id,
        trace_id = ?trace_ctx.trace_id,
        "Successfully processed YAML definitions"
    );

    Ok(())
}

/// Standalone function to process application state.
/// This allows both the service's run method and public API method to use the same logic.
#[instrument(skip(state_store, snapshot_data), fields(tenant_id = ?tenant_id, trace_id = ?trace_ctx.trace_id))]
async fn process_application_state(
    state_store: &Arc<dyn StateStore>,
    snapshot_data: DataPacket,
    tenant_id: &TenantId,
    trace_ctx: &TraceContext,
    source_info: String,
) -> Result<(), CoreError> {
    // Map application state to graph patch
    let graph_patch = app_state::map_application_state(&snapshot_data, tenant_id)
        .map_err(|e| CoreError::ingestion_error_with_context(
            "Failed to map application state to graph patch",
            Some(*tenant_id),
            Some(trace_ctx),
            Some(e),
        ))?;

    info!(
        tenant_id = ?tenant_id,
        trace_id = ?trace_ctx.trace_id,
        nodes = graph_patch.nodes.len(),
        edges = graph_patch.edges.len(),
        "Upserting application state data"
    );

    // Persist the graph patch
    state_store.upsert_graph_data(tenant_id, trace_ctx, graph_patch).await
        .map_err(|e| CoreError::db_error_with_context(
            format!("Failed to upsert application state from source: {}", source_info),
            Some(*tenant_id),
            Some(trace_ctx),
            Some(e),
        ))?;

    info!(
        tenant_id = ?tenant_id,
        trace_id = ?trace_ctx.trace_id,
        "Successfully processed application state"
    );

    Ok(())
}

/// Helper function to generate embeddings for documentation chunks.
#[instrument(skip(embedding_generator, parsed_defs), fields(chunks_count = parsed_defs.documentation_chunks.len()))]
async fn generate_embeddings_for_docs(
    embedding_generator: &Arc<dyn EmbeddingGenerator>,
    mut parsed_defs: ParsedDslDefinitions,
    tenant_id: &TenantId,
    trace_ctx: &TraceContext,
) -> Result<ParsedDslDefinitions, CoreError> {
    for chunk in &mut parsed_defs.documentation_chunks {
        info!(
            tenant_id = ?tenant_id,
            trace_id = ?trace_ctx.trace_id,
            chunk_id = ?chunk.id,
            "Generating embedding for documentation chunk"
        );

        match embedding_generator.generate_embedding(&chunk.text).await {
            Ok(embedding) => {
                chunk.embedding = embedding;
                info!(
                    tenant_id = ?tenant_id,
                    trace_id = ?trace_ctx.trace_id,
                    chunk_id = ?chunk.id,
                    "Successfully generated embedding for chunk"
                );
            }
            Err(e) => {
                // Log warning but continue processing - empty embedding will be used
                warn!(
                    tenant_id = ?tenant_id,
                    trace_id = ?trace_ctx.trace_id,
                    chunk_id = ?chunk.id,
                    error = %e,
                    "Failed to generate embedding for chunk, using empty embedding"
                );
                // Use empty embedding rather than failing the whole ingestion
                chunk.embedding = Vec::new();
            }
        }
    }
    
    Ok(parsed_defs)
}

/// Process a document chunk and persist it to the knowledge base.
/// This includes generating an embedding and connecting it to the referenced entity.
#[instrument(skip(embedding_generator, state_store, chunk), fields(tenant_id = ?tenant_id, trace_id = ?trace_ctx.trace_id))]
async fn process_document_chunk(
    embedding_generator: &Arc<dyn EmbeddingGenerator>,
    state_store: &Arc<dyn StateStore>,
    mut chunk: crate::data::DocumentationChunk,
    linked_entity_type: Option<String>,
    linked_entity_id: Option<String>,
    tenant_id: &TenantId,
    trace_ctx: &TraceContext,
) -> Result<(), CoreError> {
    // 1. Generate embedding for the chunk text
    let embedding = embedding_generator.generate_embedding(&chunk.text).await
        .map_err(|e| CoreError::embedding_error_with_context(
            "Failed to generate embedding for document chunk",
            Some(*tenant_id),
            Some(trace_ctx),
            Some(e),
        ))?;
    
    info!(
        tenant_id = ?tenant_id,
        trace_id = ?trace_ctx.trace_id,
        chunk_id = ?chunk.id,
        embedding_size = embedding.len(),
        "Generated embedding for document chunk"
    );
    
    // 2. Set the embedding in the chunk
    chunk.embedding = embedding;
    
    // 3. Create a GraphDataPatch to store the chunk and potential relationship
    let mut patch = crate::traits::GraphDataPatch {
        nodes: Vec::new(),
        edges: Vec::new(),
    };
    
    // 4. Add the DocumentationChunk node
    let chunk_node = serde_json::json!({
        "label": "DocumentationChunk",
        "id": chunk.id.0.to_string(),
        "text": chunk.text,
        "source_url": chunk.source_url,
        "scope": match chunk.scope {
            crate::data::Scope::General => "General",
            crate::data::Scope::UserDefined => "UserDefined",
            crate::data::Scope::ApplicationState => "ApplicationState",
            _ => "Other"
        },
        "embedding": chunk.embedding,
        "chunk_seq": chunk.chunk_seq,
        "created_at": chunk.created_at.to_rfc3339(),
        "tenant_id": tenant_id.0.to_string()
    });
    
    patch.nodes.push(chunk_node);
    
    // 5. Add relationship to entity if provided
    if let (Some(entity_type), Some(entity_id)) = (&linked_entity_type, &linked_entity_id) {
        let edge = serde_json::json!({
            "type": "HAS_DOCUMENTATION",
            "from": {
                "label": entity_type,
                "id": entity_id
            },
            "to": {
                "label": "DocumentationChunk",
                "id": chunk.id.0.to_string()
            },
            "properties": {}
        });
        
        patch.edges.push(edge);
        
        info!(
            tenant_id = ?tenant_id,
            trace_id = ?trace_ctx.trace_id,
            entity_type = entity_type,
            entity_id = entity_id,
            "Linking document chunk to entity"
        );
    }
    
    // 6. Persist the patch to the database
    state_store.upsert_graph_data(tenant_id, trace_ctx, patch).await
        .map_err(|e| CoreError::db_error_with_context(
            "Failed to persist document chunk",
            Some(*tenant_id),
            Some(trace_ctx),
            Some(e),
        ))?;
    
    info!(
        tenant_id = ?tenant_id,
        trace_id = ?trace_ctx.trace_id,
        "Successfully processed document chunk"
    );
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use uuid::Uuid;
    use chrono::Utc;
    use tokio::sync::mpsc;
    use serde_json::json;
    
    use crate::{
        data::{
            TenantId, TraceContext, Scope, DataPacket, 
            DocumentationChunk, StateStoreError, CoreError,
            entities::{
                ApplicationStateSnapshot, FlowInstanceConfig, StepInstanceConfig,
                ComponentDefinition
            },
            identifiers::{ComponentDefinitionId, DocumentationChunkId, VersionId},
        },
        traits::{StateStore, EmbeddingGenerator, DslParser, ParsedDslDefinitions},
        test_utils::mocks::{MockStateStore, MockEmbeddingGenerator, MockDslParser},
        services::messages::IngestionMessage,
    };
    
    use super::*;
    
    // Create test data functions here
    
    fn create_test_docs(count: usize) -> Vec<DocumentationChunk> {
        (0..count).map(|i| DocumentationChunk {
            id: DocumentationChunkId(Uuid::new_v4()),
            text: format!("Test documentation chunk {}", i),
            source_url: Some(format!("http://example.com/docs/{}", i)),
            scope: Scope::General,
            embedding: Vec::new(),
            chunk_seq: Some(i as i64),
            created_at: Utc::now(),
        }).collect()
    }

    fn create_test_parsed_defs() -> ParsedDslDefinitions {
        let comp_def = ComponentDefinition {
            id: ComponentDefinitionId(Uuid::new_v4()),
            name: "TestComponent".to_string(),
            component_type_id: "test:component".to_string(),
            source: SourceType::StdLib,
            description: Some("Test component description".to_string()),
            config_schema_ref: None,
            state_schema_ref: None,
            scope: Scope::General,
            created_at: Utc::now(),
            version_id: VersionId(Uuid::new_v4()),
        };

        ParsedDslDefinitions {
            components: vec![comp_def],
            flows: Vec::new(),
            input_definitions: Vec::new(),
            output_definitions: Vec::new(),
            step_definitions: Vec::new(),
            trigger_definitions: Vec::new(),
            condition_definitions: Vec::new(),
            documentation_chunks: create_test_docs(2),
            code_examples: Vec::new(),
        }
    }
    
    // Test functions continue below...

    #[tokio::test]
    async fn test_generate_embeddings_for_docs() {
        let mock_embedding_generator = MockEmbeddingGenerator::new();
        
        // Setup mock to return a simple embedding - will be called twice for 2 docs
        mock_embedding_generator
            .expect_generate_embedding()
            .times(2)
            .returning(Ok(vec![0.1, 0.2, 0.3]));
        
        let parsed_defs = create_test_parsed_defs();
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        
        // Cast the mock to the trait object type
        let embedding_gen: Arc<dyn EmbeddingGenerator> = Arc::new(mock_embedding_generator);
        
        // Call the function under test
        let result = generate_embeddings_for_docs(
            &embedding_gen, 
            parsed_defs, 
            &tenant_id, 
            &trace_ctx
        ).await;
        
        // Verify results
        assert!(result.is_ok());
        let processed_defs = result.unwrap();
        
        // Check that embeddings were added
        for chunk in &processed_defs.documentation_chunks {
            assert_eq!(chunk.embedding, vec![0.1, 0.2, 0.3]);
        }
    }
    
    #[tokio::test]
    async fn test_generate_embeddings_error_handling() {
        let mock_embedding_generator = MockEmbeddingGenerator::new();
        
        // First call will return error, second call will return success
        mock_embedding_generator
            .expect_generate_embedding()
            .returning(Err(CoreError::EmbeddingError("Test embedding error".to_string())));
        
        mock_embedding_generator
            .expect_generate_embedding()
            .returning(Ok(vec![0.1, 0.2, 0.3]));
        
        let parsed_defs = create_test_parsed_defs();
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        
        // Cast the mock to the trait object type
        let embedding_gen: Arc<dyn EmbeddingGenerator> = Arc::new(mock_embedding_generator);
        
        // Call the function under test
        let result = generate_embeddings_for_docs(
            &embedding_gen, 
            parsed_defs, 
            &tenant_id, 
            &trace_ctx
        ).await;
        
        // Verify results - should still succeed despite error
        assert!(result.is_ok());
        let processed_defs = result.unwrap();
        
        // First doc should have empty embedding, second should have the mock values
        assert!(processed_defs.documentation_chunks[0].embedding.is_empty());
        assert_eq!(processed_defs.documentation_chunks[1].embedding, vec![0.1, 0.2, 0.3]);
    }
    
    #[tokio::test]
    async fn test_process_yaml_definitions_success() {
        // Create mocks
        let mock_dsl_parser = MockDslParser::new();
        let mock_embedding_generator = MockEmbeddingGenerator::new();
        let mock_state_store = MockStateStore::new();
        
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        let yaml_content = "component: test";
        let scope = Scope::UserDefined;
        
        // Setup mock behaviors
        mock_dsl_parser
            .expect_parse_and_validate_yaml()
            .returning(Ok(create_test_parsed_defs()));
            
        mock_embedding_generator
            .expect_generate_embedding()
            .returning(Ok(vec![0.1, 0.2, 0.3]));
            
        mock_embedding_generator
            .expect_generate_embedding()
            .returning(Ok(vec![0.1, 0.2, 0.3]));
            
        mock_state_store
            .expect_upsert_graph_data()
            .returning_for_upsert(Ok(()));
        
        // Cast mocks to trait object types
        let dsl_parser: Arc<dyn DslParser> = Arc::new(mock_dsl_parser);
        let embedding_gen: Arc<dyn EmbeddingGenerator> = Arc::new(mock_embedding_generator);
        let state_store: Arc<dyn StateStore> = Arc::new(mock_state_store);
        
        // Call the function under test
        let result = process_yaml_definitions(
            &dsl_parser,
            &embedding_gen,
            &state_store,
            yaml_content,
            &tenant_id,
            &trace_ctx,
            scope,
            None,
        ).await;
        
        // Verify results
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_process_yaml_definitions_parse_error() {
        // Create mocks
        let mock_dsl_parser = MockDslParser::new();
        let mock_embedding_generator = MockEmbeddingGenerator::new();
        let mock_state_store = MockStateStore::new();
        
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        let yaml_content = "invalid: yaml";
        let scope = Scope::UserDefined;
        
        // Setup mock behaviors - parser returns error
        mock_dsl_parser
            .expect_parse_and_validate_yaml()
            .returning(Err(CoreError::ValidationError("Invalid YAML".to_string())));
        
        // Cast mocks to trait object types
        let dsl_parser: Arc<dyn DslParser> = Arc::new(mock_dsl_parser);
        let embedding_gen: Arc<dyn EmbeddingGenerator> = Arc::new(mock_embedding_generator);
        let state_store: Arc<dyn StateStore> = Arc::new(mock_state_store);
        
        // Call the function under test
        let result = process_yaml_definitions(
            &dsl_parser,
            &embedding_gen,
            &state_store,
            yaml_content,
            &tenant_id,
            &trace_ctx,
            scope,
            None,
        ).await;
        
        // Verify error result
        assert!(result.is_err());
        
        // Check error context
        match result.unwrap_err() {
            CoreError::IngestionErrorWithContext { tenant_id: tid, trace_id, .. } => {
                assert_eq!(tid, Some(tenant_id));
                assert_eq!(trace_id, Some(trace_ctx.trace_id.to_string()));
            },
            err => panic!("Unexpected error type: {:?}", err),
        }
    }
    
    #[tokio::test]
    async fn test_process_yaml_definitions_upsert_error() {
        // Create mocks
        let mock_dsl_parser = MockDslParser::new();
        let mock_embedding_generator = MockEmbeddingGenerator::new();
        let mock_state_store = MockStateStore::new();
        
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        let yaml_content = "component: test";
        let scope = Scope::UserDefined;
        
        // Setup mock behaviors
        mock_dsl_parser
            .expect_parse_and_validate_yaml()
            .returning(Ok(create_test_parsed_defs()));
            
        mock_embedding_generator
            .expect_generate_embedding()
            .returning(Ok(vec![0.1, 0.2, 0.3]));
        
        mock_embedding_generator
            .expect_generate_embedding()
            .returning(Ok(vec![0.1, 0.2, 0.3]));
        
        mock_state_store
            .expect_upsert_graph_data()
            .returning_for_upsert(Err(StateStoreError::ConnectionError("DB connection failed".to_string())));
        
        // Cast mocks to trait object types
        let dsl_parser: Arc<dyn DslParser> = Arc::new(mock_dsl_parser);
        let embedding_gen: Arc<dyn EmbeddingGenerator> = Arc::new(mock_embedding_generator);
        let state_store: Arc<dyn StateStore> = Arc::new(mock_state_store);
        
        // Call the function under test
        let result = process_yaml_definitions(
            &dsl_parser,
            &embedding_gen,
            &state_store,
            yaml_content,
            &tenant_id,
            &trace_ctx,
            scope,
            None,
        ).await;
        
        // Verify error result
        assert!(result.is_err());
        
        // Check error details
        match result.unwrap_err() {
            CoreError::DatabaseErrorWithContext { message, tenant_id: tid, trace_id, .. } => {
                assert_eq!(tid, Some(tenant_id));
                assert_eq!(trace_id, Some(trace_ctx.trace_id.to_string()));
                assert!(message.contains("Failed to upsert graph data"));
            },
            err => panic!("Unexpected error type: {:?}", err),
        }
    }
    
    #[tokio::test]
    async fn test_process_application_state_success() {
        // Create mocks
        let mock_state_store = MockStateStore::new();
        
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        
        // Create properly formatted ApplicationStateSnapshot as JSON
        let flow_def_version_id = Uuid::new_v4();
        let component_def_version_id = Uuid::new_v4();
        let step_def_id = Uuid::new_v4();
        
        // Create a valid ApplicationStateSnapshot structure directly
        let snapshot = ApplicationStateSnapshot {
            snapshot_id: "test-snapshot-1".to_string(),
            timestamp: Utc::now(),
            source: "test-deployment".to_string(),
            tenant_id: tenant_id.clone(),
            scope: Scope::ApplicationState,
            flow_instances: Some(vec![
                FlowInstanceConfig {
                    instance_id: "instance-1".to_string(),
                    config_source: Some("test-config".to_string()),
                    snapshot_id: "test-snapshot-1".to_string(),
                    flow_def_version_id: VersionId(flow_def_version_id),
                    tenant_id: tenant_id.clone(),
                    step_instances: Some(vec![
                        StepInstanceConfig {
                            instance_step_id: "step-1".to_string(),
                            flow_instance_id: "instance-1".to_string(),
                            step_def_id,
                            component_def_version_id: VersionId(component_def_version_id),
                            config: DataPacket::Json(json!({"key": "value"})),
                            tenant_id: tenant_id.clone(),
                        }
                    ]),
                }
            ]),
        };
        
        // Convert to JSON DataPacket
        let snapshot_data = DataPacket::Json(serde_json::to_value(snapshot).unwrap());
        let source_info = "test-deployment".to_string();
        
        // Setup mock behaviors
        mock_state_store
            .expect_upsert_graph_data()
            .returning_for_upsert(Ok(()));
        
        // Cast mock to trait object type
        let state_store: Arc<dyn StateStore> = Arc::new(mock_state_store);
        
        // Call the function under test
        let result = process_application_state(
            &state_store,
            snapshot_data,
            &tenant_id,
            &trace_ctx,
            source_info,
        ).await;
        
        // Verify results
        assert!(result.is_ok(), "Expected Ok result but got: {:?}", result);
    }
    
    #[tokio::test]
    async fn test_ingestion_service_run() {
        // Create mocks
        let mock_dsl_parser = MockDslParser::new();
        let mock_embedding_generator = MockEmbeddingGenerator::new();
        let mock_state_store = MockStateStore::new();
        
        // Create channel for sending messages to the service
        let (tx, rx) = mpsc::channel(10);
        
        // Create test message
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        let yaml_content = "component: test".to_string();
        
        // Setup mock behaviors
        mock_dsl_parser
            .expect_parse_and_validate_yaml()
            .returning(Ok(create_test_parsed_defs()));
            
        mock_embedding_generator
            .expect_generate_embedding()
            .returning(Ok(vec![0.1, 0.2, 0.3]));
            
        mock_embedding_generator
            .expect_generate_embedding()
            .returning(Ok(vec![0.1, 0.2, 0.3]));
            
        mock_state_store
            .expect_upsert_graph_data()
            .returning_for_upsert(Ok(()));
        
        // Create the service
        let mut service = IngestionService::new(
            Arc::new(mock_state_store),
            Arc::new(mock_embedding_generator),
            Arc::new(mock_dsl_parser),
            rx
        );
        
        // Send test message
        tx.send(IngestionMessage::YamlDefinitions {
            tenant_id,
            trace_ctx,
            yaml_content,
            scope: Scope::UserDefined,
            source_info: None,
        }).await.expect("Failed to send test message");
        
        // Drop tx to signal that no more messages will be sent
        drop(tx);
        
        // Run the service
        let result = service.run().await;
        
        // Service should complete successfully after processing all messages
        assert!(result.is_ok());
    }
} 
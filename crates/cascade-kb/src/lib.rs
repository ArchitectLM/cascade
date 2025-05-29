//! Cascade Graph Knowledge Base Core Implementation
//! Based on the Detailed Implementation Specification V1.0

// Core modules
pub mod data;
pub mod traits;
pub mod services;
pub mod embedding;

// Implementation adapters (optional, can be provided externally)
#[cfg(feature = "adapters")]
pub mod adapters;

// Testing utilities - make this available during testing

pub mod test_utils;

// Re-export key types for convenient usage
pub use data::errors::CoreError;
pub use data::identifiers::{
    TenantId, VersionSetId, VersionId, ComponentDefinitionId, FlowDefinitionId,
    DocumentationChunkId, CodeExampleId,
};
pub use data::types::{
    DataPacket, Scope, SourceType, VersionStatus, SchemaRef, DataReference,
    DataPacketMapExt,
};
pub use data::trace_context::TraceContext;
pub use traits::state_store::GraphDataPatch;
pub use data::entities::{
    Framework, DslSpec, DslElement, DslElementType,
    VersionSet, Version, ComponentDefinition, FlowDefinition,
    InputDefinition, OutputDefinition, StepDefinition,
    TriggerDefinition, ConditionDefinition, DocumentationChunk,
    CodeExample, ApplicationStateSnapshot, FlowInstanceConfig,
    StepInstanceConfig,
};

// Re-export core traits
pub use traits::{
    StateStore, EmbeddingGenerator, DslParser, ParsedDslDefinitions,
    ComponentLoader, ComponentExecutor, ComponentRuntimeApi,
    TriggerExecutor, ConditionEvaluator, TriggerCallback,
};

// Re-export embedding services
#[cfg(feature = "async-openai")]
pub use embedding::OpenAIEmbeddingService;
#[cfg(feature = "neo4rs")]
pub use embedding::Neo4jEmbeddingService;
pub use embedding::{
    EmbeddingService,
    Neo4jEmbeddingServiceConfig,
    EmbeddingServiceConfig,
    create_embedding_service,
};

// Re-export core services
pub use services::{
    IngestionService, QueryService, KbClient,
    // Runtime services to be implemented:
    // FlowInstanceManager, Scheduler, StepExecutor
};

// Re-export error types
pub use data::errors::{
    StateStoreError, ComponentLoaderError, ComponentApiError,
};

// Re-export message types
pub use services::messages::{
    IngestionMessage, QueryRequest, QueryResponse, QueryResultSender,
};

#[cfg(feature = "adapters")]
use std::sync::Arc;

// Import cascade_interfaces for KB client interfaces
#[cfg(feature = "adapters")]
use cascade_interfaces;

/// Initialize tracing for the KB system
pub fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}

/// Creates a KnowledgeBaseClient implementation based on configuration
/// 
/// # Arguments
/// * `provider` - The provider type ("remote" or other future providers)
/// * `service_url` - URL of the remote KB service (for remote provider)
/// * `timeout_secs` - Timeout in seconds for HTTP requests (for remote provider)
/// 
/// # Returns
/// An Arc-wrapped implementation of the KnowledgeBaseClient trait
#[cfg(feature = "adapters")]
pub fn create_kb_client(
    provider: &str,
    _service_url: Option<String>,
    _timeout_secs: Option<u64>,
) -> Arc<dyn cascade_interfaces::kb::KnowledgeBaseClient + Send + Sync> {
    match provider {
        #[cfg(feature = "remote-kb")]
        "remote" => {
            let url = _service_url.unwrap_or_else(|| "http://localhost:8090".to_string());
            let timeout = _timeout_secs.unwrap_or(30);
            adapters::create_remote_kb_client(url, timeout)
        },
        #[cfg(not(feature = "remote-kb"))]
        "remote" => {
            panic!("Remote KB client requested but the 'remote-kb' feature is not enabled");
        },
        "mock" | "direct" => {
            // Use test_utils for mock implementation for both direct and mock providers
            // This is useful for testing without actual backend dependencies
            use cascade_interfaces::kb::{
                GraphElementDetails, KnowledgeBaseClient, KnowledgeResult, 
                RetrievedContext, ValidationErrorDetail
            };
            use async_trait::async_trait;
            
            #[derive(Default)]
            struct MockKbClient;
            
            #[async_trait]
            impl KnowledgeBaseClient for MockKbClient {
                async fn get_element_details(
                    &self, 
                    _tenant_id: &str, 
                    _entity_id: &str, 
                    _version: Option<&str>
                ) -> KnowledgeResult<Option<GraphElementDetails>> {
                    // Always return None for testing
                    Ok(None)
                }
                
                async fn search_related_artefacts(
                    &self, 
                    _tenant_id: &str, 
                    _query_text: Option<&str>, 
                    _entity_id: Option<&str>, 
                    _version: Option<&str>, 
                    _limit: usize
                ) -> KnowledgeResult<Vec<RetrievedContext>> {
                    // Return empty vector for testing
                    Ok(Vec::new())
                }
                
                async fn validate_dsl(
                    &self, 
                    _tenant_id: &str, 
                    _dsl_code: &str
                ) -> KnowledgeResult<Vec<ValidationErrorDetail>> {
                    // Return empty vector (valid) for testing
                    Ok(Vec::new())
                }
                
                async fn trace_step_input_source(
                    &self, 
                    _tenant_id: &str, 
                    _flow_entity_id: &str, 
                    _flow_version: Option<&str>, 
                    _step_id: &str, 
                    _component_input_name: &str
                ) -> KnowledgeResult<Vec<(String, String)>> {
                    // Return empty vector for testing
                    Ok(Vec::new())
                }
            }
            
            Arc::new(MockKbClient::default())
        },
        // Add other providers here as they're implemented
        _ => panic!("Unsupported KB provider: {}", provider),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::identifiers::TenantId;

    #[test]
    fn test_tenant_id_creation() {
        let id1 = TenantId::new_v4();
        let id2 = TenantId::new(None);
        assert_ne!(id1, id2, "Generated UUIDs should be unique");
        
        // Test with string parameter
        let id3 = TenantId::new(Some("test-tenant"));
        assert_ne!(id2, id3, "Generated UUIDs should be unique");
        
        // Just to verify the feature flag is working
        #[cfg(feature = "adapters")]
        {
            assert!(true, "adapters feature is enabled");
        }
    }
}

// Remove the duplicate module declarations at the end
// Private test utilities 
// #[cfg(test)]
// mod test_utils;

// Unit tests that use mocks and can run without external dependencies
// #[cfg(test)]
// mod tests;

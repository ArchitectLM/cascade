use crate::data::{
    errors::CoreError,
    identifiers::TenantId,
    trace_context::TraceContext,
    types::Scope,
};
use crate::services::messages::{IngestionMessage, QueryRequest, QueryResponse, QueryResultSender};
use tokio::sync::{mpsc, oneshot};
use std::collections::HashMap;
use crate::data::DataPacket;

/// Client interface for interacting with the Knowledge Base services.
/// Provides a clean API for sending ingestion messages and making queries.
#[derive(Clone)]
pub struct KbClient {
    ingestion_tx: mpsc::Sender<IngestionMessage>,
    query_tx: mpsc::Sender<(QueryRequest, QueryResultSender)>,
}

impl KbClient {
    /// Creates a new KbClient with the provided channel senders.
    pub fn new(
        ingestion_tx: mpsc::Sender<IngestionMessage>,
        query_tx: mpsc::Sender<(QueryRequest, QueryResultSender)>,
    ) -> Self {
        KbClient { ingestion_tx, query_tx }
    }

    /// Sends an ingestion message to the IngestionService.
    pub async fn ingest(&self, message: IngestionMessage) -> Result<(), CoreError> {
        self.ingestion_tx
            .send(message)
            .await
            .map_err(|_| CoreError::Internal("Ingestion channel closed".to_string()))
    }

    /// Sends a query request and awaits the response.
    pub async fn query(&self, request: QueryRequest) -> Result<Vec<HashMap<String, DataPacket>>, CoreError> {
        let (response_tx, response_rx) = oneshot::channel();
        
        self.query_tx
            .send((request, QueryResultSender { sender: response_tx }))
            .await
            .map_err(|_| CoreError::Internal("Query channel closed".to_string()))?;
        
        let response = response_rx
            .await
            .map_err(|_| CoreError::Internal("Query response channel closed by service".to_string()))?;
        
        match response {
            QueryResponse::Success(results) => Ok(results),
            QueryResponse::Error(err) => Err(err),
        }
    }

    /// Helper method to ingest YAML definitions.
    pub async fn ingest_yaml(
        &self,
        tenant_id: TenantId,
        yaml_content: String,
        scope: Scope,
        source_info: Option<String>,
    ) -> Result<(), CoreError> {
        let message = IngestionMessage::YamlDefinitions {
            tenant_id,
            trace_ctx: TraceContext::new_root(),
            yaml_content,
            scope,
            source_info,
        };
        self.ingest(message).await
    }

    /// Client method for looking up an entity by ID
    pub async fn lookup_by_id(
        &self,
        tenant_id: TenantId,
        trace_ctx: TraceContext,
        entity_type: String,
        id: String,
        version: Option<String>,
    ) -> Result<QueryResponse, CoreError> {
        let request = QueryRequest::LookupById {
            tenant_id,
            trace_ctx,
            entity_type,
            id,
            version,
        };
        
        let results = self.query(request).await?;
        Ok(QueryResponse::Success(results))
    }

    /// Client method for semantic search
    pub async fn semantic_search(
        &self,
        tenant_id: TenantId,
        trace_ctx: TraceContext,
        query_text: String,
        k: usize,
        scope_filter: Option<Scope>,
        entity_type_filter: Option<String>,
    ) -> Result<QueryResponse, CoreError> {
        let request = QueryRequest::SemanticSearch {
            tenant_id,
            trace_ctx,
            query_text,
            k,
            scope_filter,
            entity_type_filter,
        };
        
        let results = self.query(request).await?;
        Ok(QueryResponse::Success(results))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_ingest_success() {
        // Create channels with sufficient buffer to avoid blocking
        let (ingestion_tx, mut ingestion_rx) = mpsc::channel(10);
        let (query_tx, _query_rx) = mpsc::channel(10);
        
        // Create client
        let client = KbClient::new(ingestion_tx, query_tx);
        
        // Test data
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        let test_message = IngestionMessage::YamlDefinitions {
            tenant_id,
            trace_ctx,
            yaml_content: "test: content".to_string(),
            scope: Scope::UserDefined,
            source_info: Some("test.yaml".to_string()),
        };
        
        // Call ingest method
        let result = client.ingest(test_message.clone()).await;
        
        // Verify success
        assert!(result.is_ok());
        
        // Verify the message was sent to the channel
        let received_message = ingestion_rx.recv().await.unwrap();
        
        // Match on the message type to compare fields
        match (received_message, test_message) {
            (
                IngestionMessage::YamlDefinitions { 
                    tenant_id: recv_tenant_id, 
                    yaml_content: recv_content, 
                    scope: recv_scope, 
                    source_info: recv_source, 
                    .. 
                },
                IngestionMessage::YamlDefinitions { 
                    tenant_id: orig_tenant_id, 
                    yaml_content: orig_content, 
                    scope: orig_scope, 
                    source_info: orig_source, 
                    .. 
                }
            ) => {
                assert_eq!(recv_tenant_id, orig_tenant_id);
                assert_eq!(recv_content, orig_content);
                assert_eq!(recv_scope, orig_scope);
                assert_eq!(recv_source, orig_source);
            },
            _ => panic!("Unexpected message type received"),
        }
    }
    
    #[tokio::test]
    async fn test_client_ingest_failure() {
        let (tx, _rx) = mpsc::channel(1);
        drop(_rx); // Close the channel
        
        let client = KbClient::new(tx, mpsc::channel(1).0);
        
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        
        let result = client.ingest(IngestionMessage::YamlDefinitions { 
            tenant_id,
            trace_ctx,
            yaml_content: "test".to_string(),
            scope: Scope::General,
            source_info: None
        }).await;
        
        assert!(result.is_err());
        match result {
            Err(CoreError::Internal(msg)) => {
                assert!(msg.contains("Ingestion channel closed"));
            },
            _ => panic!("Expected InternalError"),
        }
    }
    
    #[tokio::test]
    async fn test_client_success() {
        // Similar to before, but adjust the result handling
        
        // Create channels
        let (ingestion_tx, _ingestion_rx) = mpsc::channel(10);
        let (query_tx, mut query_rx) = mpsc::channel(10);
        
        // Create client
        let client = KbClient::new(ingestion_tx, query_tx);
        
        // Set up the mock handler
        tokio::spawn(async move {
            if let Some((_, sender)) = query_rx.recv().await {
                // Simulate service sending back results
                let mut results = HashMap::new();
                results.insert("name".to_string(), DataPacket::String("Test Entity".to_string()));
                let response = QueryResponse::Success(vec![results]);
                let _ = sender.sender.send(response);
            }
        });
        
        // Test lookup function
        let tenant_id = TenantId::new_v4();
        let result = client.lookup_by_id(
            tenant_id,
            TraceContext::new_root(),
            "TestEntity".to_string(),
            "test-id".to_string(),
            None,
        ).await;
        
        // Verify response content
        match result {
            Ok(QueryResponse::Success(results)) => {
                assert_eq!(results.len(), 1);
                assert!(results[0].contains_key("name"));
                match &results[0]["name"] {
                    DataPacket::String(s) => assert_eq!(s, "Test Entity"),
                    _ => panic!("Expected String DataPacket"),
                }
            },
            _ => panic!("Expected Success response"),
        }
    }
    
    #[tokio::test]
    async fn test_client_query_channel_closed() {
        let (tx, _rx) = mpsc::channel(1);
        let client = KbClient::new(mpsc::channel(1).0, tx);
        drop(_rx); // Close the channel
        
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        
        let result = client.query(QueryRequest::LookupById { 
            tenant_id,
            trace_ctx,
            entity_type: "Component".to_string(),
            id: "test".to_string(),
            version: None
        }).await;
        
        assert!(result.is_err());
        match result {
            Err(CoreError::Internal(msg)) => {
                assert!(msg.contains("Query channel closed"));
            },
            _ => panic!("Expected InternalError"),
        }
    }
    
    #[tokio::test]
    async fn test_client_query_response_channel_closed() {
        let (tx, mut rx) = mpsc::channel(1);
        let client = KbClient::new(mpsc::channel(1).0, tx);
        
        // Set up the mock handler to drop the sender
        tokio::spawn(async move {
            if let Some((_, _)) = rx.recv().await {
                // Intentionally drop the sender without sending a response
            }
        });
        
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        
        let result = client.query(QueryRequest::LookupById { 
            tenant_id,
            trace_ctx,
            entity_type: "Component".to_string(),
            id: "test".to_string(),
            version: None
        }).await;
        
        assert!(result.is_err());
        match result {
            Err(CoreError::Internal(msg)) => {
                assert!(msg.contains("Query response channel closed"));
            },
            _ => panic!("Expected InternalError"),
        }
    }
    
    #[tokio::test]
    async fn test_client_ingest_yaml() {
        // Create channels
        let (ingestion_tx, mut ingestion_rx) = mpsc::channel(10);
        let (query_tx, _query_rx) = mpsc::channel(10);
        
        // Create client
        let client = KbClient::new(ingestion_tx, query_tx);
        
        // Test data
        let tenant_id = TenantId::new_v4();
        let yaml_content = "test: yaml content";
        let scope = Scope::UserDefined;
        let source_info = Some("test.yaml".to_string());
        
        // Call ingest_yaml helper method
        let result = client.ingest_yaml(
            tenant_id.clone(),
            yaml_content.to_string(),
            scope.clone(),
            source_info.clone()
        ).await;
        
        // Verify success
        assert!(result.is_ok());
        
        // Verify the message was sent to the channel with correct content
        let received_message = ingestion_rx.recv().await.unwrap();
        match received_message {
            IngestionMessage::YamlDefinitions { 
                tenant_id: recv_tenant_id, 
                yaml_content: recv_content, 
                scope: recv_scope, 
                source_info: recv_source, 
                .. 
            } => {
                assert_eq!(recv_tenant_id, tenant_id);
                assert_eq!(recv_content, yaml_content);
                assert_eq!(recv_scope, scope);
                assert_eq!(recv_source, source_info);
            },
            _ => panic!("Unexpected message type received"),
        }
    }
    
    #[tokio::test]
    async fn test_client_lookup_by_id() {
        // Create channels
        let (ingestion_tx, _ingestion_rx) = mpsc::channel(10);
        let (query_tx, mut query_rx) = mpsc::channel(10);
        
        // Create client
        let client = KbClient::new(ingestion_tx, query_tx);
        
        // Test data
        let tenant_id1 = TenantId::new_v4();
        let tenant_id2 = tenant_id1.clone();
        let entity_type = "ComponentDefinition";
        let id = "test-component";
        let version_str = "1.0";
        let version = Some(version_str.to_string());
        
        // Spawn a task to receive the query request and send a response
        tokio::spawn(async move {
            // Receive the query request and verify it's correctly formed
            let (req, sender) = query_rx.recv().await.unwrap();
            
            match req {
                QueryRequest::LookupById { 
                    tenant_id: recv_tenant_id, 
                    entity_type: recv_entity_type, 
                    id: recv_id, 
                    version: recv_version, 
                    .. 
                } => {
                    assert_eq!(recv_tenant_id, tenant_id1);
                    assert_eq!(recv_entity_type, entity_type);
                    assert_eq!(recv_id, id);
                    assert_eq!(recv_version, Some(version_str.to_string()));
                },
                _ => panic!("Unexpected request type received"),
            }
            
            // Send a mock response
            let mut result_map = HashMap::new();
            result_map.insert(
                "name".to_string(), 
                DataPacket::String("Test Component".to_string())
            );
            let results = vec![result_map];
            let _ = sender.sender.send(QueryResponse::Success(results));
        });
        
        // Call lookup_by_id helper method
        let result = client.lookup_by_id(
            tenant_id2,
            TraceContext::new_root(),
            entity_type.to_string(),
            id.to_string(),
            version
        ).await;
        
        // Verify success
        assert!(result.is_ok());
        
        // Verify response content
        match result.unwrap() {
            QueryResponse::Success(data) => {
                assert_eq!(data.len(), 1);
                assert!(data[0].contains_key("name"));
                match &data[0]["name"] {
                    DataPacket::String(s) => assert_eq!(s, "Test Component"),
                    _ => panic!("Expected String DataPacket"),
                }
            },
            QueryResponse::Error(e) => panic!("Expected Success, got Error: {:?}", e),
        }
    }
} 
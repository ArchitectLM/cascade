#[cfg(test)]
mod tests {
    use crate::services::query::service::{lookup_by_id, semantic_search};
    use std::sync::Arc;
    use std::collections::HashMap;
    use crate::data::{TenantId, TraceContext, StateStoreError, DataPacket};
    use crate::traits::{StateStore, EmbeddingGenerator};
    use crate::test_utils::mocks::{MockStateStore, MockEmbeddingGenerator};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_lookup_by_id_success() {
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        
        // Set up mock state store
        let mock_state_store = MockStateStore::new();
        
        // Set up expectations for execute_query
        let mut expected_result = HashMap::new();
        expected_result.insert("name".to_string(), DataPacket::String("Test Entity".to_string()));
        
        mock_state_store
            .expect_execute_query()
            .returning(Ok(vec![expected_result.clone()]));
        
        let state_store: Arc<dyn StateStore> = Arc::new(mock_state_store);
        
        // Test the lookup_by_id function directly
        let result = lookup_by_id(
            &state_store,
            tenant_id.clone(),
            &trace_ctx,
            "Component",
            "test-id",
            None
        ).await;
        
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["name"], DataPacket::String("Test Entity".to_string()));
    }

    #[tokio::test]
    async fn test_semantic_search_success() {
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        let query_text = "test query";
        let k = 5;
        
        // Set up mock embedding generator
        let mock_embedding_gen = MockEmbeddingGenerator::new();
        mock_embedding_gen
            .expect_generate_embedding()
            .returning(Ok(vec![0.1, 0.2, 0.3]));
        
        // Set up mock state store
        let mock_state_store = MockStateStore::new();
        
        // Mock vector search
        let doc_id = Uuid::new_v4();
        mock_state_store
            .expect_vector_search()
            .returning_for_vector_search(Ok(vec![(doc_id, 0.95)]));
            
        // Mock execute_query for document fetching
        let mut result = HashMap::new();
        result.insert("docId".to_string(), DataPacket::String(doc_id.to_string()));
        result.insert("docText".to_string(), DataPacket::String("Sample document text".to_string()));
        mock_state_store
            .expect_execute_query()
            .returning(Ok(vec![result]));
        
        // Cast to trait objects
        let state_store: Arc<dyn StateStore> = Arc::new(mock_state_store);
        let embedding_gen: Arc<dyn EmbeddingGenerator> = Arc::new(mock_embedding_gen);
        
        // Call the function
        let result = semantic_search(
            &state_store,
            &embedding_gen,
            tenant_id.clone(), 
            &trace_ctx,
            query_text,
            k,
            None,
            None
        ).await;
        
        // Verify results
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.len(), 1);
        match &data[0]["docText"] {
            DataPacket::String(text) => assert_eq!(text, "Sample document text"),
            _ => panic!("Expected String DataPacket"),
        }
    }

    #[tokio::test]
    async fn test_semantic_search_vector_search_error() {
        let mock_state_store = MockStateStore::new();
        let mock_embedding_gen = MockEmbeddingGenerator::new();
        let tenant_id = TenantId::new_v4();
        let trace_ctx = TraceContext::new_root();
        let query_text = "How to use HTTP component";
        let k = 5;
        let scope_filter = None;
        let entity_type_filter = None;
        
        // Setup embedding generator mock
        mock_embedding_gen
            .expect_generate_embedding()
            .returning(Ok(vec![0.1, 0.2, 0.3]));
        
        // Setup vector search to return an error
        mock_state_store
            .expect_vector_search()
            .returning_for_vector_search(Err(StateStoreError::QueryError("Vector search failed".to_string())));
        
        // Setup execute_query to return an empty result for the fallback behavior
        mock_state_store
            .expect_execute_query()
            .returning(Ok(vec![]));
        
        // Cast mocks to trait object types
        let state_store: Arc<dyn StateStore> = Arc::new(mock_state_store);
        let embedding_gen: Arc<dyn EmbeddingGenerator> = Arc::new(mock_embedding_gen);
        
        // Call the function
        let result = semantic_search(
            &state_store,
            &embedding_gen,
            tenant_id,
            &trace_ctx,
            query_text,
            k,
            scope_filter,
            entity_type_filter,
        ).await;
        
        // Now we expect success with empty results since we implemented fallback
        assert!(result.is_ok());
        let data = result.unwrap();
        assert!(data.is_empty(), "Expected empty results from fallback mechanism");
    }
}
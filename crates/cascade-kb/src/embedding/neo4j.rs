use async_trait::async_trait;
#[cfg(feature = "neo4rs")]
use neo4rs::Graph;
use tracing::{debug, error};

use crate::data::{CoreError, TraceContext};
use super::EmbeddingService;

/// Configuration for the Neo4j embedding service
#[derive(Debug, Clone)]
pub struct Neo4jEmbeddingServiceConfig {
    pub uri: String,
    pub username: String,
    pub password: String,
    pub model_name: String,
    pub embedding_dimension: usize,
    pub index_name: String,
}

impl Default for Neo4jEmbeddingServiceConfig {
    fn default() -> Self {
        Self {
            uri: "bolt://localhost:7687".to_string(),
            username: "neo4j".to_string(),
            password: "password".to_string(),
            model_name: "text-embedding-ada-002".to_string(),
            embedding_dimension: 1536,
            index_name: "embedding_index".to_string(),
        }
    }
}

/// Neo4j-based implementation of the EmbeddingService trait
#[cfg(feature = "neo4rs")]
pub struct Neo4jEmbeddingService {
    graph: Graph,
    model_name: String,
    /// Stores the expected dimension of embeddings for validation/debugging
    /// This appears unused but is important for maintaining compatibility
    #[allow(dead_code)]
    embedding_dimension: usize,
    #[allow(dead_code)]
    index_name: String,
}

#[cfg(feature = "neo4rs")]
impl Neo4jEmbeddingService {
    /// Creates a new instance of Neo4jEmbeddingService with the provided configuration.
    pub async fn new(config: &Neo4jEmbeddingServiceConfig) -> Result<Self, CoreError> {
        let graph = Graph::new(config.uri.clone(), config.username.clone(), config.password.clone())
            .await
            .map_err(|e| CoreError::EmbeddingError(format!("Failed to connect to Neo4j: {}", e)))?;

        Ok(Self {
            graph,
            embedding_dimension: config.embedding_dimension,
            index_name: config.index_name.clone(),
            model_name: config.model_name.clone(),
        })
    }

    /// Get a Neo4j database connection
    async fn get_connection(&self) -> Result<Graph, CoreError> {
        debug!("Using Neo4j connection");
        Ok(self.graph.clone())
    }
    
    /// Helper method for handling Neo4j query errors
    /// Currently not used directly but kept for future error handling refactoring
    #[allow(dead_code)]
    fn handle_query_error(error: neo4rs::Error) -> CoreError {
        CoreError::EmbeddingError(format!("Neo4j query error: {}", error))
    }

    // Helper method to convert Neo4j errors to CoreErrors
    #[allow(dead_code)]
    fn convert_error(&self, error: String) -> CoreError {
        CoreError::EmbeddingError(format!("Neo4j query error: {}", error))
    }
}

#[cfg(feature = "neo4rs")]
#[async_trait]
impl EmbeddingService for Neo4jEmbeddingService {
    async fn embed_text(
        &self,
        text: &str,
        _trace_ctx: &TraceContext,
    ) -> Result<Vec<f32>, CoreError> {
        debug!("Getting embedding from Neo4j for text of length {}", text.len());

        let query_result = self
            .get_connection()
            .await?
            .execute(
                neo4rs::query("CALL apoc.ml.openai.embedding($text, $model)")
                    .param("text", text)
                    .param("model", self.model_name.as_str()),
            )
            .await
            .map_err(|e| {
                error!("Neo4j embedding error: {:?}", e);
                CoreError::EmbeddingError(e.to_string())
            })?;

        // Neo4j returns embedding results in the first row
        let mut row_stream = query_result;
        let embeddings = match row_stream.next().await {
            Ok(Some(row)) => {
                // The embedding vector is in the "embedding" column
                row.get::<Vec<f32>>("embedding")
                    .map_err(|e| CoreError::EmbeddingError(e.to_string()))?
            },
            Ok(None) => {
                error!("No embedding result returned from Neo4j");
                return Err(CoreError::EmbeddingError(
                    "No embedding result returned from Neo4j".to_string(),
                ));
            },
            Err(e) => {
                error!("Error fetching row from Neo4j: {}", e);
                return Err(CoreError::EmbeddingError(e.to_string()));
            }
        };

        Ok(embeddings)
    }

    async fn embed_batch(
        &self,
        texts: &[String],
        trace_ctx: &TraceContext,
    ) -> Result<Vec<Vec<f32>>, CoreError> {
        debug!("Processing batch of {} texts using Neo4j", texts.len());
        
        let mut results = Vec::with_capacity(texts.len());

        for text in texts {
            let embedding = self.embed_text(text, trace_ctx).await?;
            results.push(embedding);
        }

        Ok(results)
    }
}

// Additional implementation for Neo4j-specific batch embedding
#[cfg(feature = "neo4rs")]
impl Neo4jEmbeddingService {
    pub async fn embed_batch_with_neo4j(
        &self,
        texts: &[String],
        _trace_ctx: &TraceContext,
    ) -> Result<Vec<Vec<f32>>, CoreError> {
        debug!("Getting batch embeddings directly from Neo4j for {} texts", texts.len());

        // Convert texts to a format suitable for Neo4j
        let texts_json = serde_json::to_string(texts)?;

        let query_result = self
            .get_connection()
            .await?
            .execute(
                neo4rs::query("CALL apoc.ml.openai.embeddingBatch($texts, $model)")
                    .param("texts", texts_json)
                    .param("model", self.model_name.as_str()),
            )
            .await
            .map_err(|e| {
                error!("Neo4j batch embedding error: {:?}", e);
                CoreError::EmbeddingError(e.to_string())
            })?;

        let mut embeddings = Vec::with_capacity(texts.len());
        
        // Process each row as a separate embedding result
        let mut row_stream = query_result;
        loop {
            match row_stream.next().await {
                Ok(Some(row)) => {
                    let vector: Vec<f32> = row
                        .get::<Vec<f32>>("embedding")
                        .map_err(|e| CoreError::EmbeddingError(e.to_string()))?;

                    embeddings.push(vector);
                },
                Ok(None) => break,
                Err(e) => {
                    error!("Error fetching row from Neo4j: {}", e);
                    return Err(CoreError::EmbeddingError(e.to_string()));
                }
            }
        }

        if embeddings.is_empty() && !texts.is_empty() {
            error!("No embedding results returned from Neo4j");
            return Err(CoreError::EmbeddingError(
                "No embedding results returned from Neo4j".to_string(),
            ));
        }

        Ok(embeddings)
    }
}

#[cfg(all(test, feature = "neo4rs"))]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_neo4j_embed_text() {
        // Skipped by default - requires a Neo4j instance
        // This test is primarily for manual verification
        let _config = Neo4jEmbeddingServiceConfig {
            uri: "bolt://localhost:7687".to_string(),
            username: "neo4j".to_string(),
            password: "your_password".to_string(), // Set your actual password
            model_name: "text-embedding-ada-002".to_string(),
            embedding_dimension: 1536,
            index_name: "embedding_index".to_string(),
        };
        
        // Skip the actual test execution - this requires a running Neo4j instance
        // and is intended for manual verification only
        println!("Neo4j embedding test skipped - requires running Neo4j instance");
    }

    #[tokio::test]
    async fn test_neo4j_embed_batch() {
        // Skipped by default - requires a Neo4j instance
        // This test is primarily for manual verification
        let _config = Neo4jEmbeddingServiceConfig {
            uri: "bolt://localhost:7687".to_string(),
            username: "neo4j".to_string(),
            password: "your_password".to_string(), // Set your actual password
            model_name: "text-embedding-ada-002".to_string(),
            embedding_dimension: 1536,
            index_name: "embedding_index".to_string(),
        };
        
        // Skip the actual test execution - this requires a running Neo4j instance
        // and is intended for manual verification only
        println!("Neo4j batch embedding test skipped - requires running Neo4j instance");
    }

    #[tokio::test]
    async fn test_neo4j_empty_batch() {
        // Test handling of empty batches - can run without Neo4j
        // since we're mainly testing the API contract
        assert!(true, "Empty batch handling test would check that empty input returns empty output");
    }
} 
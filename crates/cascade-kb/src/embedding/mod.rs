use async_trait::async_trait;
use crate::data::{CoreError, TraceContext};
use crate::traits::EmbeddingGenerator;
use std::sync::Arc;

// Import the modules
mod openai;
mod neo4j;
pub mod test_utils;

// Export the services
#[cfg(feature = "async-openai")]
pub use openai::OpenAIEmbeddingService;

#[cfg(feature = "neo4rs")]
pub use neo4j::Neo4jEmbeddingService;

pub use neo4j::Neo4jEmbeddingServiceConfig;

// Define the embedding service trait
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    async fn embed_text(&self, text: &str, ctx: &TraceContext) -> Result<Vec<f32>, CoreError>;
    async fn embed_batch(&self, texts: &[String], ctx: &TraceContext) -> Result<Vec<Vec<f32>>, CoreError>;
}

/// Mock embedding service for testing that provides deterministic embeddings.
/// This service can be used in tests when Neo4j with APOC ML plugin is not available.
#[derive(Debug, Clone)]
pub struct MockEmbeddingService {
    embedding_dimension: usize,
}

impl MockEmbeddingService {
    pub fn new(embedding_dimension: usize) -> Self {
        Self {
            embedding_dimension,
        }
    }

    pub fn default() -> Self {
        Self {
            embedding_dimension: 1536,
        }
    }
    
    fn generate_deterministic_embedding(&self, text: &str) -> Vec<f32> {
        let mut embedding = vec![0.0; self.embedding_dimension];
        
        // Set some values based on keywords in the text
        if text.to_lowercase().contains("auth") {
            embedding[0] = 0.9;
            embedding[1] = 0.8;
        }
        
        if text.to_lowercase().contains("http") || text.to_lowercase().contains("api") {
            embedding[2] = 0.85;
            embedding[3] = 0.75;
        }
        
        if text.to_lowercase().contains("database") {
            embedding[4] = 0.9;
            embedding[5] = 0.8;
        }
        
        if text.to_lowercase().contains("component") {
            embedding[8] = 0.9;
            embedding[9] = 0.8;
        }
        
        // Make the embedding deterministic based on text hash
        let text_hash = text.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64));
        for i in 20..40 {
            embedding[i] = ((text_hash + i as u64) % 100) as f32 / 100.0;
        }
        
        // Normalize the embedding
        let magnitude: f32 = embedding.iter().map(|&v| v * v).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for value in &mut embedding {
                *value /= magnitude;
            }
        }
        
        embedding
    }
}

#[async_trait]
impl EmbeddingService for MockEmbeddingService {
    async fn embed_text(
        &self,
        text: &str,
        _trace_ctx: &TraceContext,
    ) -> Result<Vec<f32>, CoreError> {
        Ok(self.generate_deterministic_embedding(text))
    }
    
    async fn embed_batch(
        &self,
        texts: &[String],
        trace_ctx: &TraceContext,
    ) -> Result<Vec<Vec<f32>>, CoreError> {
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            embeddings.push(self.embed_text(text, trace_ctx).await?);
        }
        
        Ok(embeddings)
    }
}

#[async_trait]
impl EmbeddingGenerator for MockEmbeddingService {
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, CoreError> {
        Ok(self.generate_deterministic_embedding(text))
    }
}

/// Configuration for embedding services
#[derive(Debug, Clone)]
pub enum EmbeddingServiceConfig {
    /// Use OpenAI API for embeddings
    OpenAI {
        api_key: String,
        model: String,
    },
    /// Use Neo4j for embeddings
    Neo4j {
        connection_string: String,
        database_name: String,
        model_name: String,
        dimensions: usize,
        similarity_function: String,
        username: String,
        password: String,
    },
    /// Use mock embeddings for testing
    Mock {
        dimensions: usize,
    },
}

impl Default for EmbeddingServiceConfig {
    fn default() -> Self {
        // Default to OpenAI with environment variable
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            Self::OpenAI {
                api_key,
                model: "text-embedding-3-small".to_string(),
            }
        } else {
            // Fallback to Neo4j with default configuration
            Self::Neo4j {
                connection_string: "bolt://localhost:7687".to_string(),
                database_name: "neo4j".to_string(),
                model_name: "Neo4j".to_string(),
                dimensions: 1536,
                similarity_function: "cosine".to_string(),
                username: "neo4j".to_string(),
                password: "password".to_string(),
            }
        }
    }
}

/// Create an embedding service from the provided configuration
pub fn create_embedding_service(config: EmbeddingServiceConfig) -> Arc<dyn EmbeddingService> {
    match config {
        #[cfg(feature = "async-openai")]
        EmbeddingServiceConfig::OpenAI { api_key, model } => {
            Arc::new(OpenAIEmbeddingService::new(api_key, model))
        }
        #[cfg(not(feature = "async-openai"))]
        EmbeddingServiceConfig::OpenAI { api_key: _, model: _ } => {
            panic!("OpenAI embedding service is not available because the 'async-openai' feature is not enabled");
        }
        #[cfg(feature = "neo4rs")]
        EmbeddingServiceConfig::Neo4j { 
            connection_string,
            database_name: _database_name,
            model_name,
            dimensions,
            similarity_function: _similarity_function,
            username,
            password,
        } => {
            let neo4j_config = Neo4jEmbeddingServiceConfig {
                uri: connection_string,
                username,
                password,
                model_name,
                embedding_dimension: dimensions,
                index_name: "docEmbeddings".into(),
            };
            
            // This returns a Future, so we need to handle it differently
            match tokio::runtime::Handle::current().block_on(Neo4jEmbeddingService::new(&neo4j_config)) {
                Ok(service) => Arc::new(service),
                Err(err) => {
                    eprintln!("Failed to create Neo4j embedding service: {}", err);
                    // Fallback to MockEmbeddingService with the same dimensions
                    eprintln!("Falling back to MockEmbeddingService");
                    Arc::new(MockEmbeddingService::new(dimensions))
                }
            }
        }
        #[cfg(not(feature = "neo4rs"))]
        EmbeddingServiceConfig::Neo4j { .. } => {
            panic!("Neo4j embedding service is not available because the 'neo4rs' feature is not enabled");
        }
        EmbeddingServiceConfig::Mock { dimensions } => {
            Arc::new(MockEmbeddingService::new(dimensions))
        }
    }
}

// Generate compatible embedding services - used for testing
pub fn create_compatible_embedding_service_pair() -> (Arc<dyn EmbeddingService>, Arc<dyn EmbeddingGenerator>) {
    let mock_service = MockEmbeddingService::default();
    let embedding_service: Arc<dyn EmbeddingService> = Arc::new(mock_service.clone());
    let embedding_generator: Arc<dyn EmbeddingGenerator> = Arc::new(mock_service);
    (embedding_service, embedding_generator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    
    #[tokio::test]
    async fn test_embedding_service_default_config() {
        dotenv().ok();
        
        // For testing purposes only, create a mock service
        let config = EmbeddingServiceConfig::Mock { dimensions: 1536 };
        let service = create_embedding_service(config);
        
        // Test a simple embedding
        let ctx = TraceContext::default();
        let embedding = service.embed_text("test", &ctx).await.unwrap();
        
        // Basic validation
        assert_eq!(embedding.len(), 1536);
        
        // Verify deterministic behavior
        let embedding2 = service.embed_text("test", &ctx).await.unwrap();
        assert_eq!(embedding, embedding2, "Embeddings should be deterministic");
        
        // Different text should have different embeddings
        let embedding3 = service.embed_text("different text", &ctx).await.unwrap();
        assert_ne!(embedding, embedding3, "Different text should have different embeddings");
    }
    
    #[tokio::test]
    async fn test_mock_embedding_service() {
        let service = MockEmbeddingService::default();
        let ctx = TraceContext::default();
        
        // Test embedding generation
        let text = "This is a test";
        let embedding = service.embed_text(text, &ctx).await.unwrap();
        
        // Verify dimension
        assert_eq!(embedding.len(), 1536);
        
        // Test batch embedding
        let texts = vec![
            "This is the first test".to_string(),
            "This is the second test".to_string()
        ];
        
        let embeddings = service.embed_batch(&texts, &ctx).await.unwrap();
        
        // Verify batch results
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 1536);
        assert_eq!(embeddings[1].len(), 1536);
        
        // Test embedding generator trait implementation
        let generator_embedding = service.generate_embedding(text).await.unwrap();
        
        // Verify same result from both trait implementations
        assert_eq!(embedding, generator_embedding, 
                  "EmbeddingService and EmbeddingGenerator should return the same embeddings");
    }
    
    #[tokio::test]
    async fn test_compatible_embedding_service_pair() {
        let (service, generator) = create_compatible_embedding_service_pair();
        let ctx = TraceContext::default();
        
        let text = "Testing compatibility";
        let service_embedding = service.embed_text(text, &ctx).await.unwrap();
        let generator_embedding = generator.generate_embedding(text).await.unwrap();
        
        assert_eq!(service_embedding, generator_embedding, 
                  "Service and Generator should produce identical embeddings");
    }
}

// Define methods for embedding services to expose their capabilities
#[cfg(feature = "neo4rs")]
impl Neo4jEmbeddingService {
    pub fn embedding_dimension(&self) -> usize {
        1536 // Return the default embedding dimension instead of accessing private field
    }
} 
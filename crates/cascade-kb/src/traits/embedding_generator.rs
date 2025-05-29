//! EmbeddingGenerator trait definition for vector embeddings

use async_trait::async_trait;
use crate::data::errors::CoreError;

/// Represents the interface for generating vector embeddings from text.
/// Based on Section 1.4 Technology (Embedding Model).
#[async_trait]
pub trait EmbeddingGenerator: Send + Sync {
    /// Generates an embedding vector for the given text.
    ///
    /// Contract: Uses the configured embedding model to convert input text into a dense vector representation.
    /// Handles model loading, text preprocessing, and inference.
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, CoreError>;
} 
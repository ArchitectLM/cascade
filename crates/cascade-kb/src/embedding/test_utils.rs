use std::sync::Arc;
use async_trait::async_trait;
use crate::data::CoreError;
use crate::traits::EmbeddingGenerator;
use uuid::Uuid;

/// A predictable embedding generator for tests that creates deterministic
/// embeddings based on text content. This ensures consistent vector search
/// results in tests without requiring external embedding services.
#[derive(Debug, Clone)]
pub struct PredictableEmbeddingGenerator {
    embedding_dimension: usize,
}

impl PredictableEmbeddingGenerator {
    pub fn new(embedding_dimension: usize) -> Self {
        Self {
            embedding_dimension,
        }
    }

    pub fn with_default_dimension() -> Self {
        Self {
            embedding_dimension: 1536,
        }
    }
}

#[async_trait]
impl EmbeddingGenerator for PredictableEmbeddingGenerator {
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, CoreError> {
        let mut embedding = vec![0.0; self.embedding_dimension];
        
        // Generate a hash from the text for stability
        let text_hash = text.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64));
        
        // Seed the embedding with the hash
        let mut seed = text_hash;
        
        // Fill the embedding with deterministic but unique values
        for i in 0..self.embedding_dimension {
            // Generate a different value for each position based on the seed
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let value = ((seed >> 32) as f32) / (u32::MAX as f32) * 2.0 - 1.0;
            embedding[i] = value;
        }
        
        // Add special values for common keywords to make semantic searches more realistic
        if text.to_lowercase().contains("auth") || text.to_lowercase().contains("security") {
            // Security cluster
            for i in 0..10 {
                embedding[i] = 0.8 - (i as f32 * 0.05);
            }
        }
        
        if text.to_lowercase().contains("http") || text.to_lowercase().contains("api") {
            // API cluster
            for i in 10..20 {
                embedding[i] = 0.8 - ((i - 10) as f32 * 0.05);
            }
        }
        
        if text.to_lowercase().contains("database") || text.to_lowercase().contains("sql") {
            // Database cluster
            for i in 20..30 {
                embedding[i] = 0.8 - ((i - 20) as f32 * 0.05);
            }
        }
        
        if text.to_lowercase().contains("component") || text.to_lowercase().contains("flow") {
            // Component/Flow cluster
            for i in 30..40 {
                embedding[i] = 0.8 - ((i - 30) as f32 * 0.05);
            }
        }
        
        if text.to_lowercase().contains("route") || text.to_lowercase().contains("condition") {
            // Control flow cluster
            for i in 40..50 {
                embedding[i] = 0.8 - ((i - 40) as f32 * 0.05);
            }
        }
        
        // Normalize the embedding to unit length for cosine similarity
        let magnitude = embedding.iter()
            .map(|&v| v * v)
            .sum::<f32>()
            .sqrt();
            
        if magnitude > 0.0 {
            for value in &mut embedding {
                *value /= magnitude;
            }
        }
        
        Ok(embedding)
    }
}

/// A factory function to create an embedding generator for tests
/// Returns a mockable embedding generator that produces consistent results
pub fn create_test_embedding_generator(dimension: usize) -> Arc<dyn EmbeddingGenerator> {
    // Check environment variables
    let use_predictable = std::env::var("USE_PREDICTABLE_EMBEDDING")
        .map(|val| val.to_lowercase() == "true")
        .unwrap_or(true);
        
    if use_predictable {
        Arc::new(PredictableEmbeddingGenerator::new(dimension))
    } else {
        Arc::new(PseudoRandomEmbeddingGenerator::new(dimension))
    }
}

/// A test embedding generator that produces pseudo-random embeddings for testing
/// without relying on the rand crate
#[derive(Debug, Clone)]
pub struct PseudoRandomEmbeddingGenerator {
    embedding_dimension: usize,
}

impl PseudoRandomEmbeddingGenerator {
    pub fn new(embedding_dimension: usize) -> Self {
        Self {
            embedding_dimension,
        }
    }
    
    // Generate a pseudo-random number based on a seed
    fn pseudo_random(seed: u64, index: usize) -> f32 {
        // Simple xorshift algorithm to generate pseudo-random values
        let mut x = seed.wrapping_add(index as u64);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        // Convert to a float between -1 and 1
        (x as f32 / u64::MAX as f32) * 2.0 - 1.0
    }
}

#[async_trait]
impl EmbeddingGenerator for PseudoRandomEmbeddingGenerator {
    async fn generate_embedding(&self, _text: &str) -> Result<Vec<f32>, CoreError> {
        // Generate a random embedding for testing
        // Use the text's hash as a seed for determinism within the same run
        let seed = _text.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64));
        
        let embedding: Vec<f32> = (0..self.embedding_dimension)
            .map(|i| Self::pseudo_random(seed, i))
            .collect();
        
        // Normalize
        let magnitude = embedding.iter()
            .map(|&v| v * v)
            .sum::<f32>()
            .sqrt();
            
        if magnitude > 0.0 {
            let normalized = embedding.iter()
                .map(|&v| v / magnitude)
                .collect();
            Ok(normalized)
        } else {
            Ok(embedding)
        }
    }
} 
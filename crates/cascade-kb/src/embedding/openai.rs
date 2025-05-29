#[cfg(feature = "async-openai")]
use async_openai::{
    types::{CreateEmbeddingRequestArgs, EmbeddingInput},
    Client, config::OpenAIConfig,
};
use async_trait::async_trait;

use crate::data::{CoreError, TraceContext};
use super::EmbeddingService;

#[cfg(feature = "async-openai")]
pub struct OpenAIEmbeddingService {
    client: Client<OpenAIConfig>,
    model: String,
}

#[cfg(feature = "async-openai")]
impl OpenAIEmbeddingService {
    pub fn new(api_key: String, model: String) -> Self {
        let config = OpenAIConfig::new().with_api_key(api_key);
        let client = Client::with_config(config);
        Self { client, model }
    }
}

#[cfg(feature = "async-openai")]
#[async_trait]
impl EmbeddingService for OpenAIEmbeddingService {
    async fn embed_text(&self, text: &str, _ctx: &TraceContext) -> Result<Vec<f32>, CoreError> {
        let request = CreateEmbeddingRequestArgs::default()
            .model(&self.model)
            .input(EmbeddingInput::String(text.to_string()))
            .build()
            .map_err(|e| CoreError::EmbeddingError(e.to_string()))?;

        let response = self.client
            .embeddings()
            .create(request)
            .await
            .map_err(|e| CoreError::EmbeddingError(e.to_string()))?;

        Ok(response.data[0].embedding.clone())
    }

    async fn embed_batch(&self, texts: &[String], _ctx: &TraceContext) -> Result<Vec<Vec<f32>>, CoreError> {
        let request = CreateEmbeddingRequestArgs::default()
            .model(&self.model)
            .input(texts.to_vec())
            .build()
            .map_err(|e| CoreError::EmbeddingError(e.to_string()))?;

        let response = self.client
            .embeddings()
            .create(request)
            .await
            .map_err(|e| CoreError::EmbeddingError(e.to_string()))?;

        Ok(response.data.into_iter().map(|d| d.embedding).collect())
    }
}

#[cfg(all(test, feature = "async-openai"))]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use std::env;

    #[tokio::test]
    async fn test_embed_text() {
        dotenv().ok();
        let api_key = match env::var("OPENAI_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping test: OPENAI_API_KEY not set");
                return;
            }
        };
        
        let service = OpenAIEmbeddingService::new(api_key, "text-embedding-3-small".to_string());
        let ctx = TraceContext::default();
        
        let text = "Hello, world!";
        let result = service.embed_text(text, &ctx).await;
        
        if result.is_err() {
            let err = result.unwrap_err();
            eprintln!("OpenAI API error: {}", err);
            return;
        }
        
        let embedding = result.unwrap();
        assert!(!embedding.is_empty());
    }

    #[tokio::test]
    async fn test_embed_batch() {
        dotenv().ok();
        let api_key = match env::var("OPENAI_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                eprintln!("Skipping test: OPENAI_API_KEY not set");
                return;
            }
        };
        
        let service = OpenAIEmbeddingService::new(api_key, "text-embedding-3-small".to_string());
        let ctx = TraceContext::default();
        
        let texts = vec!["Hello".to_string(), "World".to_string()];
        let result = service.embed_batch(&texts, &ctx).await;
        
        if result.is_err() {
            let err = result.unwrap_err();
            eprintln!("OpenAI API error: {}", err);
            return;
        }
        
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 2);
        assert!(!embeddings[0].is_empty());
        assert!(!embeddings[1].is_empty());
    }
} 
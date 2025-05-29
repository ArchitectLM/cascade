#[async_trait]
impl ContentStorage for FileContentStore {
    async fn store_content_addressed(&self, content: &[u8]) -> ContentStoreResult<ContentHash> {
        // ... existing code ...
    }

    // ... other methods ...

    async fn delete_content(&self, hash: &ContentHash) -> ContentStoreResult<()> {
        // ... existing code ...
    }

    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any where Self: 'static {
        self
    }
} 
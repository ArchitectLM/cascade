//! Mock implementation of the EdgePlatform client interface.

use async_trait::async_trait;
use mockall::mock;
use mockall::predicate::*;
use serde_json::Value;

/// WorkerInfo represents information about a deployed worker
#[derive(Debug, Clone, PartialEq)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub status: WorkerStatus,
    pub deployment_url: String,
    pub version: String,
}

/// WorkerStatus represents the current status of a worker
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerStatus {
    Deployed,
    Deploying,
    Failed,
    NotFound,
}

/// Define the EdgePlatform trait interface based on the actual client
#[async_trait]
pub trait EdgePlatform: Send + Sync {
    async fn deploy_worker(&self, worker_id: &str, code: &[u8], config: Value) -> Result<WorkerInfo, EdgeError>;
    async fn delete_worker(&self, worker_id: &str) -> Result<bool, EdgeError>;
    async fn get_worker_status(&self, worker_id: &str) -> Result<Option<WorkerInfo>, EdgeError>;
    async fn list_workers(&self) -> Result<Vec<WorkerInfo>, EdgeError>;
}

/// Error type for edge platform operations
#[derive(Debug, thiserror::Error)]
pub enum EdgeError {
    #[error("Authentication error: {0}")]
    AuthError(String),
    #[error("Deployment error: {0}")]
    DeploymentError(String),
    #[error("Worker not found: {0}")]
    WorkerNotFound(String),
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Edge platform error: {0}")]
    Other(String),
}

// Generate the mock implementation
mock! {
    pub EdgePlatform {}
    
    #[async_trait]
    impl EdgePlatform for EdgePlatform {
        async fn deploy_worker(&self, worker_id: &str, code: &[u8], config: Value) -> Result<WorkerInfo, EdgeError>;
        async fn delete_worker(&self, worker_id: &str) -> Result<bool, EdgeError>;
        async fn get_worker_status(&self, worker_id: &str) -> Result<Option<WorkerInfo>, EdgeError>;
        async fn list_workers(&self) -> Result<Vec<WorkerInfo>, EdgeError>;
    }
}

/// Creates a new mock EdgePlatform instance with default expectations.
pub fn create_mock_edge_platform() -> MockEdgePlatform {
    let mut mock = MockEdgePlatform::new();
    
    // Set up default behaviors for common methods
    mock.expect_deploy_worker()
        .returning(|worker_id, _, _| {
            Ok(WorkerInfo {
                worker_id: worker_id.to_string(),
                status: WorkerStatus::Deployed,
                deployment_url: format!("https://{}.example.com", worker_id),
                version: "1.0.0".to_string(),
            })
        });
    
    mock.expect_delete_worker()
        .returning(|_| Ok(true));
    
    mock.expect_list_workers()
        .returning(|| Ok(Vec::new()));
    
    mock
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_mock_edge_platform_default_behavior() {
        let mock = create_mock_edge_platform();
        
        let worker_info = mock.deploy_worker("test-worker", b"code", json!({})).await.unwrap();
        assert_eq!(worker_info.worker_id, "test-worker");
        assert_eq!(worker_info.status, WorkerStatus::Deployed);
        
        let result = mock.delete_worker("test-worker").await.unwrap();
        assert!(result);
        
        let workers = mock.list_workers().await.unwrap();
        assert!(workers.is_empty());
    }
    
    #[tokio::test]
    async fn test_mock_edge_platform_custom_behavior() {
        let mut mock = create_mock_edge_platform();
        
        // Configure custom behavior
        mock.expect_get_worker_status()
            .returning(|worker_id| {
                if worker_id == "existing-worker" {
                    Ok(Some(WorkerInfo {
                        worker_id: worker_id.to_string(),
                        status: WorkerStatus::Deployed,
                        deployment_url: format!("https://{}.example.com", worker_id),
                        version: "1.0.0".to_string(),
                    }))
                } else {
                    Ok(None)
                }
            });
        
        // Test the behavior for existing worker
        let status = mock.get_worker_status("existing-worker").await.unwrap();
        assert!(status.is_some());
        assert_eq!(status.unwrap().worker_id, "existing-worker");
        
        // Test the behavior for non-existing worker
        let status = mock.get_worker_status("non-existing-worker").await.unwrap();
        assert!(status.is_none());
    }
} 
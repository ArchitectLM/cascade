// Test utilities for PostgreSQL state store
// This module provides utilities for testing with the PostgreSQL state store

use crate::{
    PostgresFlowInstanceRepository,
    PostgresFlowDefinitionRepository,
    PostgresComponentStateRepository,
    PostgresTimerRepository,
    PostgresConnection,
};

use cascade_core::{
    domain::repository::{
        FlowInstanceRepository, FlowDefinitionRepository, 
        ComponentStateRepository, TimerRepository
    },
    domain::flow_instance::{FlowInstance, FlowInstanceId, FlowId, StepId, CorrelationId, FlowStatus},
    domain::flow_definition::FlowDefinition,
    CoreError,
};

use tokio::task;
use std::sync::Arc;

/// A mock state store for testing with PostgreSQL repositories
#[derive(Clone)]
pub struct MockStateStore {
    pub flow_instance_repo: PostgresFlowInstanceRepository,
    pub flow_definition_repo: PostgresFlowDefinitionRepository,
    pub component_state_repo: PostgresComponentStateRepository,
    pub timer_repo: PostgresTimerRepository,
}

impl MockStateStore {
    /// Create a new mock state store for testing
    pub async fn new() -> Self {
        // Create a test mode connection that doesn't require a real database
        let conn = PostgresConnection::new_test_mode();
        
        // Create the repositories
        let flow_instance_repo = PostgresFlowInstanceRepository::new(conn.clone());
        let flow_definition_repo = PostgresFlowDefinitionRepository::new(conn.clone());
        let component_state_repo = PostgresComponentStateRepository::new(conn.clone());
        let timer_repo = PostgresTimerRepository::new(conn.clone());
        
        Self {
            flow_instance_repo,
            flow_definition_repo,
            component_state_repo,
            timer_repo,
        }
    }
    
    /// Get the flow instance repository
    pub fn flow_instance_repository(&self) -> &PostgresFlowInstanceRepository {
        &self.flow_instance_repo
    }
    
    /// Get the flow definition repository
    pub fn flow_definition_repository(&self) -> &PostgresFlowDefinitionRepository {
        &self.flow_definition_repo
    }
    
    /// Get the component state repository
    pub fn component_state_repository(&self) -> &PostgresComponentStateRepository {
        &self.component_state_repo
    }
    
    /// Get the timer repository
    pub fn timer_repository(&self) -> &PostgresTimerRepository {
        &self.timer_repo
    }
}

/// Create a mock state store for testing
/// This is a convenience function for tests
pub async fn create_mock_state_store() -> MockStateStore {
    MockStateStore::new().await
}

/// Create a mock state store synchronously
/// This is useful for tests that can't use async
pub fn create_mock_state_store_sync() -> MockStateStore {
    task::block_in_place(|| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            MockStateStore::new().await
        })
    })
} 

#[cfg(test)]
mod test {
    use super::*;
    use cascade_core::application::runtime_interface::TimerProcessingRepository;
    use std::sync::Arc;
    use std::pin::Pin;
    use std::future::Future;
    use tokio_test::block_on;
    use std::time::Duration;
    use cascade_core::domain::flow_instance::{FlowInstanceId, StepId};
    
    #[test]
    fn timer_repository_test_mode() {
        // Create a mock state store with test mode connection
        let conn = PostgresConnection::new_test_mode();
        let repo = crate::repositories::PostgresTimerRepository::new(conn);
        
        // Test timer processing in test mode
        let callback = Arc::new(|flow_id: FlowInstanceId, step_id: StepId| {
            Box::pin(async move {
                // This should never get called in test mode
                panic!("Timer callback was invoked in test mode");
            }) as Pin<Box<dyn Future<Output = ()> + Send>>
        });
        
        // Start timer processing - should return immediately in test mode
        repo.start_timer_processing(callback);
        
        // Schedule a timer - should return a mock timer ID
        let flow_id = FlowInstanceId("test-flow".to_string());
        let step_id = StepId("test-step".to_string());
        let duration = Duration::from_secs(10);
        
        let timer_id = block_on(async {
            repo.schedule(&flow_id, &step_id, duration).await.unwrap()
        });
        
        assert!(timer_id.starts_with("mock-timer-"), "Timer ID should be a mock in test mode");
        
        // Cancel should succeed in test mode
        let result = block_on(async {
            repo.cancel(&timer_id).await
        });
        
        assert!(result.is_ok(), "Cancel should succeed in test mode");
    }
}
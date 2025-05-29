#[cfg(test)]
mod tests {
    use cascade_core::domain::flow_instance::{FlowInstanceId, StepId};
    use cascade_core::domain::repository::TimerRepository;
    use cascade_core::application::runtime_interface::TimerProcessingRepository;
    use crate::PostgresConnection;
    use crate::repositories::PostgresTimerRepository;
    use std::sync::Arc;
    use std::pin::Pin;
    use std::future::Future;
    use std::time::Duration;
    use tokio_test::block_on;
    
    #[test]
    fn timer_repository_test_mode() {
        // Create a test mode connection
        let conn = PostgresConnection::new_test_mode();
        
        // Create timer repository 
        let repo = PostgresTimerRepository::new(conn);
        
        // Schedule a timer (should return mock ID)
        let flow_id = FlowInstanceId("test-flow".to_string());
        let step_id = StepId("test-step".to_string());
        let duration = Duration::from_secs(10);
        
        let timer_id = block_on(async {
            repo.schedule(&flow_id, &step_id, duration).await.unwrap()
        });
        
        assert!(timer_id.starts_with("mock-timer-"), "Timer ID should have mock- prefix");
        
        // Cancel the timer (should succeed without errors)
        let cancel_result = block_on(async {
            repo.cancel(&timer_id).await
        });
        
        assert!(cancel_result.is_ok(), "Cancel should succeed in test mode");
        
        // Test timer processing with a callback that would panic if called
        let callback = Arc::new(|_: FlowInstanceId, _: StepId| {
            Box::pin(async move {
                panic!("This should not be called in test mode");
            }) as Pin<Box<dyn Future<Output = ()> + Send>>
        });
        
        // This should not panic since the background task won't run in test mode
        repo.start_timer_processing(callback);
        
        // Test complete
        println!("Timer repository test mode test passed");
    }
} 
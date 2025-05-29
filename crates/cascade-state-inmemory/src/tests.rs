use crate::InMemoryStateStoreProvider;
use cascade_core::{
    domain::flow_instance::{FlowInstance, FlowInstanceId, FlowId, StepId},
    domain::flow_definition::FlowDefinition,
    application::repositories::{
        FlowInstanceRepository, FlowDefinitionRepository, ComponentStateRepository, TimerRepository,
        CorrelationRepository,
    },
    CoreError,
    domain::timer::TimerEvent,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_flow_instance_repository() -> Result<(), CoreError> {
    let provider = InMemoryStateStoreProvider::new();
    let (flow_instance_repo, _, _, _, _) = provider.create_repositories();
    
    // Create test flow instance
    let flow_id = FlowId("test_flow".to_string());
    let instance_id = FlowInstanceId("test_instance".to_string());
    let instance = FlowInstance {
        id: instance_id.clone(),
        flow_id: flow_id.clone(),
        status: cascade_core::domain::flow_instance::FlowStatus::Running,
        current_step: None,
        data: json!({}),
        correlation_data: Default::default(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    
    // Save instance
    flow_instance_repo.save(&instance).await?;
    
    // Find by ID
    let found = flow_instance_repo.find_by_id(&instance_id).await?;
    assert!(found.is_some());
    assert_eq!(found.unwrap().id.0, instance.id.0);
    
    // Find by flow
    let instances = flow_instance_repo.find_all_for_flow(&flow_id).await?;
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].0, instance_id.0);
    
    // Delete
    flow_instance_repo.delete(&instance_id).await?;
    let found = flow_instance_repo.find_by_id(&instance_id).await?;
    assert!(found.is_none());
    
    Ok(())
}

#[tokio::test]
async fn test_flow_definition_repository() -> Result<(), CoreError> {
    let provider = InMemoryStateStoreProvider::new();
    let (_, flow_def_repo, _, _, _) = provider.create_repositories();
    
    // Create test flow definition
    let flow_id = FlowId("test_def".to_string());
    let definition = FlowDefinition {
        id: flow_id.clone(),
        version: "1.0.0".to_string(),
        name: "Test Flow".to_string(),
        description: Some("Test description".to_string()),
        steps: vec![],
        trigger: None,
    };
    
    // Save definition
    flow_def_repo.save(&definition).await?;
    
    // Find by ID
    let found = flow_def_repo.find_by_id(&flow_id).await?;
    assert!(found.is_some());
    assert_eq!(found.unwrap().id.0, definition.id.0);
    
    // Find all
    let definitions = flow_def_repo.find_all().await?;
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].id.0, definition.id.0);
    
    // Delete
    flow_def_repo.delete(&flow_id).await?;
    let found = flow_def_repo.find_by_id(&flow_id).await?;
    assert!(found.is_none());
    
    Ok(())
}

#[tokio::test]
async fn test_component_state_repository() -> Result<(), CoreError> {
    let provider = InMemoryStateStoreProvider::new();
    let (_, _, component_state_repo, _, _) = provider.create_repositories();
    
    // Create test component state
    let flow_id = FlowInstanceId("test_flow_instance".to_string());
    let step_id = StepId("test_step".to_string());
    let state = json!({
        "counter": 42,
        "message": "Hello, World!"
    });
    
    // Save state
    component_state_repo.save_state(&flow_id, &step_id, state.clone()).await?;
    
    // Get state
    let found = component_state_repo.get_state(&flow_id, &step_id).await?;
    assert!(found.is_some());
    assert_eq!(found.unwrap()["counter"], 42);
    
    // Delete state
    component_state_repo.delete_state(&flow_id, &step_id).await?;
    let found = component_state_repo.get_state(&flow_id, &step_id).await?;
    assert!(found.is_none());
    
    Ok(())
}

#[tokio::test]
async fn test_correlation_repository() -> Result<(), CoreError> {
    let provider = InMemoryStateStoreProvider::new();
    let (_, _, _, _, correlation_repo) = provider.create_repositories();
    
    // Create test correlation
    let correlation_id = cascade_core::domain::flow_instance::CorrelationId("test_correlation".to_string());
    let instance_id = FlowInstanceId("test_instance".to_string());
    
    // Save correlation
    correlation_repo.save_correlation(&correlation_id, &instance_id).await?;
    
    // Get instance ID by correlation
    let found = correlation_repo.get_instance_id(&correlation_id).await?;
    assert!(found.is_some());
    assert_eq!(found.unwrap().0, instance_id.0);
    
    // Delete correlation
    correlation_repo.delete_correlation(&correlation_id).await?;
    let found = correlation_repo.get_instance_id(&correlation_id).await?;
    assert!(found.is_none());
    
    Ok(())
}

#[tokio::test]
async fn test_timer_repository() -> Result<(), CoreError> {
    // Setup timer receiver
    let (timer_tx, mut timer_rx) = mpsc::channel::<TimerEvent>(10);
    
    // Create provider with custom timer channel for testing
    let mut provider = InMemoryStateStoreProvider::new();
    let (_, _, _, timer_repo, _) = provider.create_repositories();
    
    // Schedule a timer (very short duration for testing)
    let flow_id = FlowInstanceId("test_flow_instance".to_string());
    let step_id = StepId("test_step".to_string());
    let timer_id = timer_repo.schedule(&flow_id, &step_id, Duration::from_millis(50)).await?;
    
    // Start the timer processor
    provider.start_timer_processor(|event| {
        if let Err(e) = timer_tx.try_send(event) {
            eprintln!("Failed to forward timer event: {}", e);
        }
        Ok(())
    })?;
    
    // Wait for timer to fire
    let timer_event = tokio::time::timeout(Duration::from_millis(200), timer_rx.recv()).await
        .map_err(|_| CoreError::TimeoutError("Timer didn't fire within expected time".to_string()))?
        .ok_or_else(|| CoreError::InternalError("Timer channel closed unexpectedly".to_string()))?;
    
    // Verify timer event
    assert_eq!(timer_event.flow_instance_id.0, flow_id.0);
    assert_eq!(timer_event.step_id.0, step_id.0);
    
    Ok(())
}

#[tokio::test]
async fn test_provider_create_repositories() {
    let provider = InMemoryStateStoreProvider::new();
    let (flow_repo, flow_def_repo, component_repo, timer_repo, correlation_repo) = provider.create_repositories();
    
    // Just verify that we can create all repositories
    assert!(Arc::strong_count(&flow_repo) == 1);
    assert!(Arc::strong_count(&flow_def_repo) == 1);
    assert!(Arc::strong_count(&component_repo) == 1);
    assert!(Arc::strong_count(&timer_repo) == 1);
    assert!(Arc::strong_count(&correlation_repo) == 1);
} 
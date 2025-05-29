//! Examples of using the in-memory state store with Cascade Core
//!
//! This module contains examples showing how to integrate the in-memory
//! state store with the Cascade Core Runtime.

#[cfg(test)]
mod examples {
    use cascade_core::{
        RuntimeInterface,
        application::flow_execution_service::DomainEventHandler,
        domain::events::DomainEvent,
        domain::flow_instance::{FlowInstanceId, FlowId},
        domain::flow_definition::{FlowDefinition, StepDefinition, DataReference, TriggerDefinition},
        CoreError,
        ComponentExecutor,
        DataPacket,
        ComponentRuntimeApi,
        ExecutionResult,
    };
    use crate::InMemoryStateStoreProvider;
    use std::sync::Arc;
    use std::collections::HashMap;
    use async_trait::async_trait;
    use serde_json::json;

    // Sample event handler
    struct NoOpEventHandler;

    #[async_trait]
    impl DomainEventHandler for NoOpEventHandler {
        async fn handle_event(&self, _event: Box<dyn DomainEvent>) -> Result<(), CoreError> {
            Ok(())
        }
    }

    // Sample component
    #[derive(Debug)]
    struct EchoComponent;

    #[async_trait]
    impl ComponentExecutor for EchoComponent {
        async fn execute(&self, api: &dyn ComponentRuntimeApi) -> ExecutionResult {
            // Echo input to output
            let input = match api.get_input("input").await {
                Ok(input) => input,
                Err(_) => DataPacket::null(),
            };
            
            let _ = api.set_output("output", input).await;
            ExecutionResult::Success
        }
    }

    // Sample component factory
    fn create_component_factory() -> Arc<dyn Fn(&str) -> Result<Arc<dyn ComponentExecutor>, CoreError> + Send + Sync> {
        Arc::new(|component_type: &str| -> Result<Arc<dyn ComponentExecutor>, CoreError> {
            match component_type {
                "Echo" => Ok(Arc::new(EchoComponent)),
                _ => Err(CoreError::ComponentNotFoundError(format!("Component type not found: {}", component_type))),
            }
        })
    }
    
    // Create a test flow definition
    fn create_test_flow_definition() -> FlowDefinition {
        let flow_id = FlowId("test_flow".to_string());
        FlowDefinition {
            id: flow_id.clone(),
            version: "1.0.0".to_string(),
            name: "Test Flow".to_string(),
            description: Some("A test flow for in-memory state store example".to_string()),
            steps: vec![
                StepDefinition {
                    id: "echo_step".to_string(),
                    component_type: "Echo".to_string(),
                    input_mapping: {
                        let mut map = HashMap::new();
                        map.insert("input".to_string(), DataReference { 
                            expression: "$flow.trigger_data".to_string() 
                        });
                        map
                    },
                    config: json!({}),
                    run_after: vec![],
                    condition: None,
                },
            ],
            trigger: Some(TriggerDefinition {
                trigger_type: "http".to_string(),
                config: json!({"method": "POST", "path": "/trigger"}),
            }),
        }
    }

    /// Example of creating and using the Cascade Core Runtime with in-memory state store
    #[tokio::test]
    async fn example_inmemory_state_store() -> Result<(), CoreError> {
        // Set up tracing (optional)
        if let Err(_) = tracing_subscriber::fmt()
            .with_env_filter("debug")
            .try_init() {
            // Ignore errors if tracing is already initialized
        }

        // Create the in-memory state store provider
        let inmemory_provider = InMemoryStateStoreProvider::new();

        // Get the repositories
        let repositories = inmemory_provider.create_repositories();

        // Create component factory
        let component_factory = create_component_factory();

        // Create event handler
        let event_handler = Arc::new(NoOpEventHandler);

        // Create the runtime
        let runtime = RuntimeInterface::create_with_repositories(
            repositories,
            component_factory,
            event_handler,
        ).await?;

        // Deploy a test flow definition
        let flow_def = create_test_flow_definition();
        let flow_id = flow_def.id.clone();
        runtime.deploy_definition(flow_def).await?;
        
        // List deployed flows
        let flows = runtime.list_definitions().await?;
        assert_eq!(flows.len(), 1);
        
        // Trigger the flow with some data
        let trigger_data = DataPacket::new(json!({"message": "Hello, World!"}));
        let instance_id = runtime.trigger_flow(flow_id, trigger_data).await?;
        
        // The flow should execute immediately since it's in-memory
        // Let's wait a tiny bit to be safe
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        
        // Check flow instance state
        let state = runtime.get_flow_instance_state(instance_id).await?;
        assert!(state.is_some());
        
        Ok(())
    }
    
    /// Demonstrate timer functionality with in-memory state store
    #[tokio::test]
    async fn example_inmemory_timer() -> Result<(), CoreError> {
        // Create provider and runtime similar to above
        let inmemory_provider = InMemoryStateStoreProvider::new();
        let repositories = inmemory_provider.create_repositories();
        let component_factory = create_component_factory();
        let event_handler = Arc::new(NoOpEventHandler);
        
        let runtime = RuntimeInterface::create_with_repositories(
            repositories,
            component_factory,
            event_handler,
        ).await?;
        
        // For this test we don't need to create a full flow
        // Just demonstrate that the timer functionality works
        let (_, _, _, timer_repo, _) = inmemory_provider.create_repositories();
        
        // Schedule a timer for 100ms
        let flow_id = FlowInstanceId("test_flow_instance".to_string());
        let step_id = cascade_core::domain::flow_instance::StepId("test_step".to_string());
        let timer_id = timer_repo.schedule(
            &flow_id, 
            &step_id, 
            std::time::Duration::from_millis(100)
        ).await?;
        
        assert!(!timer_id.is_empty());
        
        // Wait for the timer to fire
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        
        // In a real application, the timer would have triggered and the flow would resume
        
        Ok(())
    }
} 
//! Examples of using the PostgreSQL state store with Cascade Core
//!
//! This module contains examples showing how to integrate the PostgreSQL
//! state store with the Cascade Core Runtime.

#[cfg(test)]
mod examples {
    use cascade_core::{
        RuntimeInterface,
        application::flow_execution_service::DomainEventHandler,
        domain::events::DomainEvent,
        CoreError,
        ComponentExecutor,
        DataPacket,
        ComponentRuntimeApi,
        ExecutionResult,
    };
    use crate::PostgresStateStoreProvider;
    use std::sync::Arc;
    use async_trait::async_trait;

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

    /// Example of creating and using the Cascade Core Runtime with PostgreSQL state store
    #[tokio::test]
    #[ignore] // Ignored by default as it requires a running PostgreSQL instance
    async fn example_postgres_state_store() -> Result<(), CoreError> {
        // Set up tracing (optional)
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .init();

        // Create the PostgreSQL state store provider
        let postgres_provider = PostgresStateStoreProvider::new("postgres://user:password@localhost/cascade")
            .await?;

        // Get the repositories
        let repositories = postgres_provider.create_repositories().await?;

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

        // The runtime is now ready to use with PostgreSQL as the state store
        // For example, you can deploy flow definitions:
        // runtime.deploy_definition(my_flow_definition).await?;
        
        // Or trigger flows:
        // let flow_instance_id = runtime.trigger_flow(
        //     FlowId("my_flow".to_string()), 
        //     DataPacket::new(serde_json::json!({"value": "hello"}))
        // ).await?;

        Ok(())
    }
} 
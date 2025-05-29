use crate::{
    domain::flow_definition::FlowDefinition,
    domain::flow_instance::{CorrelationId, FlowId, FlowInstanceId, FlowStatus, StepId},
    domain::repository::{
        ComponentStateRepository, FlowDefinitionRepository, FlowInstanceRepository, TimerRepository,
    },
    domain::step::DefaultConditionEvaluator,
    ComponentExecutor, CoreError, DataPacket,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::application::flow_definition_service::FlowDefinitionService;
use crate::application::flow_execution_service::FlowExecutionService;
use crate::application::flow_execution_service::DomainEventHandler;
use tracing;

/// Summary information about a flow instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowInstanceSummary {
    /// Flow instance ID
    pub id: String,

    /// Flow ID
    pub flow_id: String,

    /// Current status
    pub status: String,

    /// Creation timestamp
    pub created_at: String,

    /// Last updated timestamp
    pub updated_at: String,
}

/// The main API provided by Cascade Core to external systems
#[derive(Clone)]
pub struct RuntimeInterface {
    flow_execution_service: Arc<FlowExecutionService>,
    flow_definition_service: Arc<FlowDefinitionService>,
}

impl RuntimeInterface {
    /// Create a new runtime interface
    pub fn new(
        flow_execution_service: Arc<FlowExecutionService>,
        flow_definition_service: Arc<FlowDefinitionService>,
    ) -> Self {
        Self {
            flow_execution_service,
            flow_definition_service,
        }
    }

    /// Deploy a flow definition
    pub async fn deploy_definition(&self, definition: FlowDefinition) -> Result<(), CoreError> {
        // Validate the definition
        definition.validate()?;

        // Deploy the definition
        self.flow_definition_service
            .deploy_definition(definition)
            .await
    }

    /// Undeploy a flow definition
    pub async fn undeploy_definition(&self, flow_id: FlowId) -> Result<(), CoreError> {
        self.flow_definition_service
            .undeploy_definition(flow_id)
            .await
    }

    /// Trigger a flow with data
    pub async fn trigger_flow(
        &self,
        flow_id: FlowId,
        trigger_data: DataPacket,
    ) -> Result<FlowInstanceId, CoreError> {
        self.flow_execution_service
            .start_flow(flow_id, trigger_data)
            .await
    }

    /// Resume a flow with an external event
    pub async fn resume_flow_with_event(
        &self,
        correlation_id: CorrelationId,
        event_data: DataPacket,
    ) -> Result<(), CoreError> {
        self.flow_execution_service
            .resume_flow_with_event(&correlation_id, event_data)
            .await
    }

    /// Resume a flow from a timer
    pub async fn resume_flow_from_timer(
        &self,
        flow_instance_id: FlowInstanceId,
    ) -> Result<(), CoreError> {
        let timer_step_id = StepId(flow_instance_id.0.clone());
        self.flow_execution_service
            .resume_flow_from_timer(&flow_instance_id, &timer_step_id)
            .await
    }

    /// Get the state of a flow instance
    pub async fn get_flow_instance_state(
        &self,
        id: FlowInstanceId,
    ) -> Result<Option<serde_json::Value>, CoreError> {
        self.flow_definition_service
            .get_flow_instance_state(&id)
            .await
    }

    /// List all deployed flow definitions
    pub async fn list_definitions(&self) -> Result<Vec<FlowId>, CoreError> {
        self.flow_definition_service.list_definitions().await
    }

    /// List flow instances with optional filters
    pub async fn list_instances(
        &self,
        flow_id: Option<FlowId>,
        status: Option<FlowStatus>,
    ) -> Result<Vec<FlowInstanceSummary>, CoreError> {
        self.flow_definition_service
            .list_instances(flow_id, status)
            .await
    }

    /// Create a new RuntimeInterface with externally-provided repositories
    ///
    /// This is the preferred way to create a RuntimeInterface. It allows
    /// using custom repository implementations from external crates without
    /// coupling the core to specific infrastructure.
    ///
    /// # Arguments
    ///
    /// * `repositories` - A tuple containing the repositories and an optional timer callback
    /// * `component_factory` - A factory function for creating component executors
    /// * `event_handler` - A handler for domain events
    pub async fn create_with_repositories(
        repositories: RepositoriesTuple,
        component_factory: Arc<ComponentFactoryFn>,
        event_handler: Arc<dyn DomainEventHandler>,
    ) -> Result<Self, CoreError> {
        let (
            flow_instance_repo,
            flow_definition_repo,
            component_state_repo,
            timer_repo,
            timer_callback,
        ) = repositories;

        // Create condition evaluator
        let condition_evaluator = Arc::new(DefaultConditionEvaluator);

        // Create services
        let flow_execution_service = Arc::new(FlowExecutionService::new(
            flow_instance_repo.clone(),
            flow_definition_repo.clone(),
            component_state_repo.clone(),
            timer_repo.clone(),
            component_factory,
            condition_evaluator,
            event_handler,
        ));

        let flow_definition_service = Arc::new(FlowDefinitionService::new(
            flow_definition_repo.clone(),
            flow_instance_repo.clone(),
        ));

        // Create interface
        let interface = Self::new(flow_execution_service.clone(), flow_definition_service);

        // Set up timer processing if provided
        if let Some(_callback) = timer_callback {
            // Check if the timer repository implements our timer processing trait
            // This is a safer approach that doesn't rely on downcasting
            if let Some(processing_repo) = timer_repo_as_processing(&timer_repo) {
                let exec_service = flow_execution_service.clone();
                processing_repo.start_timer_processing(Arc::new(
                    move |flow_instance_id, step_id| {
                        let service = exec_service.clone();
                        Box::pin(async move {
                            if let Err(e) = service
                                .resume_flow_from_timer(&flow_instance_id, &step_id)
                                .await
                            {
                                tracing::error!("Error resuming flow from timer: {}", e);
                            }
                        })
                    },
                ));
            }
        }

        Ok(interface)
    }
}

/// Extension trait for timer repositories that support active timer processing
pub trait TimerProcessingRepository: TimerRepository + std::any::Any {
    /// Start processing timers
    fn start_timer_processing(
        &self,
        callback: Arc<TimerCallbackFn>,
    );
}

// Helper function to check if a TimerRepository also implements TimerProcessingRepository
fn timer_repo_as_processing(
    _repo: &Arc<dyn TimerRepository>,
) -> Option<&dyn TimerProcessingRepository> {
    // This is a limitation of Rust's type system - we can't directly cast from one trait to another
    // For now, we'll simply return None to indicate this feature isn't available
    // In an actual implementation, you would need a specific concrete type that implements both traits
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::flow_definition_service::FlowDefinitionService,
        application::flow_execution_service::{
            ComponentFactory, DomainEventHandler, FlowExecutionService,
        },
        domain::flow_definition::{DataReference, StepDefinition, TriggerDefinition},
        domain::repository::memory::{
            MemoryComponentStateRepository, MemoryFlowDefinitionRepository,
            MemoryFlowInstanceRepository, MemoryTimerRepository,
        },
        domain::repository::FlowInstanceRepository,
        domain::step::DefaultConditionEvaluator,
        ComponentExecutor, ComponentExecutorBase, ComponentRuntimeApi, ExecutionResult,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::HashMap;
    use crate::domain::events::DomainEvent;

    struct MockEventHandler;

    #[async_trait]
    impl DomainEventHandler for MockEventHandler {
        async fn handle_event(&self, _event: Box<dyn DomainEvent>) -> Result<(), CoreError> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct EchoComponent;

    impl ComponentExecutorBase for EchoComponent {
        fn component_type(&self) -> &str {
            "Echo"
        }
    }

    #[async_trait]
    impl ComponentExecutor for EchoComponent {
        async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
            // Echo input to output
            let input = match api.get_input("input").await {
                Ok(input) => input,
                Err(_) => DataPacket::null(),
            };

            let _ = api.set_output("output", input).await;
            ExecutionResult::Success
        }
    }

    fn create_test_component_factory() -> ComponentFactory {
        Arc::new(|_: &str| -> Result<Arc<dyn ComponentExecutor>, CoreError> {
            Ok(Arc::new(EchoComponent))
        })
    }

    fn create_test_interface() -> (RuntimeInterface, Arc<MemoryFlowInstanceRepository>) {
        // Create repositories
        let flow_instance_repo = Arc::new(MemoryFlowInstanceRepository::new());
        let flow_definition_repo = Arc::new(MemoryFlowDefinitionRepository::new());
        let component_state_repo = Arc::new(MemoryComponentStateRepository::new());
        let (timer_repo, _timer_rx) = MemoryTimerRepository::new();
        let timer_repo: Arc<dyn TimerRepository> = Arc::new(timer_repo);

        // Create condition evaluator
        let condition_evaluator = Arc::new(DefaultConditionEvaluator);

        // Create component factory
        let component_factory = create_test_component_factory();

        // Create event handler
        let event_handler = Arc::new(MockEventHandler);

        // Create services
        let flow_execution_service = Arc::new(FlowExecutionService::new(
            flow_instance_repo.clone(),
            flow_definition_repo.clone(),
            component_state_repo.clone(),
            timer_repo.clone(),
            component_factory,
            condition_evaluator,
            event_handler,
        ));

        let flow_definition_service = Arc::new(FlowDefinitionService::new(
            flow_definition_repo.clone(),
            flow_instance_repo.clone(),
        ));

        // Create interface
        let interface = RuntimeInterface::new(flow_execution_service, flow_definition_service);

        (interface, flow_instance_repo)
    }

    fn create_test_flow_definition() -> FlowDefinition {
        let flow_id = FlowId("test_flow".to_string());
        FlowDefinition {
            id: flow_id.clone(),
            version: "1.0.0".to_string(),
            name: "Test Flow".to_string(),
            description: Some("A test flow".to_string()),
            steps: vec![StepDefinition {
                id: "echo_step".to_string(),
                component_type: "Echo".to_string(),
                input_mapping: {
                    let mut map = HashMap::new();
                    map.insert(
                        "input".to_string(),
                        DataReference {
                            expression: "$flow.trigger_data".to_string(),
                        },
                    );
                    map
                },
                config: json!({}),
                run_after: vec![],
                condition: None,
            }],
            trigger: Some(TriggerDefinition {
                trigger_type: "http".to_string(),
                config: json!({"method": "POST", "path": "/trigger"}),
            }),
        }
    }

    #[tokio::test]
    async fn test_deploy_and_trigger_flow() {
        let (interface, flow_instance_repo) = create_test_interface();
        let flow_def = create_test_flow_definition();
        let flow_id = flow_def.id.clone();

        // Deploy flow
        interface.deploy_definition(flow_def).await.unwrap();

        // List flows
        let flows = interface.list_definitions().await.unwrap();
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].0, flow_id.0);

        // Trigger flow
        let trigger_data = DataPacket::new(json!({"message": "Hello, World!"}));
        let instance_id = interface
            .trigger_flow(flow_id.clone(), trigger_data)
            .await
            .unwrap();

        // Wait for flow to complete
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Get flow instance
        let instance = flow_instance_repo
            .find_by_id(&instance_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(instance.status, FlowStatus::Completed);

        // Check output
        let output_key = crate::domain::flow_instance::StepId("echo_step.output".to_string());
        assert!(instance.step_outputs.contains_key(&output_key));

        let output = instance.step_outputs.get(&output_key).unwrap();
        assert_eq!(output.as_value()["message"], "Hello, World!");

        // List instances
        let instances = interface.list_instances(Some(flow_id), None).await.unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].id, instance_id.0);
        assert_eq!(instances[0].status, "Completed");
    }
}

/// Function type for timer callbacks that are called when a timer expires
/// This callback takes a flow instance ID and step ID, and returns a future that resolves to ()
pub type TimerCallbackFn = dyn Fn(FlowInstanceId, StepId) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
    + Send
    + Sync;

type ComponentFactoryFn = dyn Fn(&str) -> Result<Arc<dyn ComponentExecutor>, CoreError> + Send + Sync;

type RepositoriesTuple = (
    Arc<dyn FlowInstanceRepository>,
    Arc<dyn FlowDefinitionRepository>,
    Arc<dyn ComponentStateRepository>,
    Arc<dyn TimerRepository>,
    Option<Box<TimerCallbackFn>>,
);

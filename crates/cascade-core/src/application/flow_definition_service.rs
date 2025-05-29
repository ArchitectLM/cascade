use crate::{
    application::runtime_interface::FlowInstanceSummary,
    domain::flow_definition::FlowDefinition,
    domain::flow_instance::{FlowId, FlowInstanceId, FlowStatus},
    domain::repository::{FlowDefinitionRepository, FlowInstanceRepository},
    CoreError,
};
use std::sync::Arc;

/// Service for managing flow definitions
pub struct FlowDefinitionService {
    /// Repository for flow definitions
    flow_definition_repo: Arc<dyn FlowDefinitionRepository>,

    /// Repository for flow instances
    flow_instance_repo: Arc<dyn FlowInstanceRepository>,
}

impl FlowDefinitionService {
    /// Create a new flow definition service
    pub fn new(
        flow_definition_repo: Arc<dyn FlowDefinitionRepository>,
        flow_instance_repo: Arc<dyn FlowInstanceRepository>,
    ) -> Self {
        Self {
            flow_definition_repo,
            flow_instance_repo,
        }
    }

    /// Deploy a flow definition
    pub async fn deploy_definition(&self, definition: FlowDefinition) -> Result<(), CoreError> {
        // Save definition to repository
        self.flow_definition_repo.save(&definition).await?;

        // If there's a trigger, we would register it here
        if let Some(trigger) = &definition.trigger {
            tracing::info!(
                flow_id = %definition.id.0,
                trigger_type = %trigger.trigger_type,
                "Flow deployed with trigger"
            );
            // In a real implementation, we would register the trigger with a trigger manager
        } else {
            tracing::info!(
                flow_id = %definition.id.0,
                "Flow deployed without trigger"
            );
        }

        Ok(())
    }

    /// Undeploy a flow definition
    pub async fn undeploy_definition(&self, flow_id: FlowId) -> Result<(), CoreError> {
        // Check if there are running instances
        let instances = self.flow_instance_repo.find_all_for_flow(&flow_id).await?;

        // Clean up instances (in production you might want to allow completing running flows)
        for instance_id in &instances {
            self.flow_instance_repo.delete(instance_id).await?;
        }

        // Delete definition
        self.flow_definition_repo.delete(&flow_id).await?;

        tracing::info!(
            flow_id = %flow_id.0,
            instances_deleted = instances.len(),
            "Flow undeployed"
        );

        Ok(())
    }

    /// Get the current state of a flow instance
    pub async fn get_flow_instance_state(
        &self,
        id: &FlowInstanceId,
    ) -> Result<Option<serde_json::Value>, CoreError> {
        let instance = self.flow_instance_repo.find_by_id(id).await?;

        match instance {
            Some(instance) => Ok(Some(serde_json::to_value(instance)?)),
            None => Ok(None),
        }
    }

    /// List all deployed flow definitions
    pub async fn list_definitions(&self) -> Result<Vec<FlowId>, CoreError> {
        let definitions = self.flow_definition_repo.find_all().await?;
        Ok(definitions.into_iter().map(|def| def.id).collect())
    }

    /// List flow instances with optional filters
    pub async fn list_instances(
        &self,
        flow_id: Option<FlowId>,
        status: Option<FlowStatus>,
    ) -> Result<Vec<FlowInstanceSummary>, CoreError> {
        // If flow_id is provided, get instances for that flow
        let mut result = Vec::new();

        if let Some(flow_id) = flow_id {
            let instance_ids = self.flow_instance_repo.find_all_for_flow(&flow_id).await?;

            for id in instance_ids {
                if let Some(instance) = self.flow_instance_repo.find_by_id(&id).await? {
                    // Apply status filter
                    if let Some(filter_status) = status {
                        if instance.status != filter_status {
                            continue;
                        }
                    }

                    result.push(FlowInstanceSummary {
                        id: instance.id.0,
                        flow_id: instance.flow_id.0,
                        status: format!("{:?}", instance.status),
                        created_at: instance.created_at.to_rfc3339(),
                        updated_at: instance.updated_at.to_rfc3339(),
                    });
                }
            }
        } else {
            // In a real implementation, we would need a way to list all instances
            // For now, we'll return an empty list
            tracing::warn!("Listing all instances not implemented - need flow_id filter");
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::flow_definition::{DataReference, StepDefinition, TriggerDefinition},
        domain::repository::memory::{
            MemoryFlowDefinitionRepository, MemoryFlowInstanceRepository,
        },
    };
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_flow_definition() -> FlowDefinition {
        let flow_id = FlowId("test_flow".to_string());
        FlowDefinition {
            id: flow_id.clone(),
            version: "1.0.0".to_string(),
            name: "Test Flow".to_string(),
            description: Some("A test flow".to_string()),
            steps: vec![StepDefinition {
                id: "step1".to_string(),
                component_type: "TestComponent".to_string(),
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
    async fn test_deploy_undeploy_flow_definition() {
        // Create repositories
        let flow_definition_repo = Arc::new(MemoryFlowDefinitionRepository::new());
        let flow_instance_repo = Arc::new(MemoryFlowInstanceRepository::new());

        // Create service
        let service =
            FlowDefinitionService::new(flow_definition_repo.clone(), flow_instance_repo.clone());

        // Deploy flow
        let flow_def = create_test_flow_definition();
        let flow_id = flow_def.id.clone();

        service.deploy_definition(flow_def.clone()).await.unwrap();

        // Verify flow is deployed
        let definitions = service.list_definitions().await.unwrap();
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].0, flow_id.0);

        // Undeploy flow
        service.undeploy_definition(flow_id.clone()).await.unwrap();

        // Verify flow is undeployed
        let definitions = service.list_definitions().await.unwrap();
        assert_eq!(definitions.len(), 0);
    }

    #[tokio::test]
    async fn test_list_instances() {
        // Create repositories
        let flow_definition_repo = Arc::new(MemoryFlowDefinitionRepository::new());
        let flow_instance_repo = Arc::new(MemoryFlowInstanceRepository::new());

        // Create service
        let service =
            FlowDefinitionService::new(flow_definition_repo.clone(), flow_instance_repo.clone());

        // Create a test flow instance
        let flow_id = FlowId("test_flow".to_string());
        let instance = crate::domain::flow_instance::FlowInstance::new(
            flow_id.clone(),
            crate::DataPacket::new(json!({"test": "data"})),
        );

        // Save instance
        flow_instance_repo.save(&instance).await.unwrap();

        // List instances
        let instances = service
            .list_instances(Some(flow_id.clone()), None)
            .await
            .unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].id, instance.id.0);
        assert_eq!(instances[0].flow_id, flow_id.0);

        // Filter by status
        let instances = service
            .list_instances(Some(flow_id.clone()), Some(FlowStatus::Initializing))
            .await
            .unwrap();
        assert_eq!(instances.len(), 1); // Should match Initializing

        let instances = service
            .list_instances(Some(flow_id.clone()), Some(FlowStatus::Completed))
            .await
            .unwrap();
        assert_eq!(instances.len(), 0); // No completed instances
    }
}

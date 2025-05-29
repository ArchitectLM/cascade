//! Mappings for application state to graph data

use serde_json::json;
use tracing::{debug, info};
use chrono::Utc;
use uuid::Uuid;

use crate::data::{CoreError, TenantId, DataPacket, ApplicationStateSnapshot, FlowInstanceConfig, StepInstanceConfig};
use super::GraphPatch;

/// Maps application state data to a GraphPatch representation
pub fn map_application_state(
    snapshot_data: &DataPacket,
    tenant_id: &TenantId,
) -> Result<GraphPatch, CoreError> {
    debug!("Mapping application state data to graph patch");
    
    let mut patch = GraphPatch::new();
    
    // Convert snapshot data to an ApplicationStateSnapshot
    let snapshot: ApplicationStateSnapshot = match snapshot_data {
        DataPacket::Json(value) => {
            serde_json::from_value(value.clone())
                .map_err(|e| CoreError::SerializationError(e))?
        },
        _ => return Err(CoreError::ValidationError("Expected JSON data for application state".to_string())),
    };
    
    info!("Mapping application snapshot with id: {}", snapshot.snapshot_id);
    
    // Create node for the ApplicationStateSnapshot
    let snapshot_id = Uuid::new_v4();
    let snapshot_node = json!({
        "id": snapshot_id,
        "label": "ApplicationStateSnapshot",
        "properties": {
            "snapshot_id": snapshot.snapshot_id,
            "timestamp": snapshot.timestamp.to_rfc3339(),
            "source": snapshot.source,
            "tenant_id": tenant_id.to_string(),
            "scope": format!("{:?}", snapshot.scope),
            "created_at": Utc::now().to_rfc3339(),
        }
    });
    patch.nodes.push(snapshot_node);
    
    // Process flow instances if they exist
    if let Some(flow_instances) = &snapshot.flow_instances {
        for flow_instance in flow_instances {
            // Create node for FlowInstanceConfig
            let flow_instance_id = Uuid::new_v4();
            let flow_instance_node = json!({
                "id": flow_instance_id,
                "label": "FlowInstanceConfig",
                "properties": {
                    "instance_id": flow_instance.instance_id,
                    "config_source": flow_instance.config_source.clone().unwrap_or_default(),
                    "snapshot_id": flow_instance.snapshot_id,
                    "flow_def_version_id": flow_instance.flow_def_version_id.to_string(),
                    "tenant_id": flow_instance.tenant_id.to_string(),
                    "created_at": Utc::now().to_rfc3339(),
                }
            });
            patch.nodes.push(flow_instance_node);
            
            // Create edge from snapshot to flow instance
            let snapshot_to_flow_edge = json!({
                "source_id": snapshot_id,
                "target_id": flow_instance_id,
                "type": "CONTAINS_FLOW_INSTANCE",
                "properties": {
                    "created_at": Utc::now().to_rfc3339(),
                }
            });
            patch.edges.push(snapshot_to_flow_edge);
            
            // Process step instances if they exist
            if let Some(step_instances) = &flow_instance.step_instances {
                for step_instance in step_instances {
                    // Create node for StepInstanceConfig
                    let step_instance_id = Uuid::new_v4();
                    
                    // Convert config to JSON string to store as property
                    let config_json = serde_json::to_string(&step_instance.config).unwrap_or_default();
                    
                    let step_instance_node = json!({
                        "id": step_instance_id,
                        "label": "StepInstanceConfig",
                        "properties": {
                            "instance_step_id": step_instance.instance_step_id,
                            "flow_instance_id": step_instance.flow_instance_id,
                            "step_def_id": step_instance.step_def_id.to_string(),
                            "component_def_version_id": step_instance.component_def_version_id.to_string(),
                            "config": config_json,
                            "tenant_id": step_instance.tenant_id.to_string(),
                            "created_at": Utc::now().to_rfc3339(),
                        }
                    });
                    patch.nodes.push(step_instance_node);
                    
                    // Create edge from flow instance to step instance
                    let flow_to_step_edge = json!({
                        "source_id": flow_instance_id,
                        "target_id": step_instance_id,
                        "type": "CONTAINS_STEP_INSTANCE",
                        "properties": {
                            "created_at": Utc::now().to_rfc3339(),
                        }
                    });
                    patch.edges.push(flow_to_step_edge);
                }
            }
        }
    }
    
    debug!("Created graph patch with {} nodes and {} edges", patch.nodes.len(), patch.edges.len());
    Ok(patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Scope, DataPacket};
    use serde_json::json;
    use chrono::Utc;
    use uuid::Uuid;
    use crate::VersionId;

    #[test]
    fn test_map_application_state() {
        // Create test data
        let tenant_id = TenantId::new_v4();
        
        let step_instance = StepInstanceConfig {
            instance_step_id: "test-step-1".to_string(),
            flow_instance_id: "test-flow-1".to_string(),
            step_def_id: Uuid::new_v4(),
            component_def_version_id: VersionId::new_v4(),
            config: DataPacket::Json(json!({
                "key": "value"
            })),
            tenant_id: tenant_id.clone(),
        };

        let flow_instance = FlowInstanceConfig {
            instance_id: "test-flow-1".to_string(),
            config_source: Some("test-config.yaml".to_string()),
            snapshot_id: "test-snapshot-1".to_string(),
            flow_def_version_id: VersionId::new_v4(),
            tenant_id: tenant_id.clone(),
            step_instances: Some(vec![step_instance]),
        };

        let snapshot = ApplicationStateSnapshot {
            snapshot_id: "test-snapshot-1".to_string(),
            timestamp: Utc::now(),
            source: "test-deployment".to_string(),
            tenant_id: tenant_id.clone(),
            scope: Scope::ApplicationState,
            flow_instances: Some(vec![flow_instance]),
        };

        let data_packet = DataPacket::Json(serde_json::to_value(snapshot).unwrap());
        
        // Map the application state
        let result = map_application_state(&data_packet, &tenant_id);
        assert!(result.is_ok(), "Mapping should succeed");
        
        let patch = result.unwrap();
        
        // Verify the patch structure
        assert_eq!(patch.nodes.len(), 3, "Should have 3 nodes: snapshot, flow, and step");
        assert_eq!(patch.edges.len(), 2, "Should have 2 edges: snapshot->flow and flow->step");
        
        // Verify node types
        let node_labels: Vec<&str> = patch.nodes.iter()
            .map(|n| n.as_object().unwrap().get("label").unwrap().as_str().unwrap())
            .collect();

        assert!(node_labels.contains(&"ApplicationStateSnapshot"), "Missing ApplicationStateSnapshot node");
        assert!(node_labels.contains(&"FlowInstanceConfig"), "Missing FlowInstanceConfig node");
        assert!(node_labels.contains(&"StepInstanceConfig"), "Missing StepInstanceConfig node");

        // Verify edge types
        let edge_types: Vec<&str> = patch.edges.iter()
            .map(|e| e.as_object().unwrap().get("type").unwrap().as_str().unwrap())
            .collect();

        assert!(edge_types.contains(&"CONTAINS_FLOW_INSTANCE"), "Missing CONTAINS_FLOW_INSTANCE edge");
        assert!(edge_types.contains(&"CONTAINS_STEP_INSTANCE"), "Missing CONTAINS_STEP_INSTANCE edge");
    }

    #[test]
    fn test_map_application_state_invalid_data() {
        // Test with invalid data type
        let tenant_id = TenantId::new_v4();
        let data_packet = DataPacket::String("not json".to_string());
        
        let result = map_application_state(&data_packet, &tenant_id);
        assert!(result.is_err(), "Mapping should fail with non-JSON data");
        
        // Test with invalid JSON structure
        let data_packet = DataPacket::Json(json!({ "wrong_field": "value" }));
        
        let result = map_application_state(&data_packet, &tenant_id);
        assert!(result.is_err(), "Mapping should fail with invalid JSON structure");
    }
} 
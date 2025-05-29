#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;
    use crate::data::{TenantId, DataPacket};

    #[test]
    fn test_map_snapshot_to_node() {
        let tenant_id = TenantId::new_v4();
        let snapshot = ApplicationStateSnapshot {
            snapshot_id: "test-snapshot-1".to_string(),
            timestamp: Utc::now(),
            source: "test-deployment".to_string(),
            tenant_id: tenant_id.clone(),
        };

        let node = app_state::map_snapshot_to_node(&snapshot, &tenant_id);

        assert_eq!(node.label, "ApplicationStateSnapshot");
        assert_eq!(
            node.properties.get("snapshot_id").unwrap().as_str().unwrap(),
            "test-snapshot-1"
        );
        assert_eq!(
            node.properties.get("source").unwrap().as_str().unwrap(),
            "test-deployment"
        );
        assert_eq!(
            node.properties.get("scope").unwrap().as_str().unwrap(),
            "application_state"
        );
        assert_eq!(
            node.properties.get("tenant_id").unwrap().as_str().unwrap(),
            tenant_id.0.to_string()
        );
    }

    #[test]
    fn test_map_flow_instance_to_node() {
        let tenant_id = TenantId::new_v4();
        let flow_instance = FlowInstanceConfig {
            instance_id: "test-flow-1".to_string(),
            config_source: Some("test-config.yaml".to_string()),
            snapshot_id: "test-snapshot-1".to_string(),
            flow_def_version_id: VersionId::new_v4(),
            tenant_id: tenant_id.clone(),
        };

        let node = app_state::map_flow_instance_to_node(&flow_instance, &tenant_id);

        assert_eq!(node.label, "FlowInstanceConfig");
        assert_eq!(
            node.properties.get("instance_id").unwrap().as_str().unwrap(),
            "test-flow-1"
        );
        assert_eq!(
            node.properties.get("config_source").unwrap().as_str().unwrap(),
            "test-config.yaml"
        );
        assert_eq!(
            node.properties.get("snapshot_id").unwrap().as_str().unwrap(),
            "test-snapshot-1"
        );
        assert_eq!(
            node.properties.get("flow_def_version_id").unwrap().as_str().unwrap(),
            flow_instance.flow_def_version_id.0.to_string()
        );
        assert_eq!(
            node.properties.get("tenant_id").unwrap().as_str().unwrap(),
            tenant_id.0.to_string()
        );
    }

    #[test]
    fn test_map_step_instance_to_node() {
        let tenant_id = TenantId::new_v4();
        let step_instance = StepInstanceConfig {
            instance_step_id: "test-step-1".to_string(),
            flow_instance_id: "test-flow-1".to_string(),
            step_def_id: Uuid::new_v4(),
            component_def_version_id: VersionId::new_v4(),
            config: DataPacket::Json(serde_json::json!({
                "key": "value"
            })),
            tenant_id: tenant_id.clone(),
        };

        let node = app_state::map_step_instance_to_node(&step_instance, &tenant_id);

        assert_eq!(node.label, "StepInstanceConfig");
        assert_eq!(
            node.properties.get("instance_step_id").unwrap().as_str().unwrap(),
            "test-step-1"
        );
        assert_eq!(
            node.properties.get("flow_instance_id").unwrap().as_str().unwrap(),
            "test-flow-1"
        );
        assert_eq!(
            node.properties.get("step_def_id").unwrap().as_str().unwrap(),
            step_instance.step_def_id.to_string()
        );
        assert_eq!(
            node.properties.get("component_def_version_id").unwrap().as_str().unwrap(),
            step_instance.component_def_version_id.0.to_string()
        );
        assert_eq!(
            node.properties.get("tenant_id").unwrap().as_str().unwrap(),
            tenant_id.0.to_string()
        );

        // Check that config was properly serialized
        let config = node.properties.get("config").unwrap();
        assert_eq!(config.as_object().unwrap().get("key").unwrap().as_str().unwrap(), "value");
    }

    #[test]
    fn test_create_instance_edges() {
        let snapshot_id = Uuid::new_v4();
        let flow_instance_id = Uuid::new_v4();
        let step_instance_id = Uuid::new_v4();
        let flow_def_version_id = VersionId::new_v4();
        let step_def_id = Uuid::new_v4();
        let component_def_version_id = VersionId::new_v4();

        let edges = app_state::create_instance_edges(
            snapshot_id,
            flow_instance_id,
            step_instance_id,
            flow_def_version_id,
            step_def_id,
            component_def_version_id,
        );

        // Should create 5 edges
        assert_eq!(edges.len(), 5);

        // Check each edge type exists
        let edge_types: Vec<&str> = edges.iter()
            .map(|e| e.edge_type.as_str())
            .collect();

        assert!(edge_types.contains(&"CONTAINS_FLOW_INSTANCE"));
        assert!(edge_types.contains(&"CONTAINS_STEP_INSTANCE"));
        assert!(edge_types.contains(&"INSTANTIATES_VERSION"));
        assert!(edge_types.contains(&"INSTANTIATES"));
        assert!(edge_types.contains(&"USES_CONFIGURED_COMPONENT_VERSION"));

        // Check specific edge connections
        let contains_flow = edges.iter()
            .find(|e| e.edge_type == "CONTAINS_FLOW_INSTANCE")
            .unwrap();
        assert_eq!(contains_flow.source_id, snapshot_id);
        assert_eq!(contains_flow.target_id, flow_instance_id);

        let contains_step = edges.iter()
            .find(|e| e.edge_type == "CONTAINS_STEP_INSTANCE")
            .unwrap();
        assert_eq!(contains_step.source_id, flow_instance_id);
        assert_eq!(contains_step.target_id, step_instance_id);
    }

    #[test]
    fn test_graph_patch_conversion() {
        let mut patch = GraphPatch::new();

        // Add a test node
        let mut props = HashMap::new();
        props.insert("key".to_string(), JsonValue::String("value".to_string()));
        patch.add_node(NodeData {
            label: "TestNode".to_string(),
            properties: props,
        });

        // Add a test edge
        let mut edge_props = HashMap::new();
        edge_props.insert("weight".to_string(), JsonValue::Number(1.into()));
        patch.add_edge(EdgeData {
            source_id: Uuid::new_v4(),
            target_id: Uuid::new_v4(),
            edge_type: "TEST_EDGE".to_string(),
            properties: edge_props,
        });

        let store_patch = patch.into_store_format();

        assert_eq!(store_patch.nodes.len(), 1);
        assert_eq!(store_patch.edges.len(), 1);

        let node = &store_patch.nodes[0];
        assert_eq!(node.as_object().unwrap().get("label").unwrap().as_str().unwrap(), "TestNode");

        let edge = &store_patch.edges[0];
        assert_eq!(edge.as_object().unwrap().get("type").unwrap().as_str().unwrap(), "TEST_EDGE");
    }
} 
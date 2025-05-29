//! Core entity types for the Cascade Graph Knowledge Base

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::data::types::{Scope, SourceType, VersionStatus, SchemaRef, DataPacket};
use crate::data::identifiers::{
    TenantId, VersionSetId, VersionId, ComponentDefinitionId, FlowDefinitionId,
    DocumentationChunkId, CodeExampleId
};

// TODO: Implement entity structs from Section 2
// Framework, DslSpec, DSLElement, VersionSet, Version, ComponentDefinition,
// InputDefinition, OutputDefinition, FlowDefinition, StepDefinition,
// TriggerDefinition, ConditionDefinition, DocumentationChunk, CodeExample,
// ApplicationStateSnapshot, FlowInstanceConfig, StepInstanceConfig

/// Node: `Framework` (Section 2.1) - Singleton
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Framework {
    pub name: String, // e.g., "Cascade Platform"
}

/// Node: `DSLSpec` (Section 2.1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DslSpec {
    pub version: String,
    pub core_keywords: Vec<String>,
    pub spec_url: Option<String>,
}

/// Node: `DSLElement` (Section 2.1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DslElementType {
    Keyword,
    Structure,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DslElement {
    pub name: String,
    pub element_type: DslElementType,
    pub description: Option<String>,
}

/// Node: `VersionSet` (Section 2.1)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VersionSetEntityType {
    ComponentDefinition,
    FlowDefinition,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionSet {
    pub id: VersionSetId, // Primary key
    pub entity_id: String, // Logical ID, e.g., "StdLib:HttpCall", "process-customer-order"
    pub entity_type: VersionSetEntityType,
}

/// Node: `Version` (Section 2.1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Version {
    pub id: VersionId, // Primary key
    pub version_number: String,
    pub status: VersionStatus,
    pub created_at: DateTime<Utc>,
    pub notes: Option<String>,
    pub version_set_id: VersionSetId, // Link back to VersionSet
}

/// Node: `ComponentDefinition` (Section 2.1) - State at a specific version
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentDefinition {
    pub id: ComponentDefinitionId, // Primary key
    pub name: String, // Unique within this version state, e.g., "StdLib:HttpCall_v2.0"
    pub component_type_id: String, // Logical type ID, e.g., "StdLib:HttpCall" (matches VersionSet entity_id)
    pub source: SourceType,
    pub description: Option<String>,
    pub config_schema_ref: Option<SchemaRef>,
    pub state_schema_ref: Option<SchemaRef>,
    pub scope: Scope,
    pub created_at: DateTime<Utc>,
    pub version_id: VersionId, // Link back to parent Version
}

/// Node: `InputDefinition` (Section 2.1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputDefinition {
    pub name: String,
    pub description: Option<String>,
    pub is_required: bool,
    pub default: Option<DataPacket>,
    pub schema_ref: Option<SchemaRef>,
}

/// Node: `OutputDefinition` (Section 2.1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputDefinition {
    pub name: String,
    pub description: Option<String>,
    pub is_error_path: bool,
    pub schema_ref: Option<SchemaRef>,
}

/// Node: `FlowDefinition` (Section 2.1) - State at a specific version
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowDefinition {
    pub id: FlowDefinitionId, // Primary key
    pub name: String, // Unique within this version state, e.g., "process-order_v1.2"
    pub flow_id: String, // Logical flow ID, e.g., "process-order" (matches VersionSet entity_id)
    pub description: Option<String>,
    pub source: SourceType,
    pub scope: Scope,
    pub created_at: DateTime<Utc>,
    pub version_id: VersionId, // Link back to parent Version
}

/// Node: `StepDefinition` (Section 2.1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepDefinition {
    pub step_id: String, // Unique within its FlowDefinition state
    pub description: Option<String>,
    pub flow_def_id: FlowDefinitionId, // Link back to parent FlowDefinition
}

/// Node: `TriggerDefinition` (Section 2.1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerDefinition {
    pub r#type: String, // Using r#type to avoid keyword collision
    pub config: DataPacket, // Map maps to DataPacket (serde_json::Value)
    pub output_name: String, // default: "trigger"
    pub description: Option<String>,
    pub flow_def_id: FlowDefinitionId, // Link back to parent FlowDefinition
}

/// Node: `ConditionDefinition` (Section 2.1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionDefinition {
    pub r#type: String, // Using r#type to avoid keyword collision
    pub config: DataPacket, // Map maps to DataPacket (serde_json::Value) - contains data references
    pub description: Option<String>,
    pub step_def_id: uuid::Uuid, // Link back to parent StepDefinition (conceptual UUID for graph node)
}

/// Node: `DocumentationChunk` (Section 2.1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentationChunk {
    pub id: DocumentationChunkId, // Primary key
    pub text: String,
    pub source_url: Option<String>,
    pub scope: Scope, // Scope of the entity this docs relates to
    pub embedding: Vec<f32>, // Vector embedding
    pub chunk_seq: Option<i64>, // Integer maps to i64
    pub created_at: DateTime<Utc>,
}

/// Node: `CodeExample` (Section 2.1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeExample {
    pub id: CodeExampleId, // Primary key
    pub code: String, // The code snippet
    pub description: Option<String>,
    pub source: Option<String>, // Source file/context
    pub scope: Scope, // Scope of the entity this example relates to
    pub created_at: DateTime<Utc>,
}

/// ApplicationStateSnapshot represents a snapshot of the application's state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplicationStateSnapshot {
    pub tenant_id: TenantId,
    pub scope: Scope,
    pub snapshot_id: String,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    // Add field for tests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow_instances: Option<Vec<FlowInstanceConfig>>,
}

/// FlowInstanceConfig represents a configured flow instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowInstanceConfig {
    pub instance_id: String,
    pub config_source: Option<String>,
    pub snapshot_id: String,
    pub flow_def_version_id: VersionId,
    pub tenant_id: TenantId,
    // Add field for tests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_instances: Option<Vec<StepInstanceConfig>>,
}

/// Node: `StepInstanceConfig` (Section 2.1) - Optional
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepInstanceConfig {
    pub instance_step_id: String, // Primary key (from overview)
    pub flow_instance_id: String, // Link back to parent FlowInstanceConfig
    pub step_def_id: uuid::Uuid, // Links to the StepDefinition (conceptual UUID for graph node)
    pub component_def_version_id: VersionId, // Links to the specific ComponentDefinition Version
    pub config: DataPacket, // Actual configuration values for this instance
    pub tenant_id: TenantId, // Link to tenant
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn sample_tenant_id() -> TenantId {
        TenantId::new_v4()
    }

    fn sample_datetime() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()
    }

    #[test]
    fn test_framework_serialization() {
        let framework = Framework {
            name: "Cascade Platform".to_string(),
        };
        let json = serde_json::to_string(&framework).unwrap();
        let deserialized: Framework = serde_json::from_str(&json).unwrap();
        assert_eq!(framework, deserialized);
    }

    #[test]
    fn test_dsl_spec_serialization() {
        let spec = DslSpec {
            version: "1.0".to_string(),
            core_keywords: vec!["flow".to_string(), "component".to_string()],
            spec_url: Some("https://example.com/spec".to_string()),
        };
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: DslSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, deserialized);
    }

    #[test]
    fn test_version_set_serialization() {
        let version_set = VersionSet {
            id: VersionSetId::new_v4(),
            entity_id: "StdLib:HttpCall".to_string(),
            entity_type: VersionSetEntityType::ComponentDefinition,
        };
        let json = serde_json::to_string(&version_set).unwrap();
        let deserialized: VersionSet = serde_json::from_str(&json).unwrap();
        assert_eq!(version_set, deserialized);
    }

    #[test]
    fn test_component_definition_serialization() {
        let component = ComponentDefinition {
            id: ComponentDefinitionId::new_v4(),
            name: "StdLib:HttpCall_v2.0".to_string(),
            component_type_id: "StdLib:HttpCall".to_string(),
            source: SourceType::StdLib,
            description: Some("HTTP Client Component".to_string()),
            config_schema_ref: None,
            state_schema_ref: None,
            scope: Scope::General,
            created_at: sample_datetime(),
            version_id: VersionId::new_v4(),
        };
        let json = serde_json::to_string(&component).unwrap();
        let deserialized: ComponentDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(component, deserialized);
    }

    #[test]
    fn test_flow_definition_serialization() {
        let flow = FlowDefinition {
            id: FlowDefinitionId::new_v4(),
            name: "process-order_v1.2".to_string(),
            flow_id: "process-order".to_string(),
            description: Some("Order processing flow".to_string()),
            source: SourceType::UserDefined,
            scope: Scope::Project(sample_tenant_id()),
            created_at: sample_datetime(),
            version_id: VersionId::new_v4(),
        };
        let json = serde_json::to_string(&flow).unwrap();
        let deserialized: FlowDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(flow, deserialized);
    }

    #[test]
    fn test_documentation_chunk_serialization() {
        let chunk = DocumentationChunk {
            id: DocumentationChunkId::new_v4(),
            text: "Example documentation".to_string(),
            source_url: Some("https://example.com/docs".to_string()),
            scope: Scope::General,
            embedding: vec![0.1, 0.2, 0.3],
            chunk_seq: Some(1),
            created_at: sample_datetime(),
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let deserialized: DocumentationChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(chunk, deserialized);
    }

    #[test]
    fn test_trigger_definition_serialization() {
        let trigger = TriggerDefinition {
            r#type: "http".to_string(),
            config: DataPacket::Json(json!({
                "path": "/webhook",
                "method": "POST"
            })),
            output_name: "trigger".to_string(),
            description: Some("HTTP webhook trigger".to_string()),
            flow_def_id: FlowDefinitionId::new_v4(),
        };
        let json = serde_json::to_string(&trigger).unwrap();
        let deserialized: TriggerDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(trigger, deserialized);
    }

    #[test]
    fn test_application_state_snapshot_serialization() {
        let snapshot = ApplicationStateSnapshot {
            snapshot_id: uuid::Uuid::new_v4().to_string(),
            timestamp: sample_datetime(),
            source: "deployment-abc".to_string(),
            scope: Scope::ApplicationState,
            tenant_id: sample_tenant_id(),
            flow_instances: None,
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: ApplicationStateSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snapshot, deserialized);
    }
} 
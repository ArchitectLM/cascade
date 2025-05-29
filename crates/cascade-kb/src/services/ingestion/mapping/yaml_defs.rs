use crate::{
    data::{
        CoreError,
        TenantId,
        Scope,
        SourceType,
        VersionStatus,
    },
    VersionId,
    VersionSetId,
    traits::{ParsedDslDefinitions, GraphDataPatch},
};

use serde_json::json;
use chrono::Utc;
use tracing::debug;

/// Transforms parsed DSL definitions into a GraphDataPatch representation
pub fn transform_to_graph_patch(
    defs: &ParsedDslDefinitions,
    tenant_id: &TenantId,
    scope: Scope,
) -> Result<GraphDataPatch, CoreError> {
    let mut patch = GraphDataPatch {
        nodes: Vec::new(),
        edges: Vec::new(),
    };
    
    // Process ComponentDefinitions
    for component in &defs.components {
        // Create VersionSet for the component if not existing
        let version_set_id = VersionSetId::new_v4();
        let version_id = VersionId::new_v4();
        
        // Add VersionSet node
        patch.nodes.push(json!({
            "label": "VersionSet",
            "id": version_set_id.0.to_string(),
            "entity_id": component.component_type_id.clone(),
            "entity_type": "ComponentDefinition",
            "tenant_id": tenant_id.0.to_string(),
        }));
        
        // Add Version node
        patch.nodes.push(json!({
            "label": "Version",
            "id": version_id.0.to_string(),
            "version_number": "1.0.0", // Default version
            "status": format!("{:?}", VersionStatus::Active),
            "created_at": Utc::now().to_rfc3339(),
            "tenant_id": tenant_id.0.to_string(),
        }));
        
        // Connect VersionSet to Version with LATEST_VERSION relationship
        patch.edges.push(json!({
            "type": "LATEST_VERSION",
            "from": {
                "label": "VersionSet",
                "id": version_set_id.0.to_string(),
            },
            "to": {
                "label": "Version",
                "id": version_id.0.to_string(),
            },
            "properties": {}
        }));
        
        // Add ComponentDefinition node
        patch.nodes.push(json!({
            "label": "ComponentDefinition",
            "id": component.id.0.to_string(),
            "name": component.name.clone(),
            "component_type_id": component.component_type_id.clone(),
            "source": match component.source {
                SourceType::StdLib => "StdLib",
                SourceType::UserDefined => "UserDefined",
                SourceType::ApplicationState => "ApplicationState",
                _ => "Other"
            },
            "description": component.description.clone().unwrap_or_default(),
            "scope": match scope {
                Scope::General => "General",
                Scope::UserDefined => "UserDefined",
                Scope::ApplicationState => "ApplicationState",
                _ => "Other"
            },
            "created_at": Utc::now().to_rfc3339(),
            "tenant_id": tenant_id.0.to_string()
        }));
        
        // Connect Version to ComponentDefinition
        patch.edges.push(json!({
            "type": "REPRESENTS",
            "from": {
                "label": "Version",
                "id": version_id.0.to_string(),
            },
            "to": {
                "label": "ComponentDefinition",
                "id": component.id.0.to_string(),
            },
            "properties": {}
        }));
        
        // Process Input Definitions
        for input in &defs.input_definitions {
            let input_id = uuid::Uuid::new_v4();
            patch.nodes.push(json!({
                "label": "InputDefinition",
                "id": input_id.to_string(),
                "name": input.name.clone(),
                "description": input.description.clone().unwrap_or_default(),
                "is_required": input.is_required,
                "tenant_id": tenant_id.0.to_string()
            }));
            
            // Connect ComponentDefinition to InputDefinition
            patch.edges.push(json!({
                "type": "HAS_INPUT",
                "from": {
                    "label": "ComponentDefinition",
                    "id": component.id.0.to_string(),
                },
                "to": {
                    "label": "InputDefinition",
                    "id": input_id.to_string(),
                },
                "properties": {}
            }));
        }
        
        // Process Output Definitions
        for output in &defs.output_definitions {
            let output_id = uuid::Uuid::new_v4();
            patch.nodes.push(json!({
                "label": "OutputDefinition",
                "id": output_id.to_string(),
                "name": output.name.clone(),
                "description": output.description.clone().unwrap_or_default(),
                "is_error_path": output.is_error_path,
                "tenant_id": tenant_id.0.to_string()
            }));
            
            // Connect ComponentDefinition to OutputDefinition
            patch.edges.push(json!({
                "type": "HAS_OUTPUT",
                "from": {
                    "label": "ComponentDefinition",
                    "id": component.id.0.to_string(),
                },
                "to": {
                    "label": "OutputDefinition",
                    "id": output_id.to_string(),
                },
                "properties": {}
            }));
        }
    }
    
    // Process FlowDefinitions (similar structure to ComponentDefinitions)
    for flow in &defs.flows {
        // Create VersionSet for the flow if not existing
        let version_set_id = VersionSetId::new_v4();
        let version_id = VersionId::new_v4();
        
        // Add VersionSet node
        patch.nodes.push(json!({
            "label": "VersionSet",
            "id": version_set_id.0.to_string(),
            "entity_id": flow.flow_id.clone(),
            "entity_type": "FlowDefinition",
            "tenant_id": tenant_id.0.to_string(),
        }));
        
        // Add Version node
        patch.nodes.push(json!({
            "label": "Version",
            "id": version_id.0.to_string(),
            "version_number": "1.0.0", // Default version
            "status": format!("{:?}", VersionStatus::Active),
            "created_at": Utc::now().to_rfc3339(),
            "tenant_id": tenant_id.0.to_string(),
        }));
        
        // Connect VersionSet to Version with LATEST_VERSION relationship
        patch.edges.push(json!({
            "type": "LATEST_VERSION",
            "from": {
                "label": "VersionSet",
                "id": version_set_id.0.to_string(),
            },
            "to": {
                "label": "Version",
                "id": version_id.0.to_string(),
            },
            "properties": {}
        }));
        
        // Add FlowDefinition node
        patch.nodes.push(json!({
            "label": "FlowDefinition",
            "id": flow.id.0.to_string(),
            "name": flow.name.clone(),
            "flow_id": flow.flow_id.clone(),
            "source": match flow.source {
                SourceType::StdLib => "StdLib",
                SourceType::UserDefined => "UserDefined",
                SourceType::ApplicationState => "ApplicationState",
                _ => "Other"
            },
            "description": flow.description.clone().unwrap_or_default(),
            "scope": match scope {
                Scope::General => "General",
                Scope::UserDefined => "UserDefined",
                Scope::ApplicationState => "ApplicationState",
                _ => "Other"
            },
            "created_at": Utc::now().to_rfc3339(),
            "tenant_id": tenant_id.0.to_string()
        }));
        
        // Connect Version to FlowDefinition
        patch.edges.push(json!({
            "type": "REPRESENTS",
            "from": {
                "label": "Version",
                "id": version_id.0.to_string(),
            },
            "to": {
                "label": "FlowDefinition",
                "id": flow.id.0.to_string(),
            },
            "properties": {}
        }));
        
        // Process Step Definitions
        for step in &defs.step_definitions {
            let step_id = uuid::Uuid::new_v4();
            patch.nodes.push(json!({
                "label": "StepDefinition",
                "id": step_id.to_string(),
                "step_id": step.step_id.clone(),
                "description": step.description.clone().unwrap_or_default(),
                "tenant_id": tenant_id.0.to_string()
            }));
            
            // Connect FlowDefinition to StepDefinition
            patch.edges.push(json!({
                "type": "CONTAINS_STEP",
                "from": {
                    "label": "FlowDefinition",
                    "id": flow.id.0.to_string(),
                },
                "to": {
                    "label": "StepDefinition",
                    "id": step_id.to_string(),
                },
                "properties": {}
            }));
        }
    }
    
    // Process DocumentationChunks
    for doc in &defs.documentation_chunks {
        patch.nodes.push(json!({
            "label": "DocumentationChunk",
            "id": doc.id.0.to_string(),
            "text": doc.text.clone(),
            "source_url": doc.source_url.clone(),
            "scope": match doc.scope {
                Scope::General => "General",
                Scope::UserDefined => "UserDefined",
                Scope::ApplicationState => "ApplicationState",
                _ => "Other"
            },
            "embedding": doc.embedding.clone(),
            "chunk_seq": doc.chunk_seq,
            "created_at": doc.created_at.to_rfc3339(),
            "tenant_id": tenant_id.0.to_string()
        }));
        
        // For each doc, link to the associated entity if available
        // This assumes one doc is linked to one entity (could be extended for multiple links)
        if !defs.components.is_empty() {
            // Link to the first component for simplicity
            let component = &defs.components[0];
            patch.edges.push(json!({
                "type": "HAS_DOCUMENTATION",
                "from": {
                    "label": "ComponentDefinition",
                    "id": component.id.0.to_string(),
                },
                "to": {
                    "label": "DocumentationChunk",
                    "id": doc.id.0.to_string(),
                },
                "properties": {}
            }));
        } else if !defs.flows.is_empty() {
            // Link to the first flow for simplicity
            let flow = &defs.flows[0];
            patch.edges.push(json!({
                "type": "HAS_DOCUMENTATION",
                "from": {
                    "label": "FlowDefinition",
                    "id": flow.id.0.to_string(),
                },
                "to": {
                    "label": "DocumentationChunk",
                    "id": doc.id.0.to_string(),
                },
                "properties": {}
            }));
        }
    }
    
    debug!("Created graph patch with {} nodes and {} edges", 
          patch.nodes.len(), patch.edges.len());
    
    Ok(patch)
} 
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{ConditionDefinition, ComponentReference, InputMapping};

/// Definition of a step in a flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDefinition {
    /// Unique identifier for the step within the flow
    pub step_id: String,
    
    /// Reference to the component that implements this step
    pub component_ref: ComponentReference,
    
    /// Optional human-readable description
    #[serde(default)]
    pub description: Option<String>,
    
    /// Mapping of inputs to this step
    #[serde(default)]
    pub inputs_map: InputMapping,
    
    /// IDs of steps that must complete before this step starts
    #[serde(default)]
    pub run_after: Vec<String>,
    
    /// Optional condition to determine if this step should run
    #[serde(default)]
    pub condition: Option<ConditionDefinition>,
    
    /// Optional metadata for this step (arbitrary key-value pairs)
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
} 
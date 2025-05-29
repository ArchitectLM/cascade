use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Definition of a trigger that starts a flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDefinition {
    /// Type of trigger (e.g., "HttpEndpoint", "Schedule", "MessageQueue")
    #[serde(rename = "type")]
    pub trigger_type: String,
    
    /// Optional human-readable description
    #[serde(default)]
    pub description: Option<String>,
    
    /// Configuration for the trigger
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
    
    /// Optional custom name for the trigger output in the flow context 
    /// (defaults to "trigger" if not specified)
    #[serde(default)]
    pub output_name: Option<String>,
    
    /// Optional metadata for this trigger (arbitrary key-value pairs)
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
} 
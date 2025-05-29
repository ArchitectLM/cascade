use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Defines a condition that must be satisfied for a step to run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionDefinition {
    /// Type of condition (e.g., "StdLib:JsonPath", "StdLib:EqualsValue")
    #[serde(rename = "type")]
    pub condition_type: String,
    
    /// Optional human-readable description
    #[serde(default)]
    pub description: Option<String>,
    
    /// Configuration for the condition
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
    
    /// Optional metadata for this condition (arbitrary key-value pairs)
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
} 
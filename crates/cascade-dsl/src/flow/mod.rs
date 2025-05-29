mod component;
mod condition;
mod step;
mod trigger;

pub use component::{ComponentDefinition, InputDefinition, OutputDefinition, SchemaReference};
pub use condition::ConditionDefinition;
pub use step::StepDefinition;
pub use trigger::TriggerDefinition;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The complete Cascade DSL document.
/// This is the top-level structure that contains all DSL definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDocument {
    /// The DSL version (e.g., "1.0")
    pub dsl_version: String,
    
    /// All definitions in the document
    pub definitions: Definitions,
}

/// Container for all definitions in a DSL document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definitions {
    /// Component definitions
    #[serde(default)]
    pub components: Vec<ComponentDefinition>,
    
    /// Flow definitions
    #[serde(default)]
    pub flows: Vec<FlowDefinition>,
}

/// A complete flow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowDefinition {
    /// Unique name of the flow
    pub name: String,
    
    /// Optional human-readable description
    #[serde(default)]
    pub description: Option<String>,
    
    /// The trigger that starts this flow
    pub trigger: TriggerDefinition,
    
    /// Steps that make up the flow
    pub steps: Vec<StepDefinition>,
    
    /// Optional metadata for this flow (arbitrary key-value pairs)
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// A data reference (e.g., "trigger.body", "steps.step1.outputs.result")
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataReference {
    /// The reference string (e.g., "trigger.body")
    pub reference: String,
}

impl From<String> for DataReference {
    fn from(s: String) -> Self {
        DataReference { reference: s }
    }
}

impl From<&str> for DataReference {
    fn from(s: &str) -> Self {
        DataReference { reference: s.to_string() }
    }
}

impl AsRef<str> for DataReference {
    fn as_ref(&self) -> &str {
        &self.reference
    }
}

/// Input mapping for a step
pub type InputMapping = HashMap<String, String>;

/// Represents a component reference in a flow step
pub type ComponentReference = String; 
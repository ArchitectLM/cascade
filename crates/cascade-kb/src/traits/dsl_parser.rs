//! DslParser trait definition for Cascade YAML parsing

use async_trait::async_trait;
use crate::{
    data::{
        errors::CoreError,
        identifiers::TenantId,
        trace_context::TraceContext,
        entities::{
            ComponentDefinition,
            FlowDefinition,
            InputDefinition,
            OutputDefinition,
            StepDefinition,
            TriggerDefinition,
            ConditionDefinition,
            DocumentationChunk,
            CodeExample,
        },
    },
};
use serde::{Deserialize, Serialize};

/// Represents the parsed output from a DSL definition file.
/// Contains all the entities defined in the YAML, ready for ingestion into the graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedDslDefinitions {
    /// Component definitions parsed from the YAML
    pub components: Vec<ComponentDefinition>,
    
    /// Flow definitions parsed from the YAML
    pub flows: Vec<FlowDefinition>,
    
    /// Input definitions linked to components/flows
    pub input_definitions: Vec<InputDefinition>,
    
    /// Output definitions linked to components/flows
    pub output_definitions: Vec<OutputDefinition>,
    
    /// Step definitions linked to flows
    pub step_definitions: Vec<StepDefinition>,
    
    /// Trigger definitions linked to flows
    pub trigger_definitions: Vec<TriggerDefinition>,
    
    /// Condition definitions linked to steps
    pub condition_definitions: Vec<ConditionDefinition>,
    
    /// Documentation chunks parsed from inline docs
    pub documentation_chunks: Vec<DocumentationChunk>,
    
    /// Code examples parsed from the YAML
    pub code_examples: Vec<CodeExample>,
}

/// Represents the interface for parsing and validating Cascade YAML.
/// Based on Section 3 Ingestion Process ("Use the official Cascade parser/validator").
#[async_trait]
pub trait DslParser: Send + Sync {
    /// Parses and validates a Cascade YAML definition string.
    ///
    /// Contract: Takes raw YAML text, validates it against the DSL specification,
    /// and returns a structured representation of the defined entities (Components, Flows).
    /// Should report validation errors.
    async fn parse_and_validate_yaml(
        &self,
        yaml_content: &str,
        tenant_id: &TenantId,
        trace_ctx: &TraceContext,
    ) -> Result<ParsedDslDefinitions, CoreError>;
} 
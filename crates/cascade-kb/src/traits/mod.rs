//! Core traits (interfaces) for the Cascade Graph Knowledge Base

pub mod state_store;
mod embedding_generator;
mod dsl_parser;
mod runtime_api;

pub use state_store::{StateStore, GraphDataPatch};
pub use embedding_generator::EmbeddingGenerator;
pub use dsl_parser::{DslParser, ParsedDslDefinitions};
pub use runtime_api::{
    ComponentLoader, ComponentExecutor, ComponentRuntimeApi,
    TriggerExecutor, TriggerCallback, ConditionEvaluator,
    ComponentMetadata, Capability, ComponentGrant, LogLevel,
}; 
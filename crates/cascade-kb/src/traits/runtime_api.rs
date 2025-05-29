//! Runtime-related traits for component execution and flow management

use async_trait::async_trait;
use crate::data::{
    errors::{ComponentLoaderError, ComponentApiError, CoreError},
    identifiers::TenantId,
    trace_context::TraceContext,
    types::DataPacket,
    entities::{TriggerDefinition, ConditionDefinition},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

/// Represents a specific capability of a component (e.g., "network_access").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability(pub String);

/// Represents a grant/permission required by a component (e.g., "read_secrets").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComponentGrant(pub String);

/// Represents metadata about a loaded component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentMetadata {
    /// Logical type ID of the component
    pub component_type_id: String,
    
    /// Version of the component
    pub version: String,
    
    /// JSON Schema for component configuration
    pub config_schema: Option<DataPacket>,
    
    /// JSON Schema for component state
    pub state_schema: Option<DataPacket>,
    
    /// List of capabilities provided by the component
    pub capabilities: Vec<Capability>,
    
    /// List of grants/permissions required by the component
    pub grants_required: Vec<ComponentGrant>,
}

/// Represents log levels used by components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Represents the interface for loading component code/plugins.
#[async_trait]
pub trait ComponentLoader: Send + Sync {
    /// Loads component metadata given a component type ID and optional version.
    ///
    /// Contract: Retrieves the necessary information (e.g., configuration schema,
    /// state schema, capabilities) for a component without necessarily loading the
    /// executable code. Could consult the KB or an external registry.
    async fn load_component_metadata(
        &self,
        component_type_id: &str,
        version: Option<&str>,
    ) -> Result<ComponentMetadata, ComponentLoaderError>;

    /// Loads and initializes a component executor instance.
    ///
    /// Contract: Retrieves the executable code for a component, initializes it,
    /// and returns a handle that implements the `ComponentExecutor` trait.
    async fn load_component_executor(
        &self,
        component_type_id: &str,
        version: Option<&str>,
    ) -> Result<Box<dyn ComponentExecutor>, ComponentLoaderError>;
}

/// Represents the contract for executing component logic.
#[async_trait]
pub trait ComponentExecutor: Send + Sync {
    /// Executes the main logic of the component.
    ///
    /// Contract: Takes component configuration, input data, and a runtime API handle.
    /// Performs the component's action. Returns output data or an error.
    async fn execute(
        &self,
        config: DataPacket,
        inputs: DataPacket,
        runtime: Arc<dyn ComponentRuntimeApi>,
    ) -> Result<DataPacket, ComponentApiError>;
}

/// Represents the API provided by the runtime to executing components.
#[async_trait]
pub trait ComponentRuntimeApi: Send + Sync {
    /// Gets a secret value by name.
    ///
    /// Contract: Provides secure access to secrets configured for the flow/component.
    /// Should enforce permission checks based on `ComponentGrant`.
    async fn get_secret(&self, name: &str) -> Result<String, ComponentApiError>;

    /// Sets a timer to resume execution later.
    ///
    /// Contract: Allows a component to pause its execution and be resumed after a specified duration or at a specific time.
    async fn set_timer(&self, duration_secs: u64) -> Result<(), ComponentApiError>;

    /// Emits a log message with associated trace context.
    ///
    /// Contract: Provides a standardized way for components to log information.
    async fn emit_log(&self, level: LogLevel, message: &str) -> Result<(), ComponentApiError>;
}

/// Represents the interface for evaluating conditions.
#[async_trait]
pub trait ConditionEvaluator: Send + Sync {
    /// Evaluates a condition expression.
    ///
    /// Contract: Takes a condition definition (including configuration) and the relevant
    /// data context (e.g., outputs from previous steps, trigger data). Parses the expression
    /// and returns the boolean result of the evaluation.
    async fn evaluate_condition(
        &self,
        condition_def: &ConditionDefinition,
        data_context: &HashMap<String, DataPacket>,
    ) -> Result<bool, CoreError>;
}

/// Represents the interface for triggering flow executions.
#[async_trait]
pub trait TriggerExecutor: Send + Sync {
    /// Initializes and starts listening for trigger events.
    ///
    /// Contract: Configures the trigger based on its definition and starts monitoring
    /// the external source. When an event occurs, it should call the provided callback.
    async fn start_listening(
        &self,
        trigger_def: &TriggerDefinition,
        callback: Arc<dyn TriggerCallback>,
    ) -> Result<(), CoreError>;

    /// Stops listening for trigger events.
    async fn stop_listening(&self, trigger_def: &TriggerDefinition) -> Result<(), CoreError>;
}

/// Callback interface for trigger executors to signal a new flow execution is needed.
#[async_trait]
pub trait TriggerCallback: Send + Sync {
    /// Called by a trigger executor when a new event occurs.
    ///
    /// Contract: Receives the trigger's output data and initiates the flow execution process
    /// within the runtime. Includes tenant and trace context.
    async fn on_trigger(
        &self,
        tenant_id: TenantId,
        trace_ctx: TraceContext,
        trigger_output: DataPacket,
    ) -> Result<(), CoreError>;
} 
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Defines a component that can be used in flows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDefinition {
    /// Unique name of the component (to be referenced by steps)
    pub name: String,
    
    /// Type identifier for the component implementation (e.g., "StdLib:HttpCall")
    #[serde(rename = "type")]
    pub component_type: String,
    
    /// Optional human-readable description
    #[serde(default)]
    pub description: Option<String>,
    
    /// Configuration for the component
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
    
    /// Input ports defined by this component
    #[serde(default)]
    pub inputs: Vec<InputDefinition>,
    
    /// Output ports defined by this component
    #[serde(default)]
    pub outputs: Vec<OutputDefinition>,
    
    /// Optional schema for stateful components
    #[serde(default)]
    pub state_schema: Option<SchemaReference>,
    
    /// Optional shared state scope for this component
    #[serde(default)]
    pub shared_state_scope: Option<String>,
    
    /// Optional resilience pattern configuration
    #[serde(default)]
    pub resilience: Option<ResilienceConfig>,
    
    /// Optional metadata for this component (arbitrary key-value pairs)
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Defines an input port for a component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDefinition {
    /// Name of the input
    pub name: String,
    
    /// Optional human-readable description
    #[serde(default)]
    pub description: Option<String>,
    
    /// Optional schema reference for this input
    #[serde(default)]
    pub schema_ref: Option<SchemaReference>,
    
    /// Whether this input is required (defaults to true)
    #[serde(default = "default_true")]
    pub is_required: bool,
}

/// Defines an output port for a component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDefinition {
    /// Name of the output
    pub name: String,
    
    /// Optional human-readable description
    #[serde(default)]
    pub description: Option<String>,
    
    /// Optional schema reference for this output
    #[serde(default)]
    pub schema_ref: Option<SchemaReference>,
    
    /// Whether this output represents an error path (defaults to false)
    #[serde(default)]
    pub is_error_path: bool,
}

/// Resilience pattern configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilienceConfig {
    /// Enable retry pattern
    #[serde(default)]
    pub retry_enabled: bool,
    
    /// Maximum retry attempts
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
    
    /// Initial delay before first retry (milliseconds)
    #[serde(default = "default_retry_delay")]
    pub retry_delay_ms: u64,
    
    /// Backoff multiplier for retries
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    
    /// Enable circuit breaker pattern
    #[serde(default)]
    pub circuit_breaker_enabled: bool,
    
    /// Number of failures before opening circuit
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    
    /// Time to wait before trying again (milliseconds)
    #[serde(default = "default_reset_timeout")]
    pub reset_timeout_ms: u64,
    
    /// Enable rate limiter pattern
    #[serde(default)]
    pub rate_limiter_enabled: bool,
    
    /// Maximum requests in the window
    #[serde(default = "default_max_requests")]
    pub max_requests: u32,
    
    /// Time window for rate limiting (milliseconds)
    #[serde(default = "default_window_ms")]
    pub window_ms: u64,
}

/// A reference to a schema (can be a string URI or an inline schema object)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SchemaReference {
    /// A URI reference to a schema (e.g., "schema:string")
    Uri(String),
    
    /// An inline JSON Schema
    Inline(serde_json::Value),
}

/// Default value for is_required (true)
fn default_true() -> bool {
    true
}

/// Default value for retry attempts
fn default_retry_attempts() -> u32 {
    3
}

/// Default value for retry delay (milliseconds)
fn default_retry_delay() -> u64 {
    1000
}

/// Default value for backoff multiplier
fn default_backoff_multiplier() -> f64 {
    2.0
}

/// Default value for failure threshold
fn default_failure_threshold() -> u32 {
    5
}

/// Default value for reset timeout (milliseconds)
fn default_reset_timeout() -> u64 {
    30000 // 30 seconds
}

/// Default value for max requests
fn default_max_requests() -> u32 {
    100
}

/// Default value for window (milliseconds)
fn default_window_ms() -> u64 {
    60000 // 1 minute
} 
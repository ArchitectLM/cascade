use cascade_core::{
    ComponentExecutor, 
    ComponentRuntimeApi, 
    ExecutionResult,
    LogLevel,
    ComponentExecutorBase,
};
use cascade_core::types::DataPacket;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

/// UUID Generator component
/// 
/// Generates UUID values in various formats.
#[derive(Debug, Default)]
pub struct UuidGenerator {}

impl UuidGenerator {
    /// Create a new UUID generator component
    pub fn new() -> Self {
        Self {}
    }
}

impl ComponentExecutorBase for UuidGenerator {
    fn component_type(&self) -> &str {
        "UuidGenerator"
    }
}

#[async_trait]
impl ComponentExecutor for UuidGenerator {
    async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
        // Get format preference from config
        let format = match api.get_config("format").await {
            Ok(value) => value.as_str().unwrap_or("standard").to_string(),
            Err(_) => "standard".to_string(),
        };

        // Get version config - defaults to v4
        let version = match api.get_config("version").await {
            Ok(value) => value.as_str().unwrap_or("v4").to_string(),
            Err(_) => "v4".to_string(),
        };

        // Generate UUID (currently only v4 is supported)
        let uuid = match version.as_str() {
            "v1" => {
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Info, 
                    "UUID v1 not supported, falling back to v4").await;
                Uuid::new_v4()
            },
            "v3" => {
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Info, 
                    "UUID v3 not supported, falling back to v4").await;
                Uuid::new_v4()
            },
            "v5" => {
                let _ = ComponentRuntimeApi::log(&*api, LogLevel::Info, 
                    "UUID v5 not supported in this version, falling back to v4").await;
                Uuid::new_v4()
            },
            _ => Uuid::new_v4(), // Default to v4
        };

        // Format UUID based on preference
        let uuid_string = match format.as_str() {
            "simple" => uuid.as_simple().to_string(),
            "hyphenated" => uuid.as_hyphenated().to_string(),
            "urn" => uuid.as_urn().to_string(),
            "braced" => format!("{{{}}}", uuid.as_hyphenated()),
            _ => uuid.to_string(), // Default to standard hyphenated format
        };

        // Emit metric
        let _ = api.emit_metric("uuid_generated", 1.0, Default::default()).await;

        // Set the UUID as output
        let output = DataPacket::new(json!({
            "uuid": uuid_string,
            "version": version,
            "format": format
        }));

        if let Err(e) = api.set_output("uuid", output.clone()).await {
            return ExecutionResult::Failure(e);
        }

        // Also set as result for convenience
        if let Err(e) = api.set_output("result", output).await {
            return ExecutionResult::Failure(e);
        }

        ExecutionResult::Success
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Test implementations will be added here
} 
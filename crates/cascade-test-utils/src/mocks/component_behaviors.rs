//! Reusable component behavior functions for mocks.
//!
//! This module provides reusable functions for setting up common component
//! behaviors in tests, such as HTTP responses, database operations, etc.

use crate::mocks::component::{ComponentError, ComponentRuntimeAPI, DataPacket, LogLevel};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Data for HTTP simulation
#[derive(Debug, Clone)]
pub struct HttpResponseData {
    pub status_code: u16,
    pub body: serde_json::Value,
    pub headers: serde_json::Value,
}

impl HttpResponseData {
    pub fn new(status_code: u16, body: serde_json::Value) -> Self {
        Self {
            status_code,
            body,
            headers: json!({}),
        }
    }

    pub fn with_headers(mut self, headers: serde_json::Value) -> Self {
        self.headers = headers;
        self
    }
}

/// Simulates a successful HTTP response with the given data.
///
/// # Arguments
///
/// * `api` - The ComponentRuntimeAPI to interact with
/// * `data` - The HTTP response data
///
/// # Example
///
/// ```
/// use cascade_test_utils::mocks::component_behaviors::*;
/// use cascade_test_utils::mocks::component::*;
/// use serde_json::json;
/// use std::sync::Arc;
///
/// async fn test_http_behavior() {
///     let mock_runtime = Arc::new(create_mock_component_runtime_api("http-component"));
///     let response_data = HttpResponseData::new(200, json!({"success": true}));
///     
///     // Simulate HTTP success response
///     let result = simulate_http_response(mock_runtime, response_data).await;
///     assert!(result.is_ok());
/// }
/// ```
pub async fn simulate_http_response(
    api: Arc<dyn ComponentRuntimeAPI>,
    data: HttpResponseData,
) -> Result<(), ComponentError> {
    // Log the request
    api.log(
        LogLevel::Info,
        &format!(
            "Simulating HTTP response with status {} for {}",
            data.status_code,
            api.name()
        ),
    )
    .await?;

    // Set the response outputs
    api.set_output(
        "statusCode",
        DataPacket::new(json!(data.status_code)),
    )
    .await?;

    api.set_output("body", DataPacket::new(data.body)).await?;
    api.set_output("headers", DataPacket::new(data.headers)).await?;

    // For status codes >= 400, set error output too
    if data.status_code >= 400 {
        api.set_output(
            "error",
            DataPacket::new(json!({
                "message": format!("HTTP Error: {}", data.status_code),
                "statusCode": data.status_code
            })),
        )
        .await?;
    }

    Ok(())
}

/// Simulates an HTTP request with configurable latency.
///
/// # Arguments
///
/// * `api` - The ComponentRuntimeAPI to interact with
/// * `data` - The HTTP response data
/// * `latency_ms` - The simulated latency in milliseconds
pub async fn simulate_http_response_with_latency(
    api: Arc<dyn ComponentRuntimeAPI>,
    data: HttpResponseData,
    latency_ms: u64,
) -> Result<(), ComponentError> {
    api.log(
        LogLevel::Debug,
        &format!("Simulating latency of {}ms", latency_ms),
    )
    .await?;

    // Simulate network latency
    sleep(Duration::from_millis(latency_ms)).await;

    simulate_http_response(api, data).await
}

/// Simulates a failing HTTP request with network error.
///
/// # Arguments
///
/// * `api` - The ComponentRuntimeAPI to interact with
/// * `error_message` - The error message to return
pub async fn simulate_http_network_error(
    api: Arc<dyn ComponentRuntimeAPI>,
    error_message: &str,
) -> Result<(), ComponentError> {
    api.log(
        LogLevel::Error,
        &format!("Simulating network error: {}", error_message),
    )
    .await?;

    api.set_output(
        "error",
        DataPacket::new(json!({
            "message": error_message,
            "type": "NetworkError"
        })),
    )
    .await?;

    Ok(())
}

/// Data for database simulation
#[derive(Debug, Clone)]
pub struct DbOperationData {
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub affected_rows: Option<i64>,
}

impl DbOperationData {
    pub fn success(result: serde_json::Value) -> Self {
        Self {
            success: true,
            result: Some(result),
            error: None,
            affected_rows: None,
        }
    }

    pub fn failure(error: &str) -> Self {
        Self {
            success: false,
            result: None,
            error: Some(error.to_string()),
            affected_rows: None,
        }
    }

    pub fn with_affected_rows(mut self, rows: i64) -> Self {
        self.affected_rows = Some(rows);
        self
    }
}

/// Simulates a database operation with the given data.
///
/// # Arguments
///
/// * `api` - The ComponentRuntimeAPI to interact with
/// * `data` - The database operation data
pub async fn simulate_db_operation(
    api: Arc<dyn ComponentRuntimeAPI>,
    data: DbOperationData,
) -> Result<(), ComponentError> {
    api.log(
        if data.success { LogLevel::Info } else { LogLevel::Error },
        &format!(
            "Simulating DB operation for {}: {}",
            api.name(),
            if data.success { "success" } else { "failure" }
        ),
    )
    .await?;

    // Set the result/error outputs
    if data.success {
        if let Some(result) = data.result {
            api.set_output("result", DataPacket::new(result)).await?;
        }

        if let Some(rows) = data.affected_rows {
            api.set_output(
                "affectedRows",
                DataPacket::new(json!(rows)),
            )
            .await?;
        }
    } else if let Some(error) = &data.error {
        api.set_output(
            "error",
            DataPacket::new(json!({
                "message": error,
                "code": "DB_ERROR"
            })),
        )
        .await?;
    }

    Ok(())
}

/// Simulates a retry scenario with initial failures followed by success.
///
/// # Arguments
///
/// * `api` - The ComponentRuntimeAPI to interact with
/// * `attempts` - The attempt number (1-based)
/// * `success_after` - The number of attempts after which to succeed
/// * `success_data` - The data to return on success
/// * `failure_data` - The data to return on failure
///
/// # Returns
///
/// Returns a result indicating whether the operation succeeded or failed
/// based on the current attempt number.
pub async fn simulate_retry_scenario<T, E>(
    api: Arc<dyn ComponentRuntimeAPI>,
    attempts: usize,
    success_after: usize,
    success_fn: impl FnOnce(Arc<dyn ComponentRuntimeAPI>) -> T + Clone,
    failure_fn: impl FnOnce(Arc<dyn ComponentRuntimeAPI>) -> E + Clone,
) -> Result<T, E> {
    api.log(
        LogLevel::Info,
        &format!(
            "Simulating retry scenario for {}: attempt {}/{}",
            api.name(),
            attempts,
            success_after
        ),
    )
    .await
    .map_err(|_| failure_fn.clone()(api.clone()))?;

    if attempts >= success_after {
        Ok(success_fn(api))
    } else {
        Err(failure_fn(api))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mocks::component::create_mock_component_runtime_api;
    use serde_json::json;

    #[tokio::test]
    async fn test_simulate_http_response_success() {
        let mock_runtime = Arc::new(create_mock_component_runtime_api("http-test"));
        let data = HttpResponseData::new(200, json!({"result": "success"}));

        let result = simulate_http_response(mock_runtime.clone(), data).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_simulate_http_response_error() {
        let mock_runtime = Arc::new(create_mock_component_runtime_api("http-test"));
        let data = HttpResponseData::new(404, json!({"error": "not found"}));

        let result = simulate_http_response(mock_runtime.clone(), data).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_simulate_http_response_with_latency() {
        let mock_runtime = Arc::new(create_mock_component_runtime_api("http-test"));
        let data = HttpResponseData::new(200, json!({"result": "success"}));

        let start = std::time::Instant::now();
        let result = simulate_http_response_with_latency(mock_runtime.clone(), data, 100).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(elapsed.as_millis() >= 100);
    }

    #[tokio::test]
    async fn test_simulate_db_operation_success() {
        let mock_runtime = Arc::new(create_mock_component_runtime_api("db-test"));
        let data = DbOperationData::success(json!([{"id": 1, "name": "Test"}]));

        let result = simulate_db_operation(mock_runtime.clone(), data).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_simulate_db_operation_failure() {
        let mock_runtime = Arc::new(create_mock_component_runtime_api("db-test"));
        let data = DbOperationData::failure("Connection error");

        let result = simulate_db_operation(mock_runtime.clone(), data).await;
        assert!(result.is_ok());
    }
} 
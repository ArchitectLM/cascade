use thiserror::Error;

/// Error types for the test utilities
#[derive(Debug, Error)]
pub enum TestError {
    /// HTTP client error
    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Timeout error
    #[error("Timeout error: {0}")]
    Timeout(String),

    /// Server error
    #[error("Server error: {0}")]
    Server(String),

    /// Feature not enabled error
    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(String),

    /// Not implemented error
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Test setup failed
    #[error("Test setup failed: {0}")]
    TestSetupFailed(String),

    /// Other errors
    #[error("Other error: {0}")]
    Other(String),
} 
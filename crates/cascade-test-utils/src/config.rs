use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub port: u16,
    pub timeout: Duration,
    pub retry_interval: Duration,
    pub max_retries: u32,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            port: 3000,
            timeout: Duration::from_secs(5),
            retry_interval: Duration::from_millis(100),
            max_retries: 3,
        }
    }
}

impl TestConfig {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            ..Default::default()
        }
    }
} 
use cascade_server::config::ServerConfig;
use cascade_monitoring::MonitoringConfig;
use anyhow::{Result, Context};

#[tokio::main]
async fn main() -> Result<()> {
    // Set up monitoring
    let monitoring_config = MonitoringConfig {
        service_name: "cascade-server".to_string(),
        enable_metrics: true,
        log_filter: std::env::var("LOG_FILTER").unwrap_or_else(|_| "info,cascade=debug".to_string()),
        metrics_interval: std::time::Duration::from_secs(10),
        environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
        flush_interval: std::time::Duration::from_secs(60),
    };
    
    cascade_monitoring::init(monitoring_config)
        .context("Failed to initialize monitoring")?;
    
    // Load configuration from environment variables
    let config = ServerConfig::load()
        .context("Failed to load configuration")?;
    
    // Run the server using the library's run function
    cascade_server::run(config).await
        .context("Server error")?;
    
    Ok(())
} 
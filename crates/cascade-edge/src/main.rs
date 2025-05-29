use anyhow::Context;
use cascade_edge::{CascadeEdge, EdgeConfig};
use cascade_monitoring::{init, MonitoringConfig, shutdown, metrics::EdgeMetrics};
use std::time::Duration;
use tokio::signal;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Initialize monitoring
    let monitoring_config = MonitoringConfig {
        service_name: "cascade-edge".to_string(),
        enable_metrics: true,
        log_filter: std::env::var("LOG_FILTER").unwrap_or_else(|_| "info,cascade=debug".to_string()),
        metrics_interval: Duration::from_secs(5),
        environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "dev".to_string()),
        flush_interval: Duration::from_secs(60),
    };

    init(monitoring_config).context("Failed to initialize monitoring")?;
    
    // Create edge configuration, use environment variables if available
    let config = EdgeConfig {
        port: std::env::var("EDGE_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3001),
        host: std::env::var("EDGE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
        server_url: std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:8080".to_string()),
    };
    
    // Create and start the edge runtime
    let edge = CascadeEdge::new(config);
    
    // Set up graceful shutdown
    let edge_task = tokio::spawn(async move {
        if let Err(e) = edge.run().await {
            eprintln!("Edge runtime error: {}", e);
        }
    });
    
    // Wait for shutdown signal
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("Shutdown signal received, shutting down gracefully");
            EdgeMetrics::record_server_communication("/shutdown", 0.0, true);
        },
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
        },
    }
    
    // Perform cleanup
    shutdown();
    
    // Wait for edge to finish
    let _ = tokio::time::timeout(Duration::from_secs(5), edge_task).await;
    
    Ok(())
}
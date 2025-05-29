use crate::config::TestConfig;
use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub struct TestServer {
    config: TestConfig,
    app: Router,
}

impl TestServer {
    pub fn new(config: TestConfig) -> Self {
        let app = Router::new()
            .route("/health", get(|| async { "OK" }))
            .route(
                "/api/v1/orders",
                post(|_body: String| async move {
                    // In a real implementation, this would process the order
                    "Order created"
                }),
            );

        Self { config, app }
    }

    pub async fn start(&self) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.config.port));
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        
        // Clone the app to avoid borrowing self in the spawned task
        let app = self.app.clone();
        
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Ok(addr)
    }

    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        // In a real implementation, this would gracefully shut down the server
        Ok(())
    }
} 
use std::sync::Arc;
use crate::CascadeServer;
use crate::config::ServerConfig;
use crate::error::ServerResult;
use crate::edge::EdgePlatform;
use crate::edge_manager::EdgeManager;
use cascade_content_store::ContentStorage;

/// Create a new server instance with all required dependencies
pub fn create_server(
    config: ServerConfig,
    content_store: Arc<dyn ContentStorage>,
    edge_platform: Arc<dyn EdgePlatform>,
) -> ServerResult<CascadeServer> {
    // Create shared state service
    let shared_state = crate::create_shared_state_service(&config)?;

    // Create the core runtime
    let core_runtime = crate::create_core_runtime(Some(shared_state.clone()))?;
    
    // Create the edge manager
    let edge_manager = EdgeManager::new(
        edge_platform.clone(),
        content_store.clone(),
        format!("http://{}:{}/api/v1/edge/callback", config.bind_address, config.port),
        Some(config.edge_callback_jwt_secret.clone().unwrap_or_default()),
        Some(config.edge_callback_jwt_issuer.clone()),
        Some(config.edge_callback_jwt_audience.clone()),
        config.edge_callback_jwt_expiry_seconds,
    );
    
    // Create and return the server
    let server = CascadeServer::new(
        config,
        content_store,
        edge_platform,
        edge_manager,
        core_runtime,
        shared_state,
    );
    
    Ok(server)
} 
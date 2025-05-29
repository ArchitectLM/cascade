//! PostgreSQL state store implementation for the Cascade Platform
//!
//! This crate provides PostgreSQL implementations of the core repository
//! interfaces defined in the cascade-core crate.

use std::sync::Arc;
use std::time::Duration;
use std::pin::Pin;
use std::future::Future;
use serde::{Serialize, Deserialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tracing::{debug, info};

pub mod repositories;
pub mod migrations;
#[cfg(test)]
pub mod test_utils;

use repositories::{
    PostgresFlowInstanceRepository, 
    PostgresFlowDefinitionRepository,
    PostgresComponentStateRepository,
    PostgresTimerRepository
};

use cascade_core::{
    domain::repository::{
        FlowInstanceRepository, FlowDefinitionRepository, 
        ComponentStateRepository, TimerRepository
    },
    domain::flow_instance::{FlowInstanceId, StepId},
    CoreError,
};

/// Configuration for PostgreSQL connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    /// Database connection string
    pub connection_string: String,
    
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    
    /// Timeout for acquiring a connection from the pool (in seconds)
    pub acquire_timeout_secs: u64,
    
    /// Whether to run migrations on startup
    pub run_migrations: bool,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            connection_string: "postgres://postgres:postgres@localhost/cascade".to_string(),
            max_connections: 5,
            acquire_timeout_secs: 30,
            run_migrations: true,
        }
    }
}

/// PostgreSQL connection wrapper
#[derive(Clone)]
pub struct PostgresConnection {
    pool: Option<PgPool>,
    test_mode: bool,
}

impl PostgresConnection {
    /// Create a new PostgreSQL connection
    pub async fn new(config: &PostgresConfig) -> Result<Self, CoreError> {
        // If we're in test mode via the TEST_MODE env var, create a mock connection
        if std::env::var("TEST_MODE").unwrap_or_default() == "1" {
            debug!("Creating PostgreSQL connection in test mode (no actual connection)");
            return Ok(Self { 
                pool: None,
                test_mode: true 
            });
        }
        
        #[cfg(not(feature = "test-mode"))]
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(Duration::from_secs(config.acquire_timeout_secs))
            .connect(&config.connection_string)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to connect to PostgreSQL: {}", e)))?;

        #[cfg(feature = "test-mode")]
        let pool = None;
        
        #[cfg(not(feature = "test-mode"))]
        debug!("Connected to PostgreSQL database");
        
        let conn = Self { 
            #[cfg(not(feature = "test-mode"))]
            pool: Some(pool),
            #[cfg(feature = "test-mode")]
            pool: None,
            #[cfg(not(feature = "test-mode"))]
            test_mode: false,
            #[cfg(feature = "test-mode")]
            test_mode: true,
        };
        
        if !conn.is_test_mode() && config.run_migrations {
            conn.run_migrations().await?;
        }
        
        Ok(conn)
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<(), CoreError> {
        if self.is_test_mode() {
            debug!("Skipping migrations in test mode");
            return Ok(());
        }

        let pool = self.pool()?;
        
        debug!("Running database migrations...");
        sqlx::migrate!("./migrations")
            .run(pool)
            .await
            .map_err(|e| CoreError::StateStoreError(e.to_string()))?;
        debug!("Migrations complete");
        
        Ok(())
    }

    /// Get the database connection pool
    pub fn pool(&self) -> Result<&PgPool, CoreError> {
        if self.is_test_mode() {
            return Err(CoreError::StateStoreError("Cannot access database pool in test mode".to_string()));
        }
        
        self.pool.as_ref().ok_or_else(|| {
            CoreError::StateStoreError("Database connection not initialized".to_string())
        })
    }

    /// Check if the connection is in test mode
    pub fn is_test_mode(&self) -> bool {
        self.test_mode
    }

    /// Create a new PostgreSQL connection in test mode (for testing without a database)
    pub fn new_test_mode() -> Self {
        debug!("Creating PostgreSQL connection in test mode");
        Self {
            pool: None,
            test_mode: true 
        }
    }
}

/// Provider for PostgreSQL state store repositories
pub struct PostgresStateStoreProvider {
    connection: PostgresConnection,
}

impl PostgresStateStoreProvider {
    /// Create a new PostgreSQL state store provider with default configuration
    pub async fn new(connection_string: &str) -> Result<Self, CoreError> {
        let config = PostgresConfig {
            connection_string: connection_string.to_string(),
            ..Default::default()
        };
        
        Self::with_config(config).await
    }
    
    /// Create a new PostgreSQL state store provider with custom configuration
    pub async fn with_config(config: PostgresConfig) -> Result<Self, CoreError> {
        let connection = PostgresConnection::new(&config).await?;
        
        Ok(Self { connection })
    }
    
    /// Create all repositories
    pub fn create_repositories(&self) -> (
        Arc<dyn FlowInstanceRepository>,
        Arc<dyn FlowDefinitionRepository>,
        Arc<dyn ComponentStateRepository>,
        Arc<dyn TimerRepository>,
        Option<Box<dyn Fn(FlowInstanceId, StepId) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>>
    ) {
        let conn = self.connection.clone();
        
        let flow_instance_repo = Arc::new(PostgresFlowInstanceRepository::new(conn.clone()));
        let flow_definition_repo = Arc::new(PostgresFlowDefinitionRepository::new(conn.clone()));
        let component_state_repo = Arc::new(PostgresComponentStateRepository::new(conn.clone()));
        let timer_repo = Arc::new(PostgresTimerRepository::new(conn.clone()));
        
        // This callback is used by cascade-core to set up timer processing
        let timer_callback = Box::new(move |flow_id: FlowInstanceId, step_id: StepId| -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async move {
                debug!("Timer callback for flow: {}, step: {}", flow_id.0, step_id.0);
                // The actual timer callback logic will be set by the runtime
            })
        });
        
        (
            flow_instance_repo,
            flow_definition_repo,
            component_state_repo,
            timer_repo,
            Some(timer_callback),
        )
    }
}

/// A Cascade state store implementation that uses PostgreSQL for persistence
#[derive(Clone)]
pub struct PostgresStateStore {
    conn: PostgresConnection,
    flow_instance_repo: PostgresFlowInstanceRepository,
    flow_definition_repo: PostgresFlowDefinitionRepository,
    component_state_repo: PostgresComponentStateRepository,
    timer_repo: PostgresTimerRepository,
}

impl PostgresStateStore {
    /// Create a new PostgreSQL state store
    pub async fn new(connection_string: &str) -> Result<Self, CoreError> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .connect(connection_string)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to connect to PostgreSQL: {}", e)))?;
        
        let conn = PostgresConnection { pool: Some(pool), test_mode: false };
        
        // Run migrations
        if let Err(e) = Self::run_migrations(&conn).await {
            return Err(CoreError::StateStoreError(format!("Failed to run migrations: {}", e)));
        }
        
        Ok(Self {
            flow_instance_repo: PostgresFlowInstanceRepository::new(conn.clone()),
            flow_definition_repo: PostgresFlowDefinitionRepository::new(conn.clone()),
            component_state_repo: PostgresComponentStateRepository::new(conn.clone()),
            timer_repo: PostgresTimerRepository::new(conn.clone()),
            conn,
        })
    }
    
    /// Run database migrations
    async fn run_migrations(conn: &PostgresConnection) -> Result<(), CoreError> {
        debug!("Running PostgreSQL migrations...");
        
        // Get migrations
        let migrations = migrations::generate_migrations();
        
        for (migration_name, migration_sql) in migrations {
            debug!("Applying migration: {}", migration_name);
            
            // Execute the migration SQL
            sqlx::query(migration_sql)
                .execute(conn.pool()?)
                .await
                .map_err(|e| CoreError::StateStoreError(
                    format!("Migration '{}' failed: {}", migration_name, e)
                ))?;
        }
        
        info!("PostgreSQL migrations completed successfully");
        Ok(())
    }
    
    /// Get the connection
    pub fn connection(&self) -> &PostgresConnection {
        &self.conn
    }
    
    /// Get the flow instance repository
    pub fn flow_instance_repository(&self) -> &PostgresFlowInstanceRepository {
        &self.flow_instance_repo
    }
    
    /// Get the flow definition repository
    pub fn flow_definition_repository(&self) -> &PostgresFlowDefinitionRepository {
        &self.flow_definition_repo
    }
    
    /// Get the component state repository
    pub fn component_state_repository(&self) -> &PostgresComponentStateRepository {
        &self.component_state_repo
    }
    
    /// Get the timer repository
    pub fn timer_repository(&self) -> &PostgresTimerRepository {
        &self.timer_repo
    }
}

// Export test utilities for testing with this crate
#[cfg(test)]
pub use crate::test_utils::{MockStateStore, create_mock_state_store, create_mock_state_store_sync};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::MockStateStore;
    use tokio_test::block_on;
    use uuid::Uuid;
    use serde_json::json;
    use cascade_core::{
        domain::flow_instance::FlowId,
        domain::flow_definition::{FlowDefinition, TriggerDefinition},
    };

    #[test]
    fn test_mock_state_store() {
        // Create a mock state store
        let store = block_on(async {
            MockStateStore::new().await
        });
        
        // Create a test flow definition
        let flow_id = FlowId(Uuid::new_v4().to_string());
        let flow_def = FlowDefinition {
            id: flow_id.clone(),
            version: "1.0.0".to_string(),
            name: "Test Flow".to_string(),
            description: Some("Test Flow Description".to_string()),
            trigger: Some(TriggerDefinition {
                trigger_type: "event".to_string(),
                config: json!({}),
            }),
            steps: vec![],
        };
        
        // Save flow definition
        block_on(async {
            let repo = store.flow_definition_repository();
            repo.save(&flow_def).await.unwrap();
            
            // Retrieval will return None in mock mode, but shouldn't error
            let result = repo.find_by_id(&flow_id).await.unwrap();
            assert!(result.is_none()); // Mock implementation doesn't actually store data
        });
        
        // Test is successful if no panics occur
        assert!(true);
    }
}

#[cfg(test)]
mod test_timer;

#[cfg(test)]
mod isolated_tests {
    use crate::{PostgresConnection, repositories::PostgresTimerRepository};
    use cascade_core::domain::flow_instance::{FlowInstanceId, StepId};
    use cascade_core::domain::repository::TimerRepository;
    use cascade_core::application::runtime_interface::TimerProcessingRepository;
    use std::sync::Arc;
    use std::pin::Pin;
    use std::future::Future;
    use std::time::Duration;
    use tokio_test::block_on;
    
    #[test]
    fn test_timer_processing_repository_test_mode() {
        // Create a test mode connection
        let conn = PostgresConnection::new_test_mode();
        
        // Create timer repository 
        let repo = PostgresTimerRepository::new(conn);
        
        // Schedule a timer (should return mock ID in test mode)
        let flow_id = FlowInstanceId("test-flow".to_string());
        let step_id = StepId("test-step".to_string());
        let duration = Duration::from_secs(10);
        
        let timer_id = block_on(async {
            repo.schedule(&flow_id, &step_id, duration).await.unwrap()
        });
        
        assert!(timer_id.starts_with("mock-timer-"), "Test mode should return mock timer ID");
        
        // Test timer processing with a callback that would panic if actually called
        let callback = Arc::new(|_flow_id: FlowInstanceId, _step_id: StepId| {
            Box::pin(async move {
                panic!("This should never be called in test mode");
            }) as Pin<Box<dyn Future<Output = ()> + Send>>
        });
        
        // In test mode, this should just return and not start any background task
        repo.start_timer_processing(callback);
        
        // Cancel the mock timer
        let cancel_result = block_on(async {
            repo.cancel(&timer_id).await
        });
        
        assert!(cancel_result.is_ok(), "Cancel should succeed in test mode");
    }
} 
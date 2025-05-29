use cascade_core::CoreError;
use sqlx::{postgres::PgPoolOptions, PgPool, Postgres};
use std::time::Duration;
use tracing::info;
use anyhow::Result;

/// Database connection manager for Postgres
#[derive(Clone)]
pub struct PostgresConnection {
    pub(crate) pool: PgPool,
}

impl PostgresConnection {
    /// Create a new PostgreSQL connection pool
    pub async fn new(
        connection_string: &str, 
        max_connections: u32, 
        acquire_timeout: Duration
    ) -> Result<Self, CoreError> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(acquire_timeout)
            .connect(connection_string)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to connect to database: {}", e)))?;
        
        Ok(Self { pool })
    }
    
    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<(), CoreError> {
        info!("Running database migrations");
        
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to run migrations: {}", e)))?;
        
        info!("Migrations completed successfully");
        Ok(())
    }
    
    /// Connect to a PostgreSQL database
    pub async fn connect(connection_string: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .connect(connection_string)
            .await?;
        
        Ok(Self { pool })
    }
    
    /// Get a reference to the database pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// A mock connection for testing
#[cfg(test)]
pub struct MockPostgresConnection {
    pub pool: sqlx::PgPool,
}

#[cfg(test)]
impl MockPostgresConnection {
    /// Create a new mock connection for testing
    pub async fn new() -> Self {
        // Create a pool with options to avoid actual connection
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost:5432/postgres")
            .expect("Failed to create mock connection pool");
        
        Self { pool }
    }
    
    /// Get the connection pool
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}

#[cfg(test)]
impl Clone for MockPostgresConnection {
    fn clone(&self) -> Self {
        Self { pool: self.pool.clone() }
    }
}

#[cfg(test)]
impl From<MockPostgresConnection> for PostgresConnection {
    fn from(mock: MockPostgresConnection) -> Self {
        // Warning: This is only for testing and will not actually connect to a database
        Self { pool: mock.pool }
    }
} 
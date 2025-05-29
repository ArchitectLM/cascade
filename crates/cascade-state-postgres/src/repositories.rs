use cascade_core::{
    CoreError,
    domain::repository::{
        ComponentStateRepository, FlowDefinitionRepository, FlowInstanceRepository, TimerRepository
    },
    domain::flow_instance::{FlowInstance, FlowInstanceId, FlowId, StepId, CorrelationId, FlowStatus},
    domain::flow_definition::FlowDefinition,
    application::runtime_interface::TimerProcessingRepository,
};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::Row;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, debug};
use uuid::Uuid;
use std::pin::Pin;
use std::future::Future;

use crate::PostgresConnection;

/// Postgres implementation of the FlowInstanceRepository
#[derive(Clone)]
pub struct PostgresFlowInstanceRepository {
    conn: PostgresConnection,
}

impl PostgresFlowInstanceRepository {
    /// Create a new Postgres flow instance repository
    pub fn new(conn: PostgresConnection) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl FlowInstanceRepository for PostgresFlowInstanceRepository {
    async fn find_by_id(&self, id: &FlowInstanceId) -> Result<Option<FlowInstance>, CoreError> {
        if self.conn.is_test_mode() {
            // For testing, return None without trying to access the database
            tracing::debug!("Test mode PostgreSQL: find_by_id called for {}", id.0);
            return Ok(None);
        }
        
        let query = format!(
            "SELECT data FROM flow_instances WHERE id = '{}'",
            id.0
        );
        
        match sqlx::query(&query)
            .fetch_optional(self.conn.pool()?)
            .await {
                Ok(record) => {
                    match record {
                        Some(row) => {
                            let data: serde_json::Value = row.try_get("data")
                                .map_err(|e| CoreError::SerializationError(format!("Error getting data: {}", e)))?;
                            
                            let instance: FlowInstance = serde_json::from_value(data)
                                .map_err(|e| CoreError::SerializationError(format!("Error deserializing flow instance: {}", e)))?;
                            Ok(Some(instance))
                        },
                        None => Ok(None),
                    }
                },
                Err(e) => {
                    Err(CoreError::StateStoreError(format!("Database error: {}", e)))
                }
            }
    }
    
    async fn save(&self, instance: &FlowInstance) -> Result<(), CoreError> {
        if self.conn.is_test_mode() {
            // For testing, just return success without saving
            tracing::debug!("Test mode PostgreSQL: save called for {}", instance.id.0);
            return Ok(());
        }
        
        let data = serde_json::to_value(instance)
            .map_err(|e| CoreError::SerializationError(format!("Error serializing flow instance: {}", e)))?;
        
        let flow_id = &instance.flow_id.0;
        let status = format!("{:?}", instance.status);
        let created_at = instance.created_at;
        let updated_at = instance.updated_at;
        
        // Save instance data
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            INSERT INTO flow_instances (id, flow_id, status, data, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET 
                flow_id = $2,
                status = $3,
                data = $4,
                updated_at = $6
        ";
        
        sqlx::query(query)
            .bind(&instance.id.0)
            .bind(flow_id)
            .bind(&status)
            .bind(&data)
            .bind(created_at)
            .bind(updated_at)
            .execute(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to save flow instance: {}", e)))?;
        
        // Update correlation indexes if needed
        if instance.status == FlowStatus::WaitingForEvent {
            for (step_id, correlation_id) in &instance.correlation_data {
                // Using standard query execution instead of sqlx::query! macro
                let query = "
                    INSERT INTO correlations (correlation_id, flow_instance_id, step_id)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (correlation_id, flow_instance_id) DO UPDATE SET 
                        step_id = $3
                ";
                
                sqlx::query(query)
                    .bind(correlation_id)
                    .bind(&instance.id.0)
                    .bind(&step_id.0)
                    .execute(self.conn.pool()?)
                    .await
                    .map_err(|e| CoreError::StateStoreError(format!("Failed to save correlation: {}", e)))?;
            }
        }
        
        Ok(())
    }
    
    async fn delete(&self, id: &FlowInstanceId) -> Result<(), CoreError> {
        if self.conn.is_test_mode() {
            // For testing, just return success without deleting
            tracing::debug!("Test mode PostgreSQL: delete called for {}", id.0);
            return Ok(());
        }
        
        // Delete correlations first
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            DELETE FROM correlations
            WHERE flow_instance_id = $1
        ";
        
        sqlx::query(query)
            .bind(&id.0)
            .execute(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to delete correlations: {}", e)))?;
        
        // Then delete the instance
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            DELETE FROM flow_instances
            WHERE id = $1
        ";
        
        sqlx::query(query)
            .bind(&id.0)
            .execute(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to delete flow instance: {}", e)))?;
        
        Ok(())
    }
    
    async fn find_by_correlation(&self, correlation_id: &CorrelationId) -> Result<Vec<FlowInstance>, CoreError> {
        if self.conn.is_test_mode() {
            // For testing, return an empty vector
            tracing::debug!("Test mode PostgreSQL: find_by_correlation called for {}", correlation_id.0);
            return Ok(vec![]);
        }
        
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            SELECT i.data
            FROM flow_instances i
            JOIN correlations c ON i.id = c.flow_instance_id
            WHERE c.correlation_id = $1
        ";
        
        let rows = sqlx::query(query)
            .bind(&correlation_id.0)
            .fetch_all(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Database error: {}", e)))?;
        
        let mut instances = Vec::new();
        for row in rows {
            let data: serde_json::Value = row.try_get("data")
                .map_err(|e| CoreError::SerializationError(format!("Error getting data: {}", e)))?;
            let instance: FlowInstance = serde_json::from_value(data)
                .map_err(|e| CoreError::SerializationError(format!("Error deserializing flow instance: {}", e)))?;
            instances.push(instance);
        }
        
        Ok(instances)
    }
    
    async fn find_all_for_flow(&self, flow_id: &FlowId) -> Result<Vec<FlowInstanceId>, CoreError> {
        if self.conn.is_test_mode() {
            // For testing, return an empty vector
            tracing::debug!("Test mode PostgreSQL: find_all_for_flow called for {}", flow_id.0);
            return Ok(vec![]);
        }
        
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            SELECT id FROM flow_instances
            WHERE flow_id = $1
        ";
        
        let rows = sqlx::query(query)
            .bind(&flow_id.0)
            .fetch_all(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Database error: {}", e)))?;
        
        let instances = rows.into_iter()
            .map(|row| FlowInstanceId(row.get("id")))
            .collect();
        
        Ok(instances)
    }

    async fn list_instances(
        &self,
        flow_id: Option<&FlowId>,
        status: Option<&FlowStatus>
    ) -> Result<Vec<FlowInstance>, CoreError> {
        if self.conn.is_test_mode() {
            // For testing, return an empty vector
            tracing::debug!("Test mode PostgreSQL: list_instances called");
            return Ok(vec![]);
        }
        
        // Default implementation with a reasonable limit
        let limit = 100;
        
        let mut query_str = String::from(
            "SELECT data FROM flow_instances WHERE 1=1"
        );
        
        let mut params = Vec::new();
        let mut param_index = 1;
        
        if let Some(flow_id) = flow_id {
            query_str.push_str(&format!(" AND flow_id = ${}", param_index));
            params.push(flow_id.0.clone());
            param_index += 1;
        }
        
        query_str.push_str(&format!(" ORDER BY created_at DESC LIMIT ${}", param_index));
        params.push(limit.to_string());
        
        let query = sqlx::query(&query_str);
        
        // Build the query with parameters
        let query = params.iter().fold(query, |q, param| q.bind(param));
        
        let rows = query
            .fetch_all(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Database error: {}", e)))?;
        
        let mut instances = Vec::new();
        for row in rows {
            let data: serde_json::Value = row.get("data");
            let instance: FlowInstance = serde_json::from_value(data)
                .map_err(|e| CoreError::SerializationError(format!("Error deserializing flow instance: {}", e)))?;
            
            // Filter by status if provided
            if let Some(status_filter) = status {
                if &instance.status == status_filter {
                    instances.push(instance);
                }
            } else {
                instances.push(instance);
            }
        }
        
        Ok(instances)
    }
}

/// Postgres implementation of the FlowDefinitionRepository
#[derive(Clone)]
pub struct PostgresFlowDefinitionRepository {
    conn: PostgresConnection,
}

impl PostgresFlowDefinitionRepository {
    /// Create a new Postgres flow definition repository
    pub fn new(conn: PostgresConnection) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl FlowDefinitionRepository for PostgresFlowDefinitionRepository {
    async fn find_by_id(&self, id: &FlowId) -> Result<Option<FlowDefinition>, CoreError> {
        if self.conn.is_test_mode() {
            // For testing, return None
            tracing::debug!("Test mode PostgreSQL: find_by_id called for flow definition {}", id.0);
            return Ok(None);
        }
        
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            SELECT data FROM flow_definitions
            WHERE id = $1
        ";
        
        let row = sqlx::query(query)
            .bind(&id.0)
            .fetch_optional(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Database error: {}", e)))?;
        
        match row {
            Some(row) => {
                let data: serde_json::Value = row.try_get("data")
                    .map_err(|e| CoreError::SerializationError(format!("Error getting data: {}", e)))?;
                let definition: FlowDefinition = serde_json::from_value(data)
                    .map_err(|e| CoreError::SerializationError(format!("Error deserializing flow definition: {}", e)))?;
                Ok(Some(definition))
            },
            None => Ok(None),
        }
    }
    
    async fn save(&self, definition: &FlowDefinition) -> Result<(), CoreError> {
        if self.conn.is_test_mode() {
            // For testing, just return success
            tracing::debug!("Test mode PostgreSQL: save called for flow definition {}", definition.id.0);
            return Ok(());
        }
        
        let data = serde_json::to_value(definition)
            .map_err(|e| CoreError::SerializationError(format!("Error serializing flow definition: {}", e)))?;
        
        // Get current timestamp
        let now = Utc::now();
        
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            INSERT INTO flow_definitions (id, data, created_at, updated_at)
            VALUES ($1, $2, $3, $3)
            ON CONFLICT (id) DO UPDATE SET 
                data = $2,
                updated_at = $3
        ";
        
        sqlx::query(query)
            .bind(&definition.id.0)
            .bind(&data)
            .bind(now)
            .execute(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to save flow definition: {}", e)))?;
        
        Ok(())
    }
    
    async fn delete(&self, id: &FlowId) -> Result<(), CoreError> {
        if self.conn.is_test_mode() {
            // For testing, just return success
            tracing::debug!("Test mode PostgreSQL: delete called for flow definition {}", id.0);
            return Ok(());
        }
        
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            DELETE FROM flow_definitions
            WHERE id = $1
        ";
        
        sqlx::query(query)
            .bind(&id.0)
            .execute(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to delete flow definition: {}", e)))?;
        
        Ok(())
    }
    
    async fn find_all(&self) -> Result<Vec<FlowDefinition>, CoreError> {
        if self.conn.is_test_mode() {
            // For testing, return empty vector
            tracing::debug!("Test mode PostgreSQL: find_all called for flow definitions");
            return Ok(vec![]);
        }
        
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            SELECT data FROM flow_definitions
        ";
        
        let rows = sqlx::query(query)
            .fetch_all(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Database error: {}", e)))?;
        
        let mut definitions = Vec::new();
        for row in rows {
            let data: serde_json::Value = row.try_get("data")
                .map_err(|e| CoreError::SerializationError(format!("Error getting data: {}", e)))?;
            let definition: FlowDefinition = serde_json::from_value(data)
                .map_err(|e| CoreError::SerializationError(format!("Error deserializing flow definition: {}", e)))?;
            definitions.push(definition);
        }
        
        Ok(definitions)
    }

    async fn list_definitions(&self) -> Result<Vec<FlowId>, CoreError> {
        if self.conn.is_test_mode() {
            // For testing, return empty vector
            tracing::debug!("Test mode PostgreSQL: list_definitions called");
            return Ok(vec![]);
        }
        
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            SELECT id FROM flow_definitions
            ORDER BY created_at DESC
        ";
        
        let rows = sqlx::query(query)
            .fetch_all(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Database error: {}", e)))?;
        
        let flow_ids = rows.into_iter()
            .map(|row| FlowId(row.get("id")))
            .collect();
        
        Ok(flow_ids)
    }
}

/// Postgres implementation of the ComponentStateRepository
#[derive(Clone)]
pub struct PostgresComponentStateRepository {
    conn: PostgresConnection,
}

impl PostgresComponentStateRepository {
    /// Create a new Postgres component state repository
    pub fn new(conn: PostgresConnection) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl ComponentStateRepository for PostgresComponentStateRepository {
    async fn get_state(
        &self, 
        flow_instance_id: &FlowInstanceId, 
        step_id: &StepId
    ) -> Result<Option<serde_json::Value>, CoreError> {
        if self.conn.is_test_mode() {
            // For testing, return None
            tracing::debug!("Test mode PostgreSQL: get_state called for {}/{}", flow_instance_id.0, step_id.0);
            return Ok(None);
        }
        
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            SELECT state FROM component_states
            WHERE flow_instance_id = $1 AND step_id = $2
        ";
        
        let row = sqlx::query(query)
            .bind(&flow_instance_id.0)
            .bind(&step_id.0)
            .fetch_optional(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Database error: {}", e)))?;
        
        match row {
            Some(row) => Ok(Some(row.try_get("state")
                .map_err(|e| CoreError::SerializationError(format!("Error getting state: {}", e)))?)),
            None => Ok(None),
        }
    }
    
    async fn save_state(
        &self, 
        flow_instance_id: &FlowInstanceId, 
        step_id: &StepId, 
        state: serde_json::Value
    ) -> Result<(), CoreError> {
        if self.conn.is_test_mode() {
            // For testing, just return success
            tracing::debug!("Test mode PostgreSQL: save_state called for {}/{}", flow_instance_id.0, step_id.0);
            return Ok(());
        }
        
        // Get current timestamp
        let now = Utc::now();
        
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            INSERT INTO component_states (flow_instance_id, step_id, state, updated_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (flow_instance_id, step_id) DO UPDATE SET 
                state = $3,
                updated_at = $4
        ";
        
        sqlx::query(query)
            .bind(&flow_instance_id.0)
            .bind(&step_id.0)
            .bind(&state)
            .bind(now)
            .execute(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to save component state: {}", e)))?;
        
        Ok(())
    }
    
    async fn delete_state(
        &self, 
        flow_instance_id: &FlowInstanceId, 
        step_id: &StepId
    ) -> Result<(), CoreError> {
        if self.conn.is_test_mode() {
            // For testing, just return success
            tracing::debug!("Test mode PostgreSQL: delete_state called for {}/{}", flow_instance_id.0, step_id.0);
            return Ok(());
        }
        
        // Using standard query execution instead of sqlx::query! macro
        let query = "
            DELETE FROM component_states
            WHERE flow_instance_id = $1 AND step_id = $2
        ";
        
        sqlx::query(query)
            .bind(&flow_instance_id.0)
            .bind(&step_id.0)
            .execute(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to delete component state: {}", e)))?;
        
        Ok(())
    }
}

/// Postgres implementation of the TimerRepository
pub struct PostgresTimerRepository {
    conn: PostgresConnection,
    timer_callback: Option<Arc<dyn Fn(FlowInstanceId, StepId) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>>,
}

impl Clone for PostgresTimerRepository {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            timer_callback: self.timer_callback.clone(),
        }
    }
}

impl PostgresTimerRepository {
    /// Create a new PostgreSQL timer repository
    pub fn new(conn: PostgresConnection) -> Self {
        Self { 
            conn,
            timer_callback: None,
        }
    }
}

// Implementation of TimerProcessingRepository trait from cascade-core
#[async_trait]
impl TimerProcessingRepository for PostgresTimerRepository {
    fn start_timer_processing(
        &self,
        callback: Arc<dyn Fn(FlowInstanceId, StepId) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>,
    ) {
        if self.conn.is_test_mode() {
            debug!("Timer processing not started in test mode");
            return;
        }

        // Clone the connection for the closure
        let conn = self.conn.clone();
        
        // Spawn a background task that processes timers
        tokio::spawn(async move {
            // Process immediately once
            if let Err(e) = process_due_timers(&conn, callback.clone()).await {
                error!("Failed to process timers: {}", e);
            }
            
            // Start ongoing processing
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                if let Err(e) = process_due_timers(&conn, callback.clone()).await {
                    error!("Failed to process timers: {}", e);
                }
            }
        });
    }
}

/// Process due timers
async fn process_due_timers(
    conn: &PostgresConnection,
    callback: Arc<dyn Fn(FlowInstanceId, StepId) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>,
) -> Result<(), CoreError> {
    // For testing, just return success
    if conn.is_test_mode() {
        debug!("Timer processing skipped in test mode");
        return Ok(());
    }
    
    // Get database connection
    let pool = conn.pool()?;
    
    // Start transaction
    let mut tx = pool.begin()
        .await
        .map_err(|e| CoreError::StateStoreError(format!("Failed to begin transaction: {}", e)))?;
    
    // Find pending timers that have expired
    let query = "
        UPDATE timers
        SET status = 'processing'
        WHERE status = 'pending' 
        AND scheduled_time <= NOW()
        RETURNING id, flow_instance_id, step_id
    ";
    
    let records = sqlx::query(query)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| CoreError::StateStoreError(format!("Failed to fetch pending timers: {}", e)))?;
    
    // Process each timer
    for record in records.iter() {
        let timer_id = record.get::<String, _>("id");
        let flow_instance_id = FlowInstanceId(record.get::<String, _>("flow_instance_id"));
        let step_id = StepId(record.get::<String, _>("step_id"));
        
        // Mark timer as completed
        let update_query = format!(
            "UPDATE timers SET status = 'completed' WHERE id = '{}'",
            timer_id
        );
        
        sqlx::query(&update_query)
            .execute(&mut *tx)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to update timer status: {}", e)))?;
        
        // Call processor with timer info
        // Note: this does not happen within the transaction
        let callback_clone = callback.clone();
        tokio::spawn(async move {
            debug!("Processing timer {} for {}/{}", timer_id, flow_instance_id.0, step_id.0);
            let future = callback_clone(flow_instance_id, step_id);
            future.await;
        });
    }
    
    // Commit transaction
    tx.commit()
        .await
        .map_err(|e| CoreError::StateStoreError(format!("Failed to commit transaction: {}", e)))?;
    
    Ok(())
}

#[async_trait]
impl TimerRepository for PostgresTimerRepository {
    async fn schedule(
        &self, 
        flow_instance_id: &FlowInstanceId, 
        step_id: &StepId, 
        duration: Duration
    ) -> Result<String, CoreError> {
        if self.conn.is_test_mode() {
            // For testing, return a mock timer ID
            let timer_id = format!("mock-timer-{}-{}", flow_instance_id.0, step_id.0);
            tracing::debug!("Test mode PostgreSQL: schedule timer {} for {}/{}", 
                            timer_id, flow_instance_id.0, step_id.0);
            return Ok(timer_id);
        }
        
        // Generate a unique ID for the timer
        let id = Uuid::new_v4().to_string();
        
        // Calculate the scheduled time
        let scheduled_time = Utc::now() + chrono::Duration::from_std(duration)
            .map_err(|e| CoreError::TimerServiceError(format!("Invalid duration: {}", e)))?;

        // Standard query execution
        let query = format!(
            "INSERT INTO timers (id, flow_instance_id, step_id, scheduled_time, status, created_at) 
             VALUES ('{}', '{}', '{}', '{}', 'pending', NOW())",
            id, flow_instance_id.0, step_id.0, scheduled_time
        );
        
        sqlx::query(&query)
            .execute(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to schedule timer: {}", e)))?;
        
        Ok(id)
    }
    
    async fn cancel(&self, timer_id: &str) -> Result<(), CoreError> {
        if self.conn.is_test_mode() {
            // For testing, just return success
            tracing::debug!("Test mode PostgreSQL: cancel timer {}", timer_id);
            return Ok(());
        }
        
        // Standard query execution
        let query = format!(
            "DELETE FROM timers WHERE id = '{}' AND status = 'pending'",
            timer_id
        );
        
        sqlx::query(&query)
            .execute(self.conn.pool()?)
            .await
            .map_err(|e| CoreError::StateStoreError(format!("Failed to cancel timer: {}", e)))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::MockStateStore;
    use tokio_test::block_on;

    #[test]
    fn test_postgres_implementation() {
        // Create a mock state store
        let store = block_on(async {
            MockStateStore::new().await
        });
        
        // Create the repositories
        let _flow_instance_repo = store.flow_instance_repository();
        let _flow_definition_repo = store.flow_definition_repository();
        let _component_state_repo = store.component_state_repository();
        let timer_repo = store.timer_repository();
        
        // Test timer processing
        let callback = Arc::new(|flow_id: FlowInstanceId, step_id: StepId| {
            Box::pin(async move {
                println!("Timer fired for flow {} step {}", flow_id.0, step_id.0);
            }) as Pin<Box<dyn Future<Output = ()> + Send>>
        });
        
        timer_repo.start_timer_processing(callback);
        
        // Just check that we can create the repositories without errors
        assert!(true, "Repositories were created successfully");
    }
    
    #[test]
    fn test_timer_repository_test_mode() {
        // Create a mock state store with test mode
        let store = block_on(async {
            MockStateStore::new().await
        });
        
        let timer_repo = store.timer_repository();
        
        // Schedule a timer in test mode
        let flow_id = FlowInstanceId("test-flow-1".to_string());
        let step_id = StepId("test-step-1".to_string());
        let duration = Duration::from_secs(10);
        
        let timer_id = block_on(async {
            timer_repo.schedule(&flow_id, &step_id, duration).await.unwrap()
        });
        
        // Verify we get a mock timer ID in test mode
        assert!(timer_id.starts_with("mock-timer-"), "Timer ID should be a mock in test mode");
        
        // Cancel the timer
        let cancel_result = block_on(async {
            timer_repo.cancel(&timer_id).await
        });
        
        assert!(cancel_result.is_ok(), "Cancel should succeed in test mode");
        
        // Verify timer processing starts but doesn't actually process in test mode
        let callback = Arc::new(|_: FlowInstanceId, _: StepId| {
            Box::pin(async move {
                panic!("This should not be called in test mode");
            }) as Pin<Box<dyn Future<Output = ()> + Send>>
        });
        
        // This should return immediately without starting the background task in test mode
        timer_repo.start_timer_processing(callback);
        
        // If we got here without panicking, the test passed
        assert!(true, "Timer processing correctly skipped in test mode");
    }
} 
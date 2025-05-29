//! Repository traits for the Cascade Core
//!
//! This module defines the core repository traits that are used by the
//! Cascade Core runtime. External crates can implement these traits to
//! provide different persistence mechanisms.

use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

use super::flow_definition::FlowDefinition;
use super::flow_instance::{
    CorrelationId, FlowId, FlowInstance, FlowInstanceId, FlowStatus, StepId,
};
use crate::CoreError;

/// Repository for flow instances
#[async_trait]
pub trait FlowInstanceRepository: Send + Sync {
    /// Find a flow instance by ID
    async fn find_by_id(&self, id: &FlowInstanceId) -> Result<Option<FlowInstance>, CoreError>;

    /// Save a flow instance
    async fn save(&self, instance: &FlowInstance) -> Result<(), CoreError>;

    /// Delete a flow instance
    async fn delete(&self, id: &FlowInstanceId) -> Result<(), CoreError>;

    /// Find flow instances by correlation ID
    async fn find_by_correlation(
        &self,
        correlation_id: &CorrelationId,
    ) -> Result<Vec<FlowInstance>, CoreError>;

    /// Find all flow instances for a flow definition
    async fn find_all_for_flow(&self, flow_id: &FlowId) -> Result<Vec<FlowInstanceId>, CoreError>;

    /// List flow instances with optional filters
    async fn list_instances(
        &self,
        flow_id: Option<&FlowId>,
        status: Option<&FlowStatus>,
    ) -> Result<Vec<FlowInstance>, CoreError>;
}

/// Repository for flow definitions
#[async_trait]
pub trait FlowDefinitionRepository: Send + Sync {
    /// Find a flow definition by ID
    async fn find_by_id(&self, id: &FlowId) -> Result<Option<FlowDefinition>, CoreError>;

    /// Save a flow definition
    async fn save(&self, definition: &FlowDefinition) -> Result<(), CoreError>;

    /// Delete a flow definition
    async fn delete(&self, id: &FlowId) -> Result<(), CoreError>;

    /// List all flow definitions
    async fn list_definitions(&self) -> Result<Vec<FlowId>, CoreError>;

    /// Get all flow definitions
    async fn find_all(&self) -> Result<Vec<FlowDefinition>, CoreError>;
}

/// Repository for component state
#[async_trait]
pub trait ComponentStateRepository: Send + Sync {
    /// Get state for a component in a flow instance
    async fn get_state(
        &self,
        flow_instance_id: &FlowInstanceId,
        step_id: &StepId,
    ) -> Result<Option<serde_json::Value>, CoreError>;

    /// Save state for a component in a flow instance
    async fn save_state(
        &self,
        flow_instance_id: &FlowInstanceId,
        step_id: &StepId,
        state: serde_json::Value,
    ) -> Result<(), CoreError>;

    /// Delete state for a component in a flow instance
    async fn delete_state(
        &self,
        flow_instance_id: &FlowInstanceId,
        step_id: &StepId,
    ) -> Result<(), CoreError>;
}

/// Manages time-based events for flows
#[async_trait]
pub trait TimerRepository: Send + Sync {
    /// Schedule a timer that will invoke the given callback after the specified duration
    async fn schedule(
        &self,
        flow_instance_id: &FlowInstanceId,
        step_id: &StepId,
        duration: Duration,
    ) -> Result<String, CoreError>;

    /// Cancel a previously scheduled timer
    async fn cancel(&self, timer_id: &str) -> Result<(), CoreError>;
}

/// Memory implementations for testing
#[cfg(feature = "testing")]
pub mod memory {
    use super::*;
    use dashmap::DashMap;
    use std::sync::RwLock;
    use std::time::{Duration, Instant};
    use tokio::sync::mpsc;

    /// In-memory implementation of the flow instance repository with improved performance
    /// using concurrent maps to reduce lock contention
    pub struct MemoryFlowInstanceRepository {
        instances: std::sync::Arc<DashMap<String, FlowInstance>>,
        correlations: std::sync::Arc<DashMap<String, Vec<String>>>,
        flow_instances: std::sync::Arc<DashMap<String, Vec<String>>>,
    }

    impl MemoryFlowInstanceRepository {
        /// Create a new memory flow instance repository
        pub fn new() -> Self {
            Self {
                instances: std::sync::Arc::new(DashMap::with_capacity(64)),
                correlations: std::sync::Arc::new(DashMap::with_capacity(32)),
                flow_instances: std::sync::Arc::new(DashMap::with_capacity(16)),
            }
        }
    }

    impl Default for MemoryFlowInstanceRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl FlowInstanceRepository for MemoryFlowInstanceRepository {
        async fn find_by_id(&self, id: &FlowInstanceId) -> Result<Option<FlowInstance>, CoreError> {
            // Direct map lookup with no locks
            Ok(self.instances.get(&id.0).map(|instance| instance.clone()))
        }

        async fn save(&self, instance: &FlowInstance) -> Result<(), CoreError> {
            // Update instance
            self.instances
                .insert(instance.id.0.clone(), instance.clone());

            // Update flow_id index
            if let Some(mut instances) = self.flow_instances.get_mut(&instance.flow_id.0) {
                if !instances.contains(&instance.id.0) {
                    instances.push(instance.id.0.clone());
                }
            } else {
                self.flow_instances
                    .insert(instance.flow_id.0.clone(), vec![instance.id.0.clone()]);
            }

            // Update correlation index if correlation data exists
            if !instance.correlation_data.is_empty() {
                for correlation_id in instance.correlation_data.values() {
                    if let Some(mut instances) = self.correlations.get_mut(correlation_id) {
                        if !instances.contains(&instance.id.0) {
                            instances.push(instance.id.0.clone());
                        }
                    } else {
                        self.correlations
                            .insert(correlation_id.clone(), vec![instance.id.0.clone()]);
                    }
                }
            }

            Ok(())
        }

        async fn delete(&self, id: &FlowInstanceId) -> Result<(), CoreError> {
            // Find correlation data before removing the instance
            let correlation_data = if let Some(instance) = self.instances.get(&id.0) {
                instance.correlation_data.clone()
            } else {
                HashMap::new()
            };

            // Find flow_id before removing the instance
            let flow_id = if let Some(instance) = self.instances.get(&id.0) {
                instance.flow_id.0.clone()
            } else {
                return Ok(());
            };

            // Remove instance
            self.instances.remove(&id.0);

            // Update flow_id index
            if let Some(mut instances) = self.flow_instances.get_mut(&flow_id) {
                instances.retain(|i| i != &id.0);
            }

            // Update correlation index if correlation data exists
            if !correlation_data.is_empty() {
                for (_, correlation_id) in correlation_data {
                    if let Some(mut instances) = self.correlations.get_mut(&correlation_id) {
                        instances.retain(|i| i != &id.0);
                    }
                }
            }

            Ok(())
        }

        async fn find_by_correlation(
            &self,
            correlation_id: &CorrelationId,
        ) -> Result<Vec<FlowInstance>, CoreError> {
            let instances = if let Some(ids) = self.correlations.get(&correlation_id.0) {
                let mut result = Vec::new();

                for id in ids.iter() {
                    if let Some(instance) = self.instances.get(id) {
                        result.push(instance.clone());
                    }
                }

                result
            } else {
                Vec::new()
            };

            Ok(instances)
        }

        async fn find_all_for_flow(
            &self,
            flow_id: &FlowId,
        ) -> Result<Vec<FlowInstanceId>, CoreError> {
            let instance_ids = if let Some(ids) = self.flow_instances.get(&flow_id.0) {
                ids.iter().map(|id| FlowInstanceId(id.clone())).collect()
            } else {
                Vec::new()
            };

            Ok(instance_ids)
        }

        async fn list_instances(
            &self,
            flow_id: Option<&FlowId>,
            status: Option<&FlowStatus>,
        ) -> Result<Vec<FlowInstance>, CoreError> {
            let mut result = Vec::new();

            // If flow_id is provided, use the indexed lookup
            if let Some(flow_id) = flow_id {
                if let Some(instance_ids) = self.flow_instances.get(&flow_id.0) {
                    for id in instance_ids.iter() {
                        if let Some(instance) = self.instances.get(id) {
                            // Filter by status if provided
                            if let Some(status) = status {
                                if instance.status == *status {
                                    result.push(instance.clone());
                                }
                            } else {
                                result.push(instance.clone());
                            }
                        }
                    }
                }
            } else {
                // No flow_id, iterate all instances
                for instance in self.instances.iter() {
                    // Filter by status if provided
                    if let Some(status) = status {
                        if instance.status == *status {
                            result.push(instance.clone());
                        }
                    } else {
                        result.push(instance.clone());
                    }
                }
            }

            Ok(result)
        }
    }

    /// In-memory implementation of the flow definition repository
    pub struct MemoryFlowDefinitionRepository {
        definitions: std::sync::Arc<RwLock<HashMap<String, FlowDefinition>>>,
    }

    impl MemoryFlowDefinitionRepository {
        /// Create a new memory flow definition repository
        pub fn new() -> Self {
            Self {
                definitions: std::sync::Arc::new(RwLock::new(HashMap::new())),
            }
        }
    }

    impl Default for MemoryFlowDefinitionRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl FlowDefinitionRepository for MemoryFlowDefinitionRepository {
        async fn find_by_id(&self, id: &FlowId) -> Result<Option<FlowDefinition>, CoreError> {
            let definitions = self.definitions.read().map_err(|e| {
                CoreError::StateStoreError(format!("Failed to acquire read lock: {}", e))
            })?;

            Ok(definitions.get(&id.0).cloned())
        }

        async fn save(&self, definition: &FlowDefinition) -> Result<(), CoreError> {
            let mut definitions = self.definitions.write().map_err(|e| {
                CoreError::StateStoreError(format!("Failed to acquire write lock: {}", e))
            })?;

            definitions.insert(definition.id.0.clone(), definition.clone());

            Ok(())
        }

        async fn delete(&self, id: &FlowId) -> Result<(), CoreError> {
            let mut definitions = self.definitions.write().map_err(|e| {
                CoreError::StateStoreError(format!("Failed to acquire write lock: {}", e))
            })?;

            definitions.remove(&id.0);

            Ok(())
        }

        async fn list_definitions(&self) -> Result<Vec<FlowId>, CoreError> {
            let definitions = self.definitions.read().map_err(|e| {
                CoreError::StateStoreError(format!("Failed to acquire read lock: {}", e))
            })?;

            let result = definitions.keys().map(|id| FlowId(id.clone())).collect();

            Ok(result)
        }

        async fn find_all(&self) -> Result<Vec<FlowDefinition>, CoreError> {
            let definitions = self.definitions.read().map_err(|e| {
                CoreError::StateStoreError(format!("Failed to acquire read lock: {}", e))
            })?;

            let result = definitions.values().cloned().collect();

            Ok(result)
        }
    }

    /// In-memory implementation of the component state repository
    pub struct MemoryComponentStateRepository {
        states: std::sync::Arc<RwLock<HashMap<String, serde_json::Value>>>,
    }

    impl MemoryComponentStateRepository {
        /// Create a new memory component state repository
        pub fn new() -> Self {
            Self {
                states: std::sync::Arc::new(RwLock::new(HashMap::new())),
            }
        }
    }

    impl Default for MemoryComponentStateRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl ComponentStateRepository for MemoryComponentStateRepository {
        async fn get_state(
            &self,
            flow_instance_id: &FlowInstanceId,
            step_id: &StepId,
        ) -> Result<Option<serde_json::Value>, CoreError> {
            let key = format!("{}:{}", flow_instance_id.0, step_id.0);

            let states = self.states.read().map_err(|e| {
                CoreError::StateStoreError(format!("Failed to acquire read lock: {}", e))
            })?;

            Ok(states.get(&key).cloned())
        }

        async fn save_state(
            &self,
            flow_instance_id: &FlowInstanceId,
            step_id: &StepId,
            state: serde_json::Value,
        ) -> Result<(), CoreError> {
            let key = format!("{}:{}", flow_instance_id.0, step_id.0);

            let mut states = self.states.write().map_err(|e| {
                CoreError::StateStoreError(format!("Failed to acquire write lock: {}", e))
            })?;

            states.insert(key, state);

            Ok(())
        }

        async fn delete_state(
            &self,
            flow_instance_id: &FlowInstanceId,
            step_id: &StepId,
        ) -> Result<(), CoreError> {
            let key = format!("{}:{}", flow_instance_id.0, step_id.0);

            let mut states = self.states.write().map_err(|e| {
                CoreError::StateStoreError(format!("Failed to acquire write lock: {}", e))
            })?;

            states.remove(&key);

            Ok(())
        }
    }

    // Type aliases to reduce complexity
    type TimerMapEntry = (Instant, FlowInstanceId, StepId);
    type TimerMap = HashMap<String, TimerMapEntry>;
    type SharedTimerMap = std::sync::Arc<tokio::sync::RwLock<TimerMap>>;

    /// In-memory implementation of the timer repository
    pub struct MemoryTimerRepository {
        timers: SharedTimerMap,
        timer_tx: mpsc::Sender<(FlowInstanceId, StepId)>,
    }

    impl MemoryTimerRepository {
        /// Create a new memory timer repository
        pub fn new() -> (Self, mpsc::Receiver<(FlowInstanceId, StepId)>) {
            let (timer_tx, timer_rx) = mpsc::channel(32);

            let repo = Self {
                timers: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
                timer_tx,
            };

            // Start the timer processor
            let timers_ref = repo.timers.clone();
            let tx_ref = repo.timer_tx.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(50)).await;

                    let now = Instant::now();
                    let mut expired_timers = Vec::new();

                    // Find expired timers
                    {
                        let timers_map = timers_ref.read().await;
                        for (id, (expire_time, flow_id, step_id)) in timers_map.iter() {
                            if *expire_time <= now {
                                expired_timers.push((id.clone(), flow_id.clone(), step_id.clone()));
                            }
                        }
                    }

                    // Process expired timers
                    if !expired_timers.is_empty() {
                        let mut timers_map = timers_ref.write().await;
                        for (id, flow_id, step_id) in expired_timers {
                            timers_map.remove(&id);
                            if tx_ref.send((flow_id, step_id)).await.is_err() {
                                // Timer channel closed, likely shutdown
                                return;
                            }
                        }
                    }
                }
            });

            (repo, timer_rx)
        }
    }

    #[async_trait]
    impl TimerRepository for MemoryTimerRepository {
        async fn schedule(
            &self,
            flow_instance_id: &FlowInstanceId,
            step_id: &StepId,
            duration: Duration,
        ) -> Result<String, CoreError> {
            // Generate a timer ID
            let timer_id = uuid::Uuid::new_v4().to_string();

            // Calculate expiry time
            let expires_at = Instant::now() + duration;

            // Store the timer
            let mut timers = self.timers.write().await;
            timers.insert(
                timer_id.clone(),
                (expires_at, flow_instance_id.clone(), step_id.clone()),
            );

            Ok(timer_id)
        }

        async fn cancel(&self, timer_id: &str) -> Result<(), CoreError> {
            // Remove the timer
            let mut timers = self.timers.write().await;
            timers.remove(timer_id);

            Ok(())
        }
    }
}

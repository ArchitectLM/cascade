use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::sync::mpsc::Sender;
use tracing::{debug, warn};
use async_trait::async_trait;
use uuid::Uuid;

use cascade_core::{
    CoreError,
    domain::flow_instance::{FlowInstance, FlowInstanceId, FlowId, StepId, CorrelationId, FlowStatus},
    domain::flow_definition::FlowDefinition,
    domain::repository::{
        FlowInstanceRepository, 
        FlowDefinitionRepository,
        ComponentStateRepository, 
        TimerRepository
    },
    TimerProcessingRepository
};

/// Event structure for timer firing
pub struct TimerEvent {
    /// Timer ID
    pub timer_id: String,
    /// Flow instance ID
    pub flow_instance_id: FlowInstanceId,
    /// Step ID
    pub step_id: StepId,
}

/// Correlation repository trait for storing correlation IDs
#[async_trait]
pub trait CorrelationRepository: Send + Sync {
    /// Save a correlation ID for a flow instance
    async fn save_correlation(&self, correlation_id: &CorrelationId, instance_id: &FlowInstanceId) -> Result<(), CoreError>;
    
    /// Delete a correlation ID
    async fn delete_correlation(&self, correlation_id: &CorrelationId) -> Result<(), CoreError>;
    
    /// Get a flow instance ID for a correlation ID
    async fn get_instance_id(&self, correlation_id: &CorrelationId) -> Result<Option<FlowInstanceId>, CoreError>;
}

/// In-memory implementation of the FlowInstanceRepository
pub struct InMemoryFlowInstanceRepository {
    instances: Arc<RwLock<HashMap<String, FlowInstance>>>,
    correlations: Arc<RwLock<HashMap<String, String>>>,
}

impl InMemoryFlowInstanceRepository {
    /// Create a new in-memory flow instance repository
    pub fn new(
        instances: Arc<RwLock<HashMap<String, FlowInstance>>>,
        correlations: Arc<RwLock<HashMap<String, String>>>,
    ) -> Self {
        Self { instances, correlations }
    }
}

#[async_trait]
impl FlowInstanceRepository for InMemoryFlowInstanceRepository {
    async fn find_by_id(&self, id: &FlowInstanceId) -> Result<Option<FlowInstance>, CoreError> {
        let instances = self.instances.read().await;
        Ok(instances.get(&id.0).cloned())
    }
    
    async fn save(&self, instance: &FlowInstance) -> Result<(), CoreError> {
        let mut instances = self.instances.write().await;
        instances.insert(instance.id.0.clone(), instance.clone());
        Ok(())
    }
    
    async fn delete(&self, id: &FlowInstanceId) -> Result<(), CoreError> {
        let mut instances = self.instances.write().await;
        instances.remove(&id.0);
        Ok(())
    }
    
    async fn find_by_correlation(&self, correlation_id: &CorrelationId) -> Result<Vec<FlowInstance>, CoreError> {
        let correlations = self.correlations.read().await;
        let instance_id = correlations.get(&correlation_id.0).cloned();
        
        if let Some(instance_id) = instance_id {
            let instances = self.instances.read().await;
            let instance = instances.get(&instance_id).cloned();
            if let Some(instance) = instance {
                Ok(vec![instance])
            } else {
                Ok(vec![])
            }
        } else {
            Ok(vec![])
        }
    }
    
    async fn find_all_for_flow(&self, flow_id: &FlowId) -> Result<Vec<FlowInstanceId>, CoreError> {
        let instances = self.instances.read().await;
        
        let matching_instances = instances
            .iter()
            .filter(|(_, instance)| instance.flow_id.0 == flow_id.0)
            .map(|(_, instance)| instance.id.clone())
            .collect();
        
        Ok(matching_instances)
    }
    
    async fn list_instances(
        &self,
        flow_id: Option<&FlowId>,
        status: Option<&FlowStatus>
    ) -> Result<Vec<FlowInstance>, CoreError> {
        let instances = self.instances.read().await;
        
        let result = instances.values()
            .filter(|instance| {
                // Apply flow_id filter if present
                let flow_match = match flow_id {
                    Some(id) => instance.flow_id.0 == id.0,
                    None => true,
                };
                
                // Apply status filter if present
                let status_match = match status {
                    Some(s) => instance.status == *s,
                    None => true,
                };
                
                flow_match && status_match
            })
            .cloned()
            .collect();
            
        Ok(result)
    }
}

/// In-memory implementation of the CorrelationRepository
pub struct InMemoryCorrelationRepository {
    correlations: Arc<RwLock<HashMap<String, String>>>,
}

impl InMemoryCorrelationRepository {
    /// Create a new in-memory correlation repository
    pub fn new(correlations: Arc<RwLock<HashMap<String, String>>>) -> Self {
        Self { correlations }
    }
}

#[async_trait]
impl CorrelationRepository for InMemoryCorrelationRepository {
    async fn save_correlation(&self, correlation_id: &CorrelationId, instance_id: &FlowInstanceId) -> Result<(), CoreError> {
        let mut correlations = self.correlations.write().await;
        correlations.insert(correlation_id.0.clone(), instance_id.0.clone());
        Ok(())
    }
    
    async fn delete_correlation(&self, correlation_id: &CorrelationId) -> Result<(), CoreError> {
        let mut correlations = self.correlations.write().await;
        correlations.remove(&correlation_id.0);
        Ok(())
    }
    
    async fn get_instance_id(&self, correlation_id: &CorrelationId) -> Result<Option<FlowInstanceId>, CoreError> {
        let correlations = self.correlations.read().await;
        Ok(correlations.get(&correlation_id.0)
           .map(|id| FlowInstanceId(id.clone())))
    }
}

/// In-memory implementation of the FlowDefinitionRepository
pub struct InMemoryFlowDefinitionRepository {
    definitions: Arc<RwLock<HashMap<String, FlowDefinition>>>,
}

impl InMemoryFlowDefinitionRepository {
    /// Create a new in-memory flow definition repository
    pub fn new(definitions: Arc<RwLock<HashMap<String, FlowDefinition>>>) -> Self {
        Self { definitions }
    }
}

#[async_trait]
impl FlowDefinitionRepository for InMemoryFlowDefinitionRepository {
    async fn find_by_id(&self, id: &FlowId) -> Result<Option<FlowDefinition>, CoreError> {
        let definitions = self.definitions.read().await;
        Ok(definitions.get(&id.0).cloned())
    }
    
    async fn save(&self, definition: &FlowDefinition) -> Result<(), CoreError> {
        let mut definitions = self.definitions.write().await;
        definitions.insert(definition.id.0.clone(), definition.clone());
        Ok(())
    }
    
    async fn delete(&self, id: &FlowId) -> Result<(), CoreError> {
        let mut definitions = self.definitions.write().await;
        definitions.remove(&id.0);
        Ok(())
    }
    
    async fn find_all(&self) -> Result<Vec<FlowDefinition>, CoreError> {
        let definitions = self.definitions.read().await;
        Ok(definitions.values().cloned().collect())
    }
    
    async fn list_definitions(&self) -> Result<Vec<FlowId>, CoreError> {
        let definitions = self.definitions.read().await;
        let flow_ids = definitions.keys()
            .map(|key| FlowId(key.clone()))
            .collect();
        Ok(flow_ids)
    }
}

/// In-memory implementation of the ComponentStateRepository
pub struct InMemoryComponentStateRepository {
    states: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl InMemoryComponentStateRepository {
    /// Create a new in-memory component state repository
    pub fn new(states: Arc<RwLock<HashMap<String, serde_json::Value>>>) -> Self {
        Self { states }
    }
    
    fn make_key(flow_instance_id: &FlowInstanceId, step_id: &StepId) -> String {
        format!("{}:{}", flow_instance_id.0, step_id.0)
    }
}

#[async_trait]
impl ComponentStateRepository for InMemoryComponentStateRepository {
    async fn get_state(
        &self, 
        flow_instance_id: &FlowInstanceId, 
        step_id: &StepId
    ) -> Result<Option<serde_json::Value>, CoreError> {
        let key = Self::make_key(flow_instance_id, step_id);
        let states = self.states.read().await;
        Ok(states.get(&key).cloned())
    }
    
    async fn save_state(
        &self, 
        flow_instance_id: &FlowInstanceId, 
        step_id: &StepId, 
        state: serde_json::Value
    ) -> Result<(), CoreError> {
        let key = Self::make_key(flow_instance_id, step_id);
        let mut states = self.states.write().await;
        states.insert(key, state);
        Ok(())
    }
    
    async fn delete_state(
        &self, 
        flow_instance_id: &FlowInstanceId, 
        step_id: &StepId
    ) -> Result<(), CoreError> {
        let key = Self::make_key(flow_instance_id, step_id);
        let mut states = self.states.write().await;
        states.remove(&key);
        Ok(())
    }
}

/// In-memory implementation of the TimerRepository
pub struct InMemoryTimerRepository {
    timer_tx: Sender<TimerEvent>,
}

impl InMemoryTimerRepository {
    /// Create a new in-memory timer repository
    pub fn new(timer_tx: Sender<TimerEvent>) -> Self {
        Self { timer_tx }
    }
}

#[async_trait]
impl TimerRepository for InMemoryTimerRepository {
    async fn schedule(
        &self, 
        flow_instance_id: &FlowInstanceId, 
        step_id: &StepId, 
        duration: Duration
    ) -> Result<String, CoreError> {
        // Generate a unique ID for the timer
        let timer_id = Uuid::new_v4().to_string();
        
        // Calculate when the timer should fire
        let now = std::time::Instant::now();
        let fire_at = now + duration;
        
        // Clone values for the spawned task
        let timer_tx = self.timer_tx.clone();
        let flow_id_clone = flow_instance_id.clone();
        let step_id_clone = step_id.clone();
        let timer_id_clone = timer_id.clone();
        
        // Spawn a task to send the timer event when the duration elapses
        tokio::spawn(async move {
            // Sleep until the timer should fire
            tokio::time::sleep_until(tokio::time::Instant::from_std(fire_at)).await;
            
            // Create and send the timer event
            let event = TimerEvent {
                timer_id: timer_id_clone,
                flow_instance_id: flow_id_clone,
                step_id: step_id_clone,
            };
            
            if let Err(e) = timer_tx.send(event).await {
                warn!("Failed to send timer event: {}", e);
            }
        });
        
        Ok(timer_id)
    }
    
    async fn cancel(&self, _timer_id: &str) -> Result<(), CoreError> {
        // In this simple implementation, we don't actually cancel the timer
        // The timer will still fire, but it will be ignored if the flow instance
        // or step has moved on
        
        // In a more sophisticated implementation, we would keep track of all
        // timers and be able to cancel them
        
        Ok(())
    }
}

// Implementation of TimerProcessingRepository for InMemoryTimerRepository
impl TimerProcessingRepository for InMemoryTimerRepository {
    fn start_timer_processing(
        &self,
        _callback: Arc<dyn Fn(FlowInstanceId, StepId) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>,
    ) {
        // In-memory repository doesn't need this since timers are handled via tokio tasks
        // that directly send to the timer channel
        debug!("InMemoryTimerRepository.start_timer_processing called (no-op)");
    }
} 
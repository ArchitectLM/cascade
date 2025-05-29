//! In-memory state store implementation for the Cascade Platform
//!
//! This crate provides in-memory implementations of the core repository
//! interfaces defined in the cascade-core crate. It is primarily useful for 
//! development, testing, and simple deployments where persistence is not required.

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::debug;

pub mod repositories;
pub use repositories::{
    InMemoryFlowInstanceRepository,
    InMemoryFlowDefinitionRepository,
    InMemoryComponentStateRepository,
    InMemoryTimerRepository,
    TimerEvent,
};

pub mod shared_state;
pub use shared_state::InMemorySharedStateService;

use cascade_core::{
    domain::{
        flow_instance::{
            FlowInstanceId, StepId
        },
    },
    domain::repository::{
        FlowInstanceRepository, FlowDefinitionRepository, 
        ComponentStateRepository, TimerRepository
    },
    CoreError,
};

/// Provider for in-memory state store repositories
pub struct InMemoryStateStoreProvider {
    // Shared storage for flow instances
    flow_instances: Arc<RwLock<HashMap<String, cascade_core::domain::flow_instance::FlowInstance>>>,
    
    // Shared storage for flow definitions
    flow_definitions: Arc<RwLock<HashMap<String, cascade_core::domain::flow_definition::FlowDefinition>>>,
    
    // Shared storage for component states
    component_states: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    
    // Shared storage for correlation IDs
    correlations: Arc<RwLock<HashMap<String, String>>>,
    
    // Channel for timer events
    timer_tx: Option<tokio::sync::mpsc::Sender<TimerEvent>>,
    timer_rx: Option<tokio::sync::mpsc::Receiver<TimerEvent>>,
}

impl InMemoryStateStoreProvider {
    /// Create a new in-memory state store provider
    pub fn new() -> Self {
        // Create channels for timer events
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        
        Self {
            flow_instances: Arc::new(RwLock::new(HashMap::new())),
            flow_definitions: Arc::new(RwLock::new(HashMap::new())),
            component_states: Arc::new(RwLock::new(HashMap::new())),
            correlations: Arc::new(RwLock::new(HashMap::new())),
            timer_tx: Some(tx),
            timer_rx: Some(rx),
        }
    }
    
    /// Create repositories for use with RuntimeInterface
    pub fn create_repositories(&self) -> (
        Arc<dyn FlowInstanceRepository>,
        Arc<dyn FlowDefinitionRepository>,
        Arc<dyn ComponentStateRepository>,
        Arc<dyn TimerRepository>,
        Option<Box<dyn Fn(FlowInstanceId, StepId) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>> + Send + Sync>>
    ) {
        let flow_instance_repo = Arc::new(InMemoryFlowInstanceRepository::new(
            self.flow_instances.clone(),
            self.correlations.clone(),
        ));
        
        let flow_definition_repo = Arc::new(InMemoryFlowDefinitionRepository::new(
            self.flow_definitions.clone(),
        ));
        
        let component_state_repo = Arc::new(InMemoryComponentStateRepository::new(
            self.component_states.clone(),
        ));
        
        let timer_repo = Arc::new(InMemoryTimerRepository::new(
            self.timer_tx.clone().unwrap(),
        ));
        
        // This callback is typically unused for in-memory repositories
        // since timers are handled directly via tokio tasks
        let timer_callback: Box<dyn Fn(FlowInstanceId, StepId) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>> + Send + Sync> = Box::new(move |flow_id, step_id| {
            Box::pin(async move {
                debug!("Timer callback for flow: {}, step: {}", flow_id.0, step_id.0);
                // The actual timer callback is no-op for in-memory repositories
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
    
    /// Start the timer processor with a custom callback
    /// 
    /// This method sets up a task that listens for timer events from the internal 
    /// timer channel and calls the provided callback when a timer fires.
    pub fn start_timer_processor<F, Fut>(&mut self, callback: F) -> Result<(), CoreError>
    where
        F: Fn((FlowInstanceId, StepId)) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), CoreError>> + Send + 'static,
    {
        // Take ownership of the receiver
        let mut rx = self.timer_rx.take().ok_or_else(|| {
            CoreError::StateStoreError("Timer processor already started".to_string())
        })?;
        
        // Start a task to process timer events
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                // Convert TimerEvent to the tuple format expected by the callback
                let event_tuple = (event.flow_instance_id, event.step_id);
                
                // Call the callback with the timer event
                if let Err(e) = callback(event_tuple).await {
                    tracing::error!("Error processing timer event: {}", e);
                }
            }
        });
        
        Ok(())
    }
}

impl Default for InMemoryStateStoreProvider {
    fn default() -> Self {
        Self::new()
    }
} 
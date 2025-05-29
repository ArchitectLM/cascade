use crate::{
    domain::events::{
        DomainEvent, FlowInstanceCompleted, FlowInstanceCreated, FlowInstanceFailed, StepCompleted,
        StepFailed,
    },
    CoreError, DataPacket,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Flow instance status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowStatus {
    /// Flow is initializing
    Initializing,

    /// Flow is currently running
    Running,

    /// Flow is waiting for an external event
    WaitingForEvent,

    /// Flow is waiting for a timer to expire
    WaitingForTimer,

    /// Flow has completed successfully
    Completed,

    /// Flow failed
    Failed,
}

/// Aggregate: Flow instance
#[derive(Debug, Serialize, Deserialize)]
pub struct FlowInstance {
    /// Unique identifier
    pub id: FlowInstanceId,

    /// Flow definition ID
    pub flow_id: FlowId,

    /// Current status
    pub status: FlowStatus,

    /// Step outputs keyed by step_id.output_name
    pub step_outputs: HashMap<StepId, DataPacket>,

    /// Trigger data that started the flow
    pub trigger_data: DataPacket,

    /// Completed steps
    pub completed_steps: HashSet<StepId>,

    /// Failed steps
    pub failed_steps: HashSet<StepId>,

    /// Error messages for failed steps
    pub failed_step_errors: HashMap<StepId, String>,

    /// Step IDs with pending timers
    pub pending_timers: HashMap<StepId, String>,

    /// Correlation IDs for waiting steps
    pub correlation_data: HashMap<StepId, String>,

    /// Error message if flow failed
    pub error: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,

    /// Domain events
    #[serde(skip)]
    pub events: Vec<Box<dyn DomainEvent>>,
}

// Manually implement Clone for FlowInstance
impl Clone for FlowInstance {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            flow_id: self.flow_id.clone(),
            status: self.status,
            step_outputs: self.step_outputs.clone(),
            trigger_data: self.trigger_data.clone(),
            completed_steps: self.completed_steps.clone(),
            failed_steps: self.failed_steps.clone(),
            failed_step_errors: self.failed_step_errors.clone(),
            pending_timers: self.pending_timers.clone(),
            correlation_data: self.correlation_data.clone(),
            error: self.error.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            events: Vec::new(), // We don't clone domain events
        }
    }
}

/// Value object: Flow Instance ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlowInstanceId(pub String);

/// Value object: Flow ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlowId(pub String);

/// Value object: Step ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StepId(pub String);

/// Value object: Correlation ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(pub String);

impl FlowInstance {
    /// Create a new flow instance
    pub fn new(flow_id: FlowId, trigger_data: DataPacket) -> Self {
        let instance_id = FlowInstanceId(Uuid::new_v4().to_string());
        let now = Utc::now();

        let mut instance = Self {
            id: instance_id.clone(),
            flow_id: flow_id.clone(),
            status: FlowStatus::Initializing,
            step_outputs: HashMap::with_capacity(16), // Pre-allocate for better performance
            trigger_data,
            completed_steps: HashSet::with_capacity(16),
            failed_steps: HashSet::new(),
            failed_step_errors: HashMap::new(),
            pending_timers: HashMap::new(),
            correlation_data: HashMap::new(),
            error: None,
            created_at: now,
            updated_at: now,
            events: Vec::with_capacity(8),
        };

        // Add creation event
        instance.record_event(Box::new(FlowInstanceCreated {
            flow_instance_id: instance_id,
            flow_id,
            timestamp: now,
        }));

        instance
    }

    /// Start the flow execution
    #[inline]
    pub fn start(&mut self) -> Result<(), CoreError> {
        if self.status != FlowStatus::Initializing {
            return Err(CoreError::FlowExecutionError(format!(
                "Cannot start flow in state: {:?}",
                self.status
            )));
        }

        self.status = FlowStatus::Running;
        self.update_timestamp();

        // Record event
        self.record_event(Box::new(FlowInstanceCreated {
            flow_instance_id: self.id.clone(),
            flow_id: self.flow_id.clone(),
            timestamp: Utc::now(),
        }));

        Ok(())
    }

    /// Update the timestamp
    #[inline]
    pub fn update_timestamp(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Complete a step and add its outputs
    pub fn complete_step(
        &mut self,
        step_id: StepId,
        outputs: HashMap<String, DataPacket>,
    ) -> Result<(), CoreError> {
        if self.status != FlowStatus::Running {
            return Err(CoreError::FlowExecutionError(format!(
                "Cannot complete step while flow is in state: {:?}",
                self.status
            )));
        }

        // Add step outputs - avoid unnecessary clones
        for (output_name, output) in outputs {
            let output_id = StepId(format!("{}.{}", step_id.0, output_name));
            self.step_outputs.insert(output_id, output);
        }

        // Mark step as completed
        self.completed_steps.insert(step_id.clone());

        // Record event
        self.record_event(Box::new(StepCompleted {
            flow_instance_id: self.id.clone(),
            step_id,
            timestamp: Utc::now(),
        }));

        self.update_timestamp();
        Ok(())
    }

    /// Get a step output value
    #[inline]
    pub fn get_step_output(&self, step_id: &str, output_name: &str) -> Option<&DataPacket> {
        let output_id = StepId(format!("{}.{}", step_id, output_name));
        self.step_outputs.get(&output_id)
    }

    /// Check if a step is completed
    #[inline]
    pub fn is_step_completed(&self, step_id: &StepId) -> bool {
        self.completed_steps.contains(step_id)
    }

    /// Check if all steps are completed
    #[inline]
    pub fn all_steps_completed(&self, all_step_ids: &HashSet<StepId>) -> bool {
        // Check if completed_steps contains all steps
        all_step_ids
            .iter()
            .all(|step_id| self.completed_steps.contains(step_id))
    }

    /// Check if flow has any failed steps
    #[inline]
    pub fn has_failed_steps(&self) -> bool {
        !self.failed_steps.is_empty()
    }

    /// Mark a step as failed
    pub fn fail_step(&mut self, step_id: StepId, error: String) -> Result<(), CoreError> {
        if self.status != FlowStatus::Running {
            return Err(CoreError::FlowExecutionError(format!(
                "Cannot fail step while flow is in state: {:?}",
                self.status
            )));
        }

        // Mark step as failed
        self.failed_steps.insert(step_id.clone());

        // Store error message
        self.failed_step_errors
            .insert(step_id.clone(), error.clone());

        // Record event
        self.record_event(Box::new(StepFailed {
            flow_instance_id: self.id.clone(),
            step_id,
            error,
            timestamp: Utc::now(),
        }));

        self.update_timestamp();
        Ok(())
    }

    /// Set pending timer for a step
    pub fn suspend_for_timer(&mut self, step_id: StepId, timer_id: &str) -> Result<(), CoreError> {
        if self.status != FlowStatus::Running {
            return Err(CoreError::FlowExecutionError(format!(
                "Cannot suspend flow in state: {:?}",
                self.status
            )));
        }

        self.pending_timers.insert(step_id.clone(), timer_id.to_string());
        self.status = FlowStatus::WaitingForTimer;
        
        // Record event
        self.record_event(Box::new(crate::domain::events::FlowInstanceSuspendedForTimer {
            flow_instance_id: self.id.clone(),
            step_id,
            timer_id: timer_id.to_string(),
            timestamp: Utc::now(),
        }));
        
        self.update_timestamp();
        Ok(())
    }

    /// Set correlation ID for an external event
    pub fn suspend_for_event(
        &mut self,
        step_id: StepId,
        correlation_id: CorrelationId,
    ) -> Result<(), CoreError> {
        if self.status != FlowStatus::Running {
            return Err(CoreError::FlowExecutionError(format!(
                "Cannot suspend flow in state: {:?}",
                self.status
            )));
        }

        self.correlation_data.insert(step_id.clone(), correlation_id.0.clone());
        self.status = FlowStatus::WaitingForEvent;
        
        // Record event
        self.record_event(Box::new(crate::domain::events::FlowInstanceSuspendedForEvent {
            flow_instance_id: self.id.clone(),
            step_id,
            correlation_id: correlation_id.0,
            timestamp: Utc::now(),
        }));
        
        self.update_timestamp();
        Ok(())
    }

    /// Resume a suspended flow
    pub fn resume(&mut self) -> Result<(), CoreError> {
        if self.status != FlowStatus::WaitingForTimer && self.status != FlowStatus::WaitingForEvent
        {
            return Err(CoreError::FlowExecutionError(format!(
                "Cannot resume flow in state: {:?}",
                self.status
            )));
        }

        self.status = FlowStatus::Running;
        
        // Record event
        self.record_event(Box::new(crate::domain::events::FlowInstanceResumed {
            flow_instance_id: self.id.clone(),
            timestamp: Utc::now(),
        }));
        
        self.update_timestamp();
        Ok(())
    }

    /// Complete the flow successfully
    pub fn complete(&mut self) -> Result<(), CoreError> {
        if self.status != FlowStatus::Running {
            return Err(CoreError::FlowExecutionError(format!(
                "Cannot complete flow in state: {:?}",
                self.status
            )));
        }

        self.status = FlowStatus::Completed;

        // Record event
        self.record_event(Box::new(FlowInstanceCompleted {
            flow_instance_id: self.id.clone(),
            timestamp: Utc::now(),
        }));

        self.update_timestamp();
        Ok(())
    }

    /// Set the flow as failed
    pub fn fail(&mut self, error: String) -> Result<(), CoreError> {
        if self.status == FlowStatus::Completed || self.status == FlowStatus::Failed {
            return Err(CoreError::FlowExecutionError(format!(
                "Cannot fail flow in state: {:?}",
                self.status
            )));
        }
        
        self.status = FlowStatus::Failed;
        self.error = Some(error.clone());

        // Record event
        self.record_event(Box::new(FlowInstanceFailed {
            flow_instance_id: self.id.clone(),
            error,
            timestamp: Utc::now(),
        }));

        self.update_timestamp();
        Ok(())
    }

    /// Record a domain event
    pub fn record_event(&mut self, event: Box<dyn DomainEvent>) {
        self.events.push(event);
    }

    /// Get and clear all domain events
    pub fn take_events(&mut self) -> Vec<Box<dyn DomainEvent>> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn test_flow_instance_creation() {
        let flow_id = FlowId("test_flow".to_string());
        let trigger_data = DataPacket::new(json!({"input": "value"}));
        
        let instance = FlowInstance::new(flow_id.clone(), trigger_data.clone());
        
        assert_eq!(instance.flow_id, flow_id);
        assert_eq!(instance.status, FlowStatus::Initializing);
        assert!(instance.step_outputs.is_empty());
        assert!(instance.failed_step_errors.is_empty());
        assert!(instance.completed_steps.is_empty());
        assert!(instance.failed_steps.is_empty());
        assert!(instance.pending_timers.is_empty());
        assert!(instance.correlation_data.is_empty());
        
        // Check that the instance ID was generated
        assert!(!instance.id.0.is_empty());
        
        // Verify creation timestamp
        assert!(instance.created_at <= chrono::Utc::now());
        
        // Check trigger data
        assert_eq!(*instance.trigger_data.as_value(), *trigger_data.as_value());
    }

    #[test]
    fn test_flow_status_transitions() {
        let flow_id = FlowId("status_test".to_string());
        let trigger_data = DataPacket::new(json!({"test": true}));
        
        let mut instance = FlowInstance::new(flow_id, trigger_data);
        
        // Initial state
        assert_eq!(instance.status, FlowStatus::Initializing);
        
        // Start the flow
        instance.status = FlowStatus::Running;
        assert_eq!(instance.status, FlowStatus::Running);
        
        // Complete the flow
        instance.status = FlowStatus::Completed;
        assert_eq!(instance.status, FlowStatus::Completed);
        
        // Create a new instance for error path
        let mut error_instance = FlowInstance::new(
            FlowId("error_flow".to_string()),
            DataPacket::new(json!({"test": "error"}))
        );
        
        // Start the flow
        error_instance.status = FlowStatus::Running;
        assert_eq!(error_instance.status, FlowStatus::Running);
        
        // Fail the flow
        error_instance.status = FlowStatus::Failed;
        assert_eq!(error_instance.status, FlowStatus::Failed);
        
        // Check serialization of status
        let running_json = serde_json::to_string(&FlowStatus::Running).unwrap();
        let deserialized: FlowStatus = serde_json::from_str(&running_json).unwrap();
        assert_eq!(deserialized, FlowStatus::Running);
    }

    #[test]
    fn test_step_tracking() {
        let flow_id = FlowId("step_test".to_string());
        let trigger_data = DataPacket::new(json!({"test": true}));
        
        let mut instance = FlowInstance::new(flow_id, trigger_data);
        
        // Track steps through their lifecycle
        let step1_id = StepId("step1".to_string());
        let step2_id = StepId("step2".to_string());
        let step3_id = StepId("step3".to_string());
        
        // Mark step1 as running
        instance.completed_steps.insert(step1_id.clone());
        assert_eq!(instance.completed_steps.len(), 1);
        assert!(instance.completed_steps.contains(&step1_id));
        
        // Mark step2 as running
        instance.completed_steps.insert(step2_id.clone());
        assert_eq!(instance.completed_steps.len(), 2);
        
        // Complete step1
        instance.completed_steps.remove(&step1_id);
        assert_eq!(instance.completed_steps.len(), 1);
        
        // Add step output for step1
        let output_key = StepId("step1.result".to_string());
        let output_value = DataPacket::new(json!({"success": true, "value": 42}));
        instance.step_outputs.insert(output_key, output_value);
        
        // Fail step2
        instance.failed_steps.insert(step2_id.clone());
        assert_eq!(instance.failed_steps.len(), 1);
        
        // Add error for step2
        let error = crate::CoreError::ComponentError("Test error".to_string());
        instance.failed_step_errors.insert(step2_id.clone(), error.to_string());
        
        // Mark step3 as pending
        let pending_reason = "Waiting for external event".to_string();
        instance.pending_timers.insert(step3_id.clone(), pending_reason.clone());
        assert_eq!(instance.pending_timers.len(), 1);
        assert_eq!(instance.pending_timers.get(&step3_id).unwrap(), &pending_reason);
    }

    #[test]
    fn test_flow_instance_serialization() {
        let flow_id = FlowId("serialize_test".to_string());
        let trigger_data = DataPacket::new(json!({"input": "serialize"}));
        
        let mut instance = FlowInstance::new(flow_id, trigger_data);
        
        // Add some data to the instance
        let step_id = StepId("test_step".to_string());
        instance.completed_steps.insert(step_id.clone());
        
        let output_key = StepId("test_step.result".to_string());
        let output_value = DataPacket::new(json!({"success": true}));
        instance.step_outputs.insert(output_key, output_value);
        
        // Set a correlation ID
        instance.correlation_data.insert(step_id.clone(), "test-correlation".to_string());
        
        // Serialize and deserialize
        let serialized = serde_json::to_string(&instance).unwrap();
        let deserialized: FlowInstance = serde_json::from_str(&serialized).unwrap();
        
        // Verify key fields
        assert_eq!(deserialized.id, instance.id);
        assert_eq!(deserialized.flow_id, instance.flow_id);
        assert_eq!(deserialized.status, instance.status);
        assert_eq!(deserialized.correlation_data.get(&step_id).unwrap(), "test-correlation");
        assert_eq!(deserialized.completed_steps.len(), 1);
        assert!(deserialized.completed_steps.contains(&step_id));
        
        // Check step outputs
        assert_eq!(deserialized.step_outputs.len(), 1);
        let deserialize_output_key = StepId("test_step.result".to_string());
        assert!(deserialized.step_outputs.contains_key(&deserialize_output_key));
    }

    #[test]
    fn test_correlation_id() {
        // Test creation
        let correlation_id = CorrelationId("test-correlation-123".to_string());
        assert_eq!(correlation_id.0, "test-correlation-123");
        
        // Test serialization
        let serialized = serde_json::to_string(&correlation_id).unwrap();
        let deserialized: CorrelationId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, correlation_id);
    }

    #[test]
    fn test_flow_id() {
        // Test creation
        let flow_id = FlowId("test-flow-123".to_string());
        assert_eq!(flow_id.0, "test-flow-123");
        
        // Test serialization
        let serialized = serde_json::to_string(&flow_id).unwrap();
        let deserialized: FlowId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, flow_id);
        
        // Test Display implementation if available, or just string conversion
        let flow_id_str = format!("{:?}", flow_id);
        assert!(flow_id_str.contains("test-flow-123"));
    }

    #[test]
    fn test_step_id() {
        // Test creation
        let step_id = StepId("test-step-123".to_string());
        assert_eq!(step_id.0, "test-step-123");
        
        // Test serialization
        let serialized = serde_json::to_string(&step_id).unwrap();
        let deserialized: StepId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, step_id);
        
        // Test Debug implementation
        let step_id_str = format!("{:?}", step_id);
        assert!(step_id_str.contains("test-step-123"));
    }

    #[test]
    fn test_step_output_key() {
        // Test creation
        let step_id = StepId("output-step".to_string());
        let output_key = StepId("output-step.result".to_string());
        
        // They should be different because one has the output name appended
        assert_ne!(output_key, step_id);
        assert!(output_key.0.starts_with(&step_id.0));
        assert!(output_key.0.contains(".result"));
        
        // Test serialization
        let serialized = serde_json::to_string(&output_key).unwrap();
        let deserialized: StepId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, output_key);
    }

    #[test]
    fn test_fail_step() {
        // Create a running flow instance
        let mut instance = create_running_flow_instance();
        let step_id = StepId("step1".to_string());
        
        // Fail the step
        let result = instance.fail_step(step_id.clone(), "Test failure".to_string());
        assert!(result.is_ok());
        
        // Check state changes
        assert!(instance.failed_steps.contains(&step_id));
        assert_eq!(
            instance.failed_step_errors.get(&step_id).unwrap(),
            "Test failure"
        );
        
        // Check that events were recorded
        assert!(!instance.events.is_empty());
        // Clear events for the next test
        instance.take_events();
        
        // Try to fail the same step again - should be ok
        let result = instance.fail_step(step_id.clone(), "Another failure".to_string());
        assert!(result.is_ok());
        assert_eq!(
            instance.failed_step_errors.get(&step_id).unwrap(),
            "Another failure"
        );
    }
    
    #[test]
    fn test_fail_step_invalid_state() {
        // Create a completed flow instance
        let mut instance = create_running_flow_instance();
        instance.status = FlowStatus::Completed;
        
        // Attempt to fail a step while the flow is completed
        let step_id = StepId("step1".to_string());
        let result = instance.fail_step(step_id, "Test failure".to_string());
        
        // This should fail because the flow is not in Running state
        assert!(result.is_err());
        match result {
            Err(CoreError::FlowExecutionError(msg)) => {
                assert!(msg.contains("Cannot fail step while flow is in state"));
            }
            _ => panic!("Expected FlowExecutionError"),
        }
    }
    
    #[test]
    fn test_suspend_for_timer() {
        // Create a running flow instance
        let mut instance = create_running_flow_instance();
        let step_id = StepId("timer_step".to_string());
        let timer_id = "timer-123";
        
        // Suspend for timer
        let result = instance.suspend_for_timer(step_id.clone(), timer_id);
        assert!(result.is_ok());
        
        // Check state changes
        assert_eq!(instance.status, FlowStatus::WaitingForTimer);
        assert_eq!(instance.pending_timers.get(&step_id).unwrap(), timer_id);
        
        // Check that events were recorded
        assert!(!instance.events.is_empty());
    }
    
    #[test]
    fn test_suspend_for_timer_invalid_state() {
        // Create a waiting flow instance
        let mut instance = create_running_flow_instance();
        instance.status = FlowStatus::WaitingForEvent;
        
        // Attempt to suspend for timer when already waiting
        let step_id = StepId("timer_step".to_string());
        let result = instance.suspend_for_timer(step_id, "timer-123");
        
        // This should fail because the flow is not in Running state
        assert!(result.is_err());
        match result {
            Err(CoreError::FlowExecutionError(msg)) => {
                assert!(msg.contains("Cannot suspend flow in state"));
            }
            _ => panic!("Expected FlowExecutionError"),
        }
    }
    
    #[test]
    fn test_suspend_for_event() {
        // Create a running flow instance
        let mut instance = create_running_flow_instance();
        let step_id = StepId("event_step".to_string());
        let correlation_id = CorrelationId("corr-123".to_string());
        
        // Suspend for event
        let result = instance.suspend_for_event(step_id.clone(), correlation_id.clone());
        assert!(result.is_ok());
        
        // Check state changes
        assert_eq!(instance.status, FlowStatus::WaitingForEvent);
        assert_eq!(instance.correlation_data.get(&step_id).unwrap(), &correlation_id.0);
        
        // Check that events were recorded
        assert!(!instance.events.is_empty());
    }
    
    #[test]
    fn test_suspend_for_event_invalid_state() {
        // Create a waiting flow instance
        let mut instance = create_running_flow_instance();
        instance.status = FlowStatus::WaitingForTimer;
        
        // Attempt to suspend for event when already waiting
        let step_id = StepId("event_step".to_string());
        let correlation_id = CorrelationId("corr-123".to_string());
        let result = instance.suspend_for_event(step_id, correlation_id);
        
        // This should fail because the flow is not in Running state
        assert!(result.is_err());
        match result {
            Err(CoreError::FlowExecutionError(msg)) => {
                assert!(msg.contains("Cannot suspend flow in state"));
            }
            _ => panic!("Expected FlowExecutionError"),
        }
    }
    
    #[test]
    fn test_resume() {
        // Create a waiting flow instance
        let mut instance = create_running_flow_instance();
        instance.status = FlowStatus::WaitingForEvent;
        
        // Resume
        let result = instance.resume();
        assert!(result.is_ok());
        
        // Check state changes
        assert_eq!(instance.status, FlowStatus::Running);
        
        // Check that events were recorded
        assert!(!instance.events.is_empty());
    }
    
    #[test]
    fn test_resume_invalid_state() {
        // Create a running flow instance
        let mut instance = create_running_flow_instance();
        
        // Attempt to resume a flow that's already running
        let result = instance.resume();
        
        // This should fail because the flow is not in a waiting state
        assert!(result.is_err());
        match result {
            Err(CoreError::FlowExecutionError(msg)) => {
                assert!(msg.contains("Cannot resume flow in state"));
            }
            _ => panic!("Expected FlowExecutionError"),
        }
    }
    
    #[test]
    fn test_complete() {
        // Create a running flow instance
        let mut instance = create_running_flow_instance();
        
        // Complete the flow
        let result = instance.complete();
        assert!(result.is_ok());
        
        // Check state changes
        assert_eq!(instance.status, FlowStatus::Completed);
        
        // Check that events were recorded
        assert!(!instance.events.is_empty());
    }
    
    #[test]
    fn test_complete_invalid_state() {
        // Create a failed flow instance
        let mut instance = create_running_flow_instance();
        instance.status = FlowStatus::Failed;
        
        // Attempt to complete a flow that's already failed
        let result = instance.complete();
        
        // This should fail because the flow is in Failed state
        assert!(result.is_err());
        match result {
            Err(CoreError::FlowExecutionError(msg)) => {
                assert!(msg.contains("Cannot complete flow in state"));
            }
            _ => panic!("Expected FlowExecutionError"),
        }
    }
    
    #[test]
    fn test_fail() {
        // Create a running flow instance
        let mut instance = create_running_flow_instance();
        
        // Fail the flow
        let result = instance.fail("Test flow failure".to_string());
        assert!(result.is_ok());
        
        // Check state changes
        assert_eq!(instance.status, FlowStatus::Failed);
        assert_eq!(instance.error.as_ref().unwrap(), "Test flow failure");
        
        // Check that events were recorded
        assert!(!instance.events.is_empty());
    }
    
    #[test]
    fn test_fail_invalid_state() {
        // Create a completed flow instance
        let mut instance = create_running_flow_instance();
        instance.status = FlowStatus::Completed;
        
        // Attempt to fail a flow that's already completed
        let result = instance.fail("Test failure".to_string());
        
        // This should fail because the flow is in Completed state
        assert!(result.is_err());
        match result {
            Err(CoreError::FlowExecutionError(msg)) => {
                assert!(msg.contains("Cannot fail flow in state"));
            }
            _ => panic!("Expected FlowExecutionError"),
        }
    }
    
    #[test]
    fn test_all_steps_completed_when_empty() {
        // Create a flow instance
        let instance = create_running_flow_instance();
        
        // Test with empty step set
        let all_steps = HashSet::new();
        assert!(instance.all_steps_completed(&all_steps));
    }
    
    #[test]
    fn test_has_failed_steps() {
        // Create a flow instance
        let mut instance = create_running_flow_instance();
        
        // Initially no failed steps
        assert!(!instance.has_failed_steps());
        
        // Add a failed step
        instance.failed_steps.insert(StepId("failed_step".to_string()));
        assert!(instance.has_failed_steps());
    }

    #[test]
    fn test_get_step_output() {
        // Create a flow instance
        let flow_id = FlowId("output_test".to_string());
        let mut instance = FlowInstance::new(
            flow_id,
            DataPacket::new(json!({"input": "value"})),
        );
        
        // Set up some step outputs
        let step1_id = StepId("step1.output".to_string());
        let step1_data = DataPacket::new(json!({"result": 42}));
        instance.step_outputs.insert(step1_id, step1_data);
        
        let step2_id = StepId("step2.result".to_string());
        let step2_data = DataPacket::new(json!("Success"));
        instance.step_outputs.insert(step2_id, step2_data);
        
        // Test getting outputs
        let output1 = instance.get_step_output("step1", "output");
        assert!(output1.is_some());
        assert_eq!(output1.unwrap().as_value().get("result").unwrap(), &json!(42));
        
        let output2 = instance.get_step_output("step2", "result");
        assert!(output2.is_some());
        assert_eq!(output2.unwrap().as_value(), &json!("Success"));
        
        // Test getting non-existent output
        let missing = instance.get_step_output("step3", "missing");
        assert!(missing.is_none());
    }

    #[test]
    fn test_complete_step() {
        // Create a running flow instance
        let mut instance = create_running_flow_instance();
        let step_id = StepId("test_step".to_string());
        
        // Create outputs
        let mut outputs = HashMap::new();
        outputs.insert("result".to_string(), DataPacket::new(json!({"status": "complete"})));
        outputs.insert("code".to_string(), DataPacket::new(json!(200)));
        
        // Complete the step
        let result = instance.complete_step(step_id.clone(), outputs);
        assert!(result.is_ok());
        
        // Verify step is marked as completed
        assert!(instance.completed_steps.contains(&step_id));
        
        // Verify outputs are stored correctly
        let result_output = instance.get_step_output("test_step", "result");
        assert!(result_output.is_some());
        assert_eq!(result_output.unwrap().as_value().get("status").unwrap(), &json!("complete"));
        
        let code_output = instance.get_step_output("test_step", "code");
        assert!(code_output.is_some());
        assert_eq!(code_output.unwrap().as_value(), &json!(200));
        
        // Verify events were recorded
        assert!(!instance.events.is_empty());
    }

    #[test]
    fn test_complete_step_invalid_state() {
        // Create a completed flow instance
        let mut instance = create_running_flow_instance();
        instance.status = FlowStatus::Completed;
        
        let step_id = StepId("test_step".to_string());
        let outputs = HashMap::new();
        
        // Try to complete a step when flow is already completed
        let result = instance.complete_step(step_id, outputs);
        
        // This should fail
        assert!(result.is_err());
        match result {
            Err(CoreError::FlowExecutionError(msg)) => {
                assert!(msg.contains("Cannot complete step while flow is in state"));
            }
            _ => panic!("Expected FlowExecutionError"),
        }
    }

    #[test]
    fn test_start_invalid_state() {
        // Create a running flow instance (not initializing)
        let mut instance = create_running_flow_instance();
        
        // Try to start a flow that's already running
        let result = instance.start();
        
        // This should fail
        assert!(result.is_err());
        match result {
            Err(CoreError::FlowExecutionError(msg)) => {
                assert!(msg.contains("Cannot start flow in state"));
            }
            _ => panic!("Expected FlowExecutionError"),
        }
    }

    #[test]
    fn test_all_steps_completed_mixed() {
        // Create a flow instance with some completed steps
        let mut instance = create_running_flow_instance();
        
        // Add some completed steps
        instance.completed_steps.insert(StepId("step1".to_string()));
        instance.completed_steps.insert(StepId("step2".to_string()));
        
        // Create a set with all required steps
        let mut all_steps = HashSet::new();
        all_steps.insert(StepId("step1".to_string()));
        all_steps.insert(StepId("step2".to_string()));
        all_steps.insert(StepId("step3".to_string())); // Not completed
        
        // Not all steps are completed
        assert!(!instance.all_steps_completed(&all_steps));
        
        // Complete the last step
        instance.completed_steps.insert(StepId("step3".to_string()));
        
        // Now all steps should be completed
        assert!(instance.all_steps_completed(&all_steps));
    }

    #[test]
    fn test_is_step_completed() {
        // Create a flow instance
        let mut instance = create_running_flow_instance();
        
        // Define steps
        let completed_step = StepId("completed_step".to_string());
        let not_completed_step = StepId("not_completed_step".to_string());
        
        // Mark one step as completed
        instance.completed_steps.insert(completed_step.clone());
        
        // Test the is_step_completed method - should specifically cover lines 224-225
        assert!(instance.is_step_completed(&completed_step));
        assert!(!instance.is_step_completed(&not_completed_step));
        
        // Make sure checking a completed step is different from checking if a step is in failed_steps
        assert!(instance.completed_steps.contains(&completed_step));
        assert!(!instance.failed_steps.contains(&completed_step));
    }
    
    // Helper function to create a running flow instance
    fn create_running_flow_instance() -> FlowInstance {
        let mut instance = FlowInstance::new(
            FlowId("test_flow".to_string()),
            DataPacket::new(json!({"input": "test"})),
        );
        instance.status = FlowStatus::Running;
        instance.events.clear(); // Clear events
        instance
    }
}

use crate::domain::flow_instance::{FlowId, FlowInstanceId, StepId};
use chrono::{DateTime, Utc};
use std::fmt::Debug;

/// Domain event trait for all events in the system
pub trait DomainEvent: Debug + Send + Sync {
    /// Returns the type of the event as a string
    fn event_type(&self) -> &'static str;

    /// Returns the flow instance ID this event is associated with
    fn flow_instance_id(&self) -> &FlowInstanceId;

    /// Returns the timestamp when the event occurred
    fn timestamp(&self) -> DateTime<Utc>;
}

/// Event: Flow instance created
#[derive(Debug)]
pub struct FlowInstanceCreated {
    /// The unique identifier of the flow instance
    pub flow_instance_id: FlowInstanceId,

    /// The identifier of the flow definition
    pub flow_id: FlowId,

    /// The timestamp when the flow instance was created
    pub timestamp: DateTime<Utc>,
}

impl DomainEvent for FlowInstanceCreated {
    fn event_type(&self) -> &'static str {
        "flow_instance.created"
    }

    fn flow_instance_id(&self) -> &FlowInstanceId {
        &self.flow_instance_id
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

/// Event: Flow instance suspended for timer
#[derive(Debug)]
pub struct FlowInstanceSuspendedForTimer {
    /// The unique identifier of the flow instance
    pub flow_instance_id: FlowInstanceId,

    /// The identifier of the step that was suspended
    pub step_id: StepId,

    /// The identifier of the timer that was created
    pub timer_id: String,

    /// The timestamp when the flow instance was suspended
    pub timestamp: DateTime<Utc>,
}

impl DomainEvent for FlowInstanceSuspendedForTimer {
    fn event_type(&self) -> &'static str {
        "flow_instance.suspended_for_timer"
    }

    fn flow_instance_id(&self) -> &FlowInstanceId {
        &self.flow_instance_id
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

/// Event: Flow instance suspended for external event
#[derive(Debug)]
pub struct FlowInstanceSuspendedForEvent {
    /// The unique identifier of the flow instance
    pub flow_instance_id: FlowInstanceId,

    /// The identifier of the step that was suspended
    pub step_id: StepId,

    /// The correlation identifier used to match incoming events
    pub correlation_id: String,

    /// The timestamp when the flow instance was suspended
    pub timestamp: DateTime<Utc>,
}

impl DomainEvent for FlowInstanceSuspendedForEvent {
    fn event_type(&self) -> &'static str {
        "flow_instance.suspended_for_event"
    }

    fn flow_instance_id(&self) -> &FlowInstanceId {
        &self.flow_instance_id
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

/// Event: Flow instance resumed
#[derive(Debug)]
pub struct FlowInstanceResumed {
    /// The unique identifier of the flow instance
    pub flow_instance_id: FlowInstanceId,
    /// The timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
}

impl DomainEvent for FlowInstanceResumed {
    fn event_type(&self) -> &'static str {
        "flow_instance.resumed"
    }

    fn flow_instance_id(&self) -> &FlowInstanceId {
        &self.flow_instance_id
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

/// Event: Step scheduled for execution
#[derive(Debug)]
pub struct StepScheduled {
    /// The unique identifier of the flow instance
    pub flow_instance_id: FlowInstanceId,
    /// The identifier of the step that was scheduled
    pub step_id: StepId,
    /// The timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
}

impl DomainEvent for StepScheduled {
    fn event_type(&self) -> &'static str {
        "step.scheduled"
    }

    fn flow_instance_id(&self) -> &FlowInstanceId {
        &self.flow_instance_id
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

/// Event: Step started execution
#[derive(Debug)]
pub struct StepStarted {
    /// The unique identifier of the flow instance
    pub flow_instance_id: FlowInstanceId,
    /// The identifier of the step that started
    pub step_id: StepId,
    /// The timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
}

impl DomainEvent for StepStarted {
    fn event_type(&self) -> &'static str {
        "step.started"
    }

    fn flow_instance_id(&self) -> &FlowInstanceId {
        &self.flow_instance_id
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

/// Event: Step completed
#[derive(Debug)]
pub struct StepCompleted {
    /// The unique identifier of the flow instance
    pub flow_instance_id: FlowInstanceId,
    /// The identifier of the step that completed
    pub step_id: StepId,
    /// The timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
}

impl DomainEvent for StepCompleted {
    fn event_type(&self) -> &'static str {
        "step.completed"
    }

    fn flow_instance_id(&self) -> &FlowInstanceId {
        &self.flow_instance_id
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

/// Event: Step failed
#[derive(Debug)]
pub struct StepFailed {
    /// The unique identifier of the flow instance
    pub flow_instance_id: FlowInstanceId,
    /// The identifier of the step that failed
    pub step_id: StepId,
    /// The error message
    pub error: String,
    /// The timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
}

impl DomainEvent for StepFailed {
    fn event_type(&self) -> &'static str {
        "step.failed"
    }

    fn flow_instance_id(&self) -> &FlowInstanceId {
        &self.flow_instance_id
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

/// Event: Flow instance completed
#[derive(Debug)]
pub struct FlowInstanceCompleted {
    /// The unique identifier of the flow instance
    pub flow_instance_id: FlowInstanceId,
    /// The timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
}

impl DomainEvent for FlowInstanceCompleted {
    fn event_type(&self) -> &'static str {
        "flow_instance.completed"
    }

    fn flow_instance_id(&self) -> &FlowInstanceId {
        &self.flow_instance_id
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

/// Event: Flow instance failed
#[derive(Debug)]
pub struct FlowInstanceFailed {
    /// The unique identifier of the flow instance
    pub flow_instance_id: FlowInstanceId,
    /// The error message
    pub error: String,
    /// The timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
}

impl DomainEvent for FlowInstanceFailed {
    fn event_type(&self) -> &'static str {
        "flow_instance.failed"
    }

    fn flow_instance_id(&self) -> &FlowInstanceId {
        &self.flow_instance_id
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    // Helper to create a flow instance ID for testing
    fn create_test_flow_instance_id() -> FlowInstanceId {
        FlowInstanceId(Uuid::new_v4().to_string())
    }

    // Helper to create a flow ID for testing
    fn create_test_flow_id() -> FlowId {
        FlowId(Uuid::new_v4().to_string())
    }

    // Helper to create a step ID for testing
    fn create_test_step_id() -> StepId {
        StepId(Uuid::new_v4().to_string())
    }

    #[test]
    fn test_flow_instance_created_event() {
        let flow_instance_id = create_test_flow_instance_id();
        let flow_id = create_test_flow_id();
        let timestamp = Utc::now();

        let event = FlowInstanceCreated {
            flow_instance_id: flow_instance_id.clone(),
            flow_id: flow_id.clone(),
            timestamp,
        };

        assert_eq!(event.event_type(), "flow_instance.created");
        assert_eq!(event.flow_instance_id(), &flow_instance_id);
        assert_eq!(event.timestamp(), timestamp);
    }

    #[test]
    fn test_flow_instance_suspended_for_timer_event() {
        let flow_instance_id = create_test_flow_instance_id();
        let step_id = create_test_step_id();
        let timer_id = "test-timer-1".to_string();
        let timestamp = Utc::now();

        let event = FlowInstanceSuspendedForTimer {
            flow_instance_id: flow_instance_id.clone(),
            step_id,
            timer_id,
            timestamp,
        };

        assert_eq!(event.event_type(), "flow_instance.suspended_for_timer");
        assert_eq!(event.flow_instance_id(), &flow_instance_id);
        assert_eq!(event.timestamp(), timestamp);
    }

    #[test]
    fn test_flow_instance_suspended_for_event_event() {
        let flow_instance_id = create_test_flow_instance_id();
        let step_id = create_test_step_id();
        let correlation_id = "test-correlation-1".to_string();
        let timestamp = Utc::now();

        let event = FlowInstanceSuspendedForEvent {
            flow_instance_id: flow_instance_id.clone(),
            step_id,
            correlation_id,
            timestamp,
        };

        assert_eq!(event.event_type(), "flow_instance.suspended_for_event");
        assert_eq!(event.flow_instance_id(), &flow_instance_id);
        assert_eq!(event.timestamp(), timestamp);
    }

    #[test]
    fn test_flow_instance_resumed_event() {
        let flow_instance_id = create_test_flow_instance_id();
        let timestamp = Utc::now();

        let event = FlowInstanceResumed {
            flow_instance_id: flow_instance_id.clone(),
            timestamp,
        };

        assert_eq!(event.event_type(), "flow_instance.resumed");
        assert_eq!(event.flow_instance_id(), &flow_instance_id);
        assert_eq!(event.timestamp(), timestamp);
    }

    #[test]
    fn test_step_scheduled_event() {
        let flow_instance_id = create_test_flow_instance_id();
        let step_id = create_test_step_id();
        let timestamp = Utc::now();

        let event = StepScheduled {
            flow_instance_id: flow_instance_id.clone(),
            step_id,
            timestamp,
        };

        assert_eq!(event.event_type(), "step.scheduled");
        assert_eq!(event.flow_instance_id(), &flow_instance_id);
        assert_eq!(event.timestamp(), timestamp);
    }

    #[test]
    fn test_step_started_event() {
        let flow_instance_id = create_test_flow_instance_id();
        let step_id = create_test_step_id();
        let timestamp = Utc::now();

        let event = StepStarted {
            flow_instance_id: flow_instance_id.clone(),
            step_id,
            timestamp,
        };

        assert_eq!(event.event_type(), "step.started");
        assert_eq!(event.flow_instance_id(), &flow_instance_id);
        assert_eq!(event.timestamp(), timestamp);
    }

    #[test]
    fn test_step_completed_event() {
        let flow_instance_id = create_test_flow_instance_id();
        let step_id = create_test_step_id();
        let timestamp = Utc::now();

        let event = StepCompleted {
            flow_instance_id: flow_instance_id.clone(),
            step_id,
            timestamp,
        };

        assert_eq!(event.event_type(), "step.completed");
        assert_eq!(event.flow_instance_id(), &flow_instance_id);
        assert_eq!(event.timestamp(), timestamp);
    }

    #[test]
    fn test_step_failed_event() {
        let flow_instance_id = create_test_flow_instance_id();
        let step_id = create_test_step_id();
        let error = "Test error message".to_string();
        let timestamp = Utc::now();

        let event = StepFailed {
            flow_instance_id: flow_instance_id.clone(),
            step_id,
            error,
            timestamp,
        };

        assert_eq!(event.event_type(), "step.failed");
        assert_eq!(event.flow_instance_id(), &flow_instance_id);
        assert_eq!(event.timestamp(), timestamp);
    }

    #[test]
    fn test_flow_instance_completed_event() {
        let flow_instance_id = create_test_flow_instance_id();
        let timestamp = Utc::now();

        let event = FlowInstanceCompleted {
            flow_instance_id: flow_instance_id.clone(),
            timestamp,
        };

        assert_eq!(event.event_type(), "flow_instance.completed");
        assert_eq!(event.flow_instance_id(), &flow_instance_id);
        assert_eq!(event.timestamp(), timestamp);
    }

    #[test]
    fn test_flow_instance_failed_event() {
        let flow_instance_id = create_test_flow_instance_id();
        let error = "Flow failed with test error".to_string();
        let timestamp = Utc::now();

        let event = FlowInstanceFailed {
            flow_instance_id: flow_instance_id.clone(),
            error,
            timestamp,
        };

        assert_eq!(event.event_type(), "flow_instance.failed");
        assert_eq!(event.flow_instance_id(), &flow_instance_id);
        assert_eq!(event.timestamp(), timestamp);
    }
}

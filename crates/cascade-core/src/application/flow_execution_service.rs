use crate::{
    domain::events::{DomainEvent, FlowInstanceResumed},
    domain::flow_definition::FlowDefinition,
    domain::flow_instance::{
        CorrelationId, FlowId, FlowInstance, FlowInstanceId, FlowStatus, StepId,
    },
    domain::repository::{
        ComponentStateRepository, FlowDefinitionRepository, FlowInstanceRepository, TimerRepository,
    },
    domain::step::ConditionEvaluator,
    ComponentExecutor, ComponentRuntimeApi, ComponentRuntimeApiBase, CoreError, DataPacket,
    ExecutionResult, LogLevel,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, error, info, trace, warn};

/// Factory function to create component executors
pub type ComponentFactory =
    Arc<dyn Fn(&str) -> Result<Arc<dyn ComponentExecutor>, CoreError> + Send + Sync>;

/// Service for executing flows
pub struct FlowExecutionService {
    /// Repository for flow instances
    flow_instance_repo: Arc<dyn FlowInstanceRepository>,

    /// Repository for flow definitions
    flow_definition_repo: Arc<dyn FlowDefinitionRepository>,

    /// Repository for component state
    component_state_repo: Arc<dyn ComponentStateRepository>,

    /// Repository for timers
    timer_repo: Arc<dyn TimerRepository>,

    /// Factory for component executors
    component_factory: ComponentFactory,

    /// Condition evaluator
    condition_evaluator: Arc<dyn ConditionEvaluator>,

    /// Event handler
    event_handler: Arc<dyn DomainEventHandler>,
}

impl FlowExecutionService {
    /// Create a new flow execution service
    pub fn new(
        flow_instance_repo: Arc<dyn FlowInstanceRepository>,
        flow_definition_repo: Arc<dyn FlowDefinitionRepository>,
        component_state_repo: Arc<dyn ComponentStateRepository>,
        timer_repo: Arc<dyn TimerRepository>,
        component_factory: ComponentFactory,
        condition_evaluator: Arc<dyn ConditionEvaluator>,
        event_handler: Arc<dyn DomainEventHandler>,
    ) -> Self {
        Self {
            flow_instance_repo,
            flow_definition_repo,
            component_state_repo,
            timer_repo,
            component_factory,
            condition_evaluator,
            event_handler,
        }
    }

    /// Start a new flow instance
    pub async fn start_flow(
        &self,
        flow_id: FlowId,
        trigger_data: DataPacket,
    ) -> Result<FlowInstanceId, CoreError> {
        // Find flow definition
        let flow_definition = self
            .flow_definition_repo
            .find_by_id(&flow_id)
            .await?
            .ok_or_else(|| {
                CoreError::FlowExecutionError(format!("Flow not found: {}", flow_id.0))
            })?;

        // Create a new flow instance
        let mut flow_instance = FlowInstance::new(flow_id.clone(), trigger_data);

        // Start the flow
        flow_instance.start()?;

        // Save initial state
        self.flow_instance_repo.save(&flow_instance).await?;

        // Handle initial events
        self.handle_events(&mut flow_instance).await?;

        // Start execution asynchronously
        let service = self.clone();
        let instance_id = flow_instance.id.clone();

        tokio::spawn(async move {
            if let Err(e) = service.execute_flow(flow_definition, flow_instance).await {
                eprintln!("Flow execution error: {}", e);
            }
        });

        Ok(instance_id)
    }

    /// Resume a waiting flow instance
    pub async fn resume_flow_from_timer(
        &self,
        flow_instance_id: &FlowInstanceId,
        step_id: &StepId,
    ) -> Result<(), CoreError> {
        // Load flow instance
        let mut flow_instance = self
            .flow_instance_repo
            .find_by_id(flow_instance_id)
            .await?
            .ok_or_else(|| {
                CoreError::FlowExecutionError(format!(
                    "Flow instance not found: {}",
                    flow_instance_id.0
                ))
            })?;

        // Check if flow is waiting for timer
        if flow_instance.status != FlowStatus::WaitingForTimer {
            return Err(CoreError::FlowExecutionError(format!(
                "Flow instance is not waiting for timer: {}",
                flow_instance_id.0
            )));
        }

        // Remove timer reference
        flow_instance.pending_timers.remove(step_id);

        // Resume flow
        flow_instance.resume()?;

        // Add resumed event
        flow_instance.record_event(Box::new(FlowInstanceResumed {
            flow_instance_id: flow_instance.id.clone(),
            timestamp: chrono::Utc::now(),
        }));

        // Save state
        self.flow_instance_repo.save(&flow_instance).await?;

        // Handle events
        self.handle_events(&mut flow_instance).await?;

        // Find flow definition
        let flow_definition = self
            .flow_definition_repo
            .find_by_id(&flow_instance.flow_id)
            .await?
            .ok_or_else(|| {
                CoreError::FlowExecutionError(format!(
                    "Flow not found: {}",
                    flow_instance.flow_id.0
                ))
            })?;

        // Continue execution asynchronously
        let service = self.clone();

        tokio::spawn(async move {
            if let Err(e) = service.execute_flow(flow_definition, flow_instance).await {
                eprintln!("Flow resume error: {}", e);
            }
        });

        Ok(())
    }

    /// Resume a flow waiting for external event
    pub async fn resume_flow_with_event(
        &self,
        correlation_id: &CorrelationId,
        event_data: DataPacket,
    ) -> Result<(), CoreError> {
        // Find flow instances waiting for this correlation ID
        let waiting_instances = self
            .flow_instance_repo
            .find_by_correlation(correlation_id)
            .await?;

        if waiting_instances.is_empty() {
            return Err(CoreError::FlowExecutionError(format!(
                "No flow instances waiting for correlation ID: {}",
                correlation_id.0
            )));
        }

        // Process each waiting instance
        for mut flow_instance in waiting_instances {
            // Find which step is waiting for this correlation ID
            let waiting_step = flow_instance
                .correlation_data
                .iter()
                .find_map(|(step_id, corr)| {
                    if corr == &correlation_id.0 {
                        Some(step_id.clone())
                    } else {
                        None
                    }
                });

            if let Some(step_id) = waiting_step {
                // Remove correlation ID reference
                flow_instance.correlation_data.remove(&step_id);

                // Store event data as output for the waiting step
                let output_id = StepId(format!("{}.result", step_id.0));
                flow_instance
                    .step_outputs
                    .insert(output_id, event_data.clone());
                flow_instance.completed_steps.insert(step_id);

                // Resume flow
                flow_instance.resume()?;

                // Add resumed event
                flow_instance.record_event(Box::new(FlowInstanceResumed {
                    flow_instance_id: flow_instance.id.clone(),
                    timestamp: chrono::Utc::now(),
                }));

                // Save state
                self.flow_instance_repo.save(&flow_instance).await?;

                // Handle events
                self.handle_events(&mut flow_instance).await?;

                // Find flow definition
                let flow_definition = self
                    .flow_definition_repo
                    .find_by_id(&flow_instance.flow_id)
                    .await?
                    .ok_or_else(|| {
                        CoreError::FlowExecutionError(format!(
                            "Flow not found: {}",
                            flow_instance.flow_id.0
                        ))
                    })?;

                // Continue execution asynchronously
                let service = self.clone();
                let instance = flow_instance.clone();

                tokio::spawn(async move {
                    if let Err(e) = service.execute_flow(flow_definition, instance).await {
                        eprintln!("Flow resume error: {}", e);
                    }
                });
            }
        }

        Ok(())
    }

    /// Execute a flow instance
    async fn execute_flow(
        &self,
        flow_definition: FlowDefinition,
        mut flow_instance: FlowInstance,
    ) -> Result<(), CoreError> {
        // Resolve steps from definition - this is a costly operation, do it once
        let steps = self.resolve_steps(&flow_definition).await?;

        // Create a cache for step dependencies to avoid repeated lookups
        let step_ids: HashSet<StepId> = steps.iter().map(|s| s.id.clone()).collect();

        // Continue execution until flow completes or is suspended
        let mut progress = true;

        while progress && flow_instance.status == FlowStatus::Running {
            progress = false;

            // Find ready steps - this is performance-critical
            let ready_steps = self.determine_ready_steps(&steps, &flow_instance);

            // Execute ready steps in parallel if there are multiple
            if ready_steps.len() > 1 {
                // Create future for each step
                let mut futures = Vec::with_capacity(ready_steps.len());

                for step_ref in &ready_steps {
                    let step_clone = (**step_ref).clone();
                    let service = self.clone();
                    let flow_instance_ref = flow_instance.clone();

                    futures.push(async move {
                        let step_id = step_clone.id.clone();
                        match service.execute_step(&step_clone, &flow_instance_ref).await {
                            Ok(outputs) => (step_id, Ok(outputs)),
                            Err(e) => (step_id, Err(e)),
                        }
                    });
                }

                // Execute all steps in parallel
                let results = futures::future::join_all(futures).await;

                // Process results
                for (step_id, result) in results {
                    match result {
                        Ok(outputs) => {
                            // Update flow instance with step outputs
                            for (output_name, output_value) in outputs {
                                let output_id = StepId(format!("{}.{}", step_id.0, output_name));
                                flow_instance.step_outputs.insert(output_id, output_value);
                            }
                            flow_instance.completed_steps.insert(step_id);
                            progress = true;
                        }
                        Err(error) => {
                            // Mark step as failed
                            flow_instance.failed_steps.insert(step_id.clone());
                            flow_instance
                                .failed_step_errors
                                .insert(step_id, error.to_string());

                            // For now, any step failure is considered fatal
                            flow_instance.fail(error.to_string())?;
                            progress = false;
                            break;
                        }
                    }
                }
            } else {
                // Single step execution path (more common)
                for step in &ready_steps {
                    // Execute the step
                    let result = self.execute_step(step, &flow_instance).await;

                    match result {
                        Ok(outputs) => {
                            // Update flow instance with step outputs
                            for (output_name, output_value) in outputs {
                                let output_id = StepId(format!("{}.{}", step.id.0, output_name));
                                flow_instance.step_outputs.insert(output_id, output_value);
                            }
                            flow_instance.completed_steps.insert(step.id.clone());
                            progress = true;
                        }
                        Err(error) => {
                            // Mark step as failed
                            flow_instance.failed_steps.insert(step.id.clone());
                            flow_instance
                                .failed_step_errors
                                .insert(step.id.clone(), error.to_string());

                            // Check if this is a fatal error
                            // For now, any step failure is considered fatal
                            flow_instance.fail(error.to_string())?;
                            progress = false;
                            break;
                        }
                    }
                }
            }

            if progress {
                // Save updated flow state
                self.flow_instance_repo.save(&flow_instance).await?;

                // Handle events
                self.handle_events(&mut flow_instance).await?;
            }

            // If no progress and still running, check if flow should be completed
            if !progress && flow_instance.status == FlowStatus::Running {
                // Check if all steps are completed
                if flow_instance.all_steps_completed(&step_ids) {
                    flow_instance.complete()?;
                    self.flow_instance_repo.save(&flow_instance).await?;
                    self.handle_events(&mut flow_instance).await?;
                } else if flow_instance.has_failed_steps() {
                    // If any steps have failed and we can't make progress, fail the flow
                    flow_instance.fail("Flow execution blocked due to failed steps".to_string())?;
                    self.flow_instance_repo.save(&flow_instance).await?;
                    self.handle_events(&mut flow_instance).await?;
                }
            }
        }

        Ok(())
    }

    /// Resolve steps from flow definition
    async fn resolve_steps(
        &self,
        flow_definition: &FlowDefinition,
    ) -> Result<Vec<ResolvedStep>, CoreError> {
        let mut resolved_steps = Vec::new();

        for step_def in &flow_definition.steps {
            // Map dependencies
            let dependencies = step_def
                .run_after
                .iter()
                .map(|id| StepId(id.clone()))
                .collect();

            // Map input mappings
            let input_mappings = step_def
                .input_mapping
                .iter()
                .map(|(k, v)| (k.clone(), v.expression.clone()))
                .collect();

            // Convert config to HashMap
            let config = match &step_def.config {
                Value::Object(obj) => {
                    let mut map = HashMap::new();
                    for (k, v) in obj {
                        map.insert(k.clone(), v.clone());
                    }
                    map
                }
                _ => HashMap::new(),
            };

            // Resolve condition
            let condition = step_def
                .condition
                .as_ref()
                .map(|c| (c.expression.clone(), c.language.clone()));

            // Create resolved step
            let resolved_step = ResolvedStep {
                id: StepId(step_def.id.clone()),
                component_type: step_def.component_type.clone(),
                dependencies,
                config,
                input_mappings,
                condition,
            };

            resolved_steps.push(resolved_step);
        }

        Ok(resolved_steps)
    }

    /// Determine which steps are ready to execute
    fn determine_ready_steps<'a>(
        &self,
        steps: &'a [ResolvedStep],
        flow_instance: &FlowInstance,
    ) -> Vec<&'a ResolvedStep> {
        let mut ready_steps = Vec::new();

        for step in steps {
            // Skip already completed steps
            if flow_instance.completed_steps.contains(&step.id) {
                continue;
            }

            // Skip already failed steps
            if flow_instance.failed_steps.contains(&step.id) {
                continue;
            }

            // Check if all dependencies are met
            let dependencies_met = step
                .dependencies
                .iter()
                .all(|dep| flow_instance.completed_steps.contains(dep));

            if dependencies_met {
                ready_steps.push(step);
            }
        }

        ready_steps
    }

    /// Execute a single step
    async fn execute_step(
        &self,
        step: &ResolvedStep,
        flow_instance: &FlowInstance,
    ) -> Result<HashMap<String, DataPacket>, CoreError> {
        tracing::debug!(
            flow_id = %flow_instance.flow_id.0,
            step_id = ?step.id,
            component_type = %step.component_type,
            "Executing step"
        );

        // Check condition if present
        if let Some((condition_expr, condition_lang)) = &step.condition {
            // Build evaluation context only when needed
            let context = self.build_evaluation_context(flow_instance);

            // Evaluate condition
            let result =
                self.condition_evaluator
                    .evaluate(condition_expr, condition_lang, &context)?;

            if !result {
                // Condition not met, skip step
                tracing::debug!(
                    flow_id = %flow_instance.flow_id.0,
                    step_id = ?step.id,
                    "Skipping step due to condition evaluation"
                );
                return Ok(HashMap::new());
            }
        }

        // Resolve input values based on mappings
        let mut inputs = HashMap::with_capacity(step.input_mappings.len());

        for (input_name, expression) in &step.input_mappings {
            match self.resolve_data_reference(expression, flow_instance) {
                Ok(value) => {
                    inputs.insert(input_name.clone(), value);
                }
                Err(err) => {
                    // Missing or invalid input reference
                    tracing::warn!(
                        flow_id = %flow_instance.flow_id.0,
                        step_id = ?step.id,
                        input = %input_name,
                        expression = %expression,
                        error = %err,
                        "Failed to resolve input reference"
                    );
                    return Err(err);
                }
            }
        }

        // Create step context
        let context = StepExecutionContext::new(
            flow_instance.id.clone(),
            step.id.clone(),
            inputs,
            step.config.clone(),
        );

        // Create step runtime API
        let api = Arc::new(StepRuntimeApi {
            context: Arc::new(Mutex::new(context)),
            component_state_repo: self.component_state_repo.clone(),
            timer_repo: self.timer_repo.clone(),
        });

        // Create component executor - this may be expensive so skip if already done
        let component = (self.component_factory)(&step.component_type)?;

        // Execute component
        tracing::debug!(
            flow_id = %flow_instance.flow_id.0,
            step_id = ?step.id,
            "Invoking component execution"
        );

        let result = component.execute(api.clone()).await;

        match result {
            ExecutionResult::Success => {
                // Get outputs from API
                let context = api.context.lock().await;
                let outputs = context.get_outputs();

                tracing::debug!(
                    flow_id = %flow_instance.flow_id.0,
                    step_id = ?step.id,
                    outputs_count = outputs.len(),
                    "Step executed successfully"
                );

                Ok(outputs)
            }
            ExecutionResult::Failure(error) => {
                tracing::warn!(
                    flow_id = %flow_instance.flow_id.0,
                    step_id = ?step.id,
                    error = %error,
                    "Step execution failed"
                );

                Err(error)
            }
            ExecutionResult::Pending(reason) => {
                tracing::debug!(
                    flow_id = %flow_instance.flow_id.0,
                    step_id = ?step.id,
                    reason = %reason,
                    "Step execution pending"
                );

                // Return empty outputs for now
                Ok(HashMap::new())
            }
        }
    }

    /// Resolve data reference from a path expression
    fn resolve_data_reference(
        &self,
        expression: &str,
        flow_instance: &FlowInstance,
    ) -> Result<DataPacket, CoreError> {
        // Special case for trigger data (most common) to avoid complex parsing
        if expression == "$flow.trigger_data" {
            return Ok(flow_instance.trigger_data.clone());
        }

        // Special cases for other common patterns
        if expression.starts_with("$steps.") && expression.contains(".outputs.") {
            // Parse $steps.{step_id}.outputs.{output_name}
            let parts: Vec<&str> = expression.split('.').collect();
            if parts.len() == 4 && parts[0] == "$steps" && parts[2] == "outputs" {
                let step_id = parts[1];
                let output_name = parts[3];

                // Direct lookup which is more efficient
                if let Some(output) = flow_instance.get_step_output(step_id, output_name) {
                    return Ok(output.clone());
                }

                return Err(CoreError::ReferenceError(format!(
                    "Output not found: {}.{}",
                    step_id, output_name
                )));
            }
        }

        // For more complex expressions, fall back to the full evaluation context
        let context = self.build_evaluation_context(flow_instance);

        // Try JSON path evaluation
        if let Some(path) = expression.strip_prefix("$") {
            match jmespath::compile(path) {
                Ok(compiled) => match compiled.search(&context) {
                    Ok(value) => {
                        // Convert jmespath result to serde_json::Value
                        let json_value =
                            serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
                        Ok(DataPacket::new(json_value))
                    }
                    Err(e) => Err(CoreError::ExpressionError(format!(
                        "Failed to evaluate JMESPath expression: {}: {}",
                        path, e
                    ))),
                },
                Err(e) => Err(CoreError::ExpressionError(format!(
                    "Failed to compile JMESPath expression: {}: {}",
                    path, e
                ))),
            }
        } else {
            // Literal value
            match serde_json::from_str::<serde_json::Value>(expression) {
                Ok(value) => Ok(DataPacket::new(value)),
                Err(_) => Err(CoreError::ExpressionError(format!(
                    "Invalid expression: {}",
                    expression
                ))),
            }
        }
    }

    /// Build context for condition evaluation
    fn build_evaluation_context(&self, flow_instance: &FlowInstance) -> Value {
        let mut context = json!({
            "flow": {
                "id": flow_instance.flow_id.0,
                "instance_id": flow_instance.id.0,
                "trigger_data": flow_instance.trigger_data.as_value(),
            },
            "steps": {}
        });

        // Add step outputs to context
        let steps_obj = context["steps"].as_object_mut().unwrap();

        for (output_id, output) in &flow_instance.step_outputs {
            let parts: Vec<&str> = output_id.0.splitn(2, '.').collect();
            if parts.len() == 2 {
                let step_id = parts[0];
                let output_name = parts[1];

                if !steps_obj.contains_key(step_id) {
                    steps_obj.insert(
                        step_id.to_string(),
                        json!({
                            "outputs": {}
                        }),
                    );
                }

                let step_outputs = steps_obj[step_id]["outputs"].as_object_mut().unwrap();
                step_outputs.insert(output_name.to_string(), output.as_value().clone());
            }
        }

        context
    }

    /// Handle domain events
    async fn handle_events(&self, flow_instance: &mut FlowInstance) -> Result<(), CoreError> {
        let events = flow_instance.take_events();

        for event in events {
            self.event_handler.handle_event(event).await?;
        }

        Ok(())
    }
}

/// Clone implementation for FlowExecutionService
impl Clone for FlowExecutionService {
    fn clone(&self) -> Self {
        Self {
            flow_instance_repo: self.flow_instance_repo.clone(),
            flow_definition_repo: self.flow_definition_repo.clone(),
            component_state_repo: self.component_state_repo.clone(),
            timer_repo: self.timer_repo.clone(),
            component_factory: self.component_factory.clone(),
            condition_evaluator: self.condition_evaluator.clone(),
            event_handler: self.event_handler.clone(),
        }
    }
}

/// Handler for domain events
#[async_trait]
pub trait DomainEventHandler: Send + Sync {
    /// Handle a domain event
    async fn handle_event(&self, event: Box<dyn DomainEvent>) -> Result<(), CoreError>;
}

/// Runtime API implementation for step execution
struct StepRuntimeApi {
    context: Arc<Mutex<StepExecutionContext>>,
    component_state_repo: Arc<dyn ComponentStateRepository>,
    timer_repo: Arc<dyn TimerRepository>,
}

impl ComponentRuntimeApiBase for StepRuntimeApi {
    fn log(&self, level: tracing::Level, message: &str) {
        let context = self.context.try_lock();
        if let Ok(ctx) = context {
            let step_id = &ctx.step_id;
            let flow_id = &ctx.flow_instance_id;

            match level {
                tracing::Level::ERROR => {
                    tracing::error!(flow_id = %flow_id.0, step_id = %step_id.0, "{}", message);
                }
                tracing::Level::WARN => {
                    tracing::warn!(flow_id = %flow_id.0, step_id = %step_id.0, "{}", message);
                }
                tracing::Level::INFO => {
                    tracing::info!(flow_id = %flow_id.0, step_id = %step_id.0, "{}", message);
                }
                tracing::Level::DEBUG => {
                    tracing::debug!(flow_id = %flow_id.0, step_id = %step_id.0, "{}", message);
                }
                _ => {
                    tracing::trace!(flow_id = %flow_id.0, step_id = %step_id.0, "{}", message);
                }
            }
        } else {
            // If we can't get the context, log without context info
            match level {
                tracing::Level::ERROR => {
                    tracing::error!("{}", message);
                }
                tracing::Level::WARN => {
                    tracing::warn!("{}", message);
                }
                tracing::Level::INFO => {
                    tracing::info!("{}", message);
                }
                tracing::Level::DEBUG => {
                    tracing::debug!("{}", message);
                }
                _ => {
                    tracing::trace!("{}", message);
                }
            }
        }
    }
}

#[async_trait]
impl ComponentRuntimeApi for StepRuntimeApi {
    async fn get_input(&self, name: &str) -> Result<DataPacket, CoreError> {
        let context = self.context.lock().await;
        context
            .get_input(name)
            .ok_or_else(|| CoreError::ReferenceError(format!("Input not found: {}", name)))
    }

    async fn get_config(&self, name: &str) -> Result<Value, CoreError> {
        let context = self.context.lock().await;
        context
            .get_config(name)
            .ok_or_else(|| CoreError::ReferenceError(format!("Config not found: {}", name)))
    }

    async fn set_output(&self, name: &str, data: DataPacket) -> Result<(), CoreError> {
        let mut context = self.context.lock().await;
        context.set_output(name, data);
        Ok(())
    }

    async fn get_state(&self) -> Result<Option<Value>, CoreError> {
        let context = self.context.lock().await;
        let flow_instance_id = &context.flow_instance_id;
        let step_id = &context.step_id;

        self.component_state_repo
            .get_state(flow_instance_id, step_id)
            .await
    }

    async fn save_state(&self, state: Value) -> Result<(), CoreError> {
        let context = self.context.lock().await;
        let flow_instance_id = &context.flow_instance_id;
        let step_id = &context.step_id;

        self.component_state_repo
            .save_state(flow_instance_id, step_id, state)
            .await
    }

    async fn schedule_timer(&self, duration: Duration) -> Result<(), CoreError> {
        let context = self.context.lock().await;
        let flow_instance_id = &context.flow_instance_id;
        let step_id = &context.step_id;

        self.timer_repo
            .schedule(flow_instance_id, step_id, duration)
            .await?;

        // Set a flag in the context to indicate that execution is waiting for a timer
        context.set_pending_timer(step_id.clone());

        Ok(())
    }

    async fn correlation_id(&self) -> Result<CorrelationId, CoreError> {
        let context = self.context.lock().await;
        Ok(CorrelationId(format!(
            "flow-{}-step-{}",
            context.flow_instance_id.0, context.step_id.0
        )))
    }

    async fn log(&self, level: LogLevel, message: &str) -> Result<(), CoreError> {
        let tracing_level: tracing::Level = level.into();
        match tracing_level {
            tracing::Level::ERROR => error!("{}", message),
            tracing::Level::WARN => warn!("{}", message),
            tracing::Level::INFO => info!("{}", message),
            tracing::Level::DEBUG => debug!("{}", message),
            tracing::Level::TRACE => trace!("{}", message),
        }
        Ok(())
    }

    async fn emit_metric(
        &self,
        name: &str,
        value: f64,
        _labels: HashMap<String, String>,
    ) -> Result<(), CoreError> {
        // Convert to tracing event for now
        let context = self.context.lock().await;
        tracing::debug!(
            flow_id = %context.flow_instance_id.0,
            instance_id = %context.flow_instance_id.0,
            component = %context.step_id.0,
            metric_name = %name,
            metric_value = %value,
            "Component emitted metric"
        );
        Ok(())
    }
}

/// A step with dependencies resolved
#[derive(Debug, Clone)]
struct ResolvedStep {
    id: StepId,
    component_type: String,
    dependencies: HashSet<StepId>,
    config: HashMap<String, Value>,
    input_mappings: HashMap<String, String>,
    condition: Option<(String, String)>,
}

impl ResolvedStep {
    /// Resolve input mappings for this step
    #[allow(dead_code)]
    fn resolve_inputs(&self, flow_instance: &FlowInstance) -> HashMap<String, DataPacket> {
        let mut inputs = HashMap::new();

        for (input_name, mapping) in &self.input_mappings {
            // Parse the reference expression
            if mapping.starts_with("$flow.trigger_data") {
                // Return trigger data
                inputs.insert(input_name.clone(), flow_instance.trigger_data.clone());
            } else if mapping.starts_with("$steps.") {
                // Parse step and output names
                let parts: Vec<&str> = mapping.trim_start_matches("$steps.").split('.').collect();
                if parts.len() >= 2 {
                    let step_id = parts[0];
                    let output_part = &parts[1..].join(".");

                    // Look up in step outputs
                    let output_id = StepId(format!("{}.{}", step_id, output_part));
                    if let Some(output) = flow_instance.step_outputs.get(&output_id) {
                        inputs.insert(input_name.clone(), output.clone());
                    }
                }
            }
        }

        inputs
    }
}

/// Context for step execution
struct StepExecutionContext {
    flow_instance_id: FlowInstanceId,
    step_id: StepId,
    inputs: HashMap<String, DataPacket>,
    outputs: HashMap<String, DataPacket>,
    config: HashMap<String, Value>,
    pending_timer: std::sync::Mutex<Option<StepId>>,
}

impl StepExecutionContext {
    /// Create a new step execution context
    fn new(
        flow_instance_id: FlowInstanceId,
        step_id: StepId,
        inputs: HashMap<String, DataPacket>,
        config: HashMap<String, Value>,
    ) -> Self {
        Self {
            flow_instance_id,
            step_id,
            inputs,
            outputs: HashMap::new(),
            config,
            pending_timer: std::sync::Mutex::new(None),
        }
    }

    /// Get an input value by name
    fn get_input(&self, name: &str) -> Option<DataPacket> {
        self.inputs.get(name).cloned()
    }

    /// Get a config value by name
    fn get_config(&self, name: &str) -> Option<Value> {
        self.config.get(name).cloned()
    }

    /// Set an output value
    fn set_output(&mut self, name: &str, value: DataPacket) {
        self.outputs.insert(name.to_string(), value);
    }

    /// Get all outputs
    fn get_outputs(&self) -> HashMap<String, DataPacket> {
        self.outputs.clone()
    }

    /// Set a pending timer
    fn set_pending_timer(&self, step_id: StepId) {
        // Use the mutex to safely update the pending timer
        if let Ok(mut guard) = self.pending_timer.lock() {
            *guard = Some(step_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repository::memory::{
        MemoryComponentStateRepository, MemoryFlowDefinitionRepository,
        MemoryFlowInstanceRepository, MemoryTimerRepository,
    };
    use crate::domain::step::DefaultConditionEvaluator;
    use crate::ComponentExecutorBase;
    use serde_json::json;

    struct MockEventHandler;

    #[async_trait]
    impl DomainEventHandler for MockEventHandler {
        async fn handle_event(&self, _event: Box<dyn DomainEvent>) -> Result<(), CoreError> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct MockComponent {
        name: String,
    }

    impl MockComponent {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    impl ComponentExecutorBase for MockComponent {
        fn component_type(&self) -> &str {
            &self.name
        }
    }

    #[async_trait]
    impl ComponentExecutor for MockComponent {
        async fn execute(&self, api: Arc<dyn ComponentRuntimeApi>) -> ExecutionResult {
            // Get inputs
            let input_value = match api.get_input("input").await {
                Ok(input) => input,
                Err(_) => DataPacket::null(),
            };

            // Log
            ComponentRuntimeApiBase::log(
                &api,
                tracing::Level::INFO,
                &format!("Executing component: {}", self.name),
            );

            // Set output
            let output = json!({
                "componentName": self.name,
                "processedInput": input_value.as_value(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });

            if let Err(e) = api.set_output("result", DataPacket::new(output)).await {
                return ExecutionResult::Failure(e);
            }

            ExecutionResult::Success
        }
    }

    #[tokio::test]
    async fn test_flow_execution_service() {
        // Create repositories
        let flow_instance_repo = Arc::new(MemoryFlowInstanceRepository::new());
        let flow_definition_repo = Arc::new(MemoryFlowDefinitionRepository::new());
        let component_state_repo = Arc::new(MemoryComponentStateRepository::new());
        let (timer_repo, _timer_rx) = MemoryTimerRepository::new();
        let timer_repo: Arc<dyn TimerRepository> = Arc::new(timer_repo);

        // Create component factory
        let component_factory: ComponentFactory = Arc::new(
            |component_type: &str| -> Result<Arc<dyn ComponentExecutor>, CoreError> {
                Ok(Arc::new(MockComponent::new(component_type)))
            },
        );

        // Create condition evaluator
        let condition_evaluator = Arc::new(DefaultConditionEvaluator);

        // Create event handler
        let event_handler = Arc::new(MockEventHandler);

        // Create service
        let service = FlowExecutionService::new(
            flow_instance_repo.clone(),
            flow_definition_repo.clone(),
            component_state_repo.clone(),
            timer_repo.clone(),
            component_factory,
            condition_evaluator,
            event_handler,
        );

        // Create a test flow definition
        use crate::domain::flow_definition::{
            DataReference, FlowDefinition, StepDefinition, TriggerDefinition,
        };

        let flow_id = FlowId("test_flow".to_string());
        let flow_definition = FlowDefinition {
            id: flow_id.clone(),
            version: "1.0.0".to_string(),
            name: "Test Flow".to_string(),
            description: Some("A test flow".to_string()),
            steps: vec![
                StepDefinition {
                    id: "step1".to_string(),
                    component_type: "TestComponentA".to_string(),
                    input_mapping: {
                        let mut map = HashMap::new();
                        map.insert(
                            "input".to_string(),
                            DataReference {
                                expression: "$flow.trigger_data".to_string(),
                            },
                        );
                        map
                    },
                    config: json!({"param": "value"}),
                    run_after: vec![],
                    condition: None,
                },
                StepDefinition {
                    id: "step2".to_string(),
                    component_type: "TestComponentB".to_string(),
                    input_mapping: {
                        let mut map = HashMap::new();
                        map.insert(
                            "input".to_string(),
                            DataReference {
                                expression: "$steps.step1.result".to_string(),
                            },
                        );
                        map
                    },
                    config: json!({}),
                    run_after: vec!["step1".to_string()],
                    condition: None,
                },
            ],
            trigger: Some(TriggerDefinition {
                trigger_type: "http".to_string(),
                config: json!({"method": "POST", "path": "/trigger"}),
            }),
        };

        // Save flow definition
        flow_definition_repo.save(&flow_definition).await.unwrap();

        // Start a flow instance
        let trigger_data = DataPacket::new(json!({"test": "data"}));
        let instance_id = service
            .start_flow(flow_id.clone(), trigger_data)
            .await
            .unwrap();

        // Allow the flow to execute
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check results
        let flow_instance = flow_instance_repo
            .find_by_id(&instance_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(flow_instance.status, FlowStatus::Completed);
        assert!(flow_instance
            .completed_steps
            .contains(&StepId("step1".to_string())));
        assert!(flow_instance
            .completed_steps
            .contains(&StepId("step2".to_string())));

        // Check step outputs
        assert!(flow_instance
            .step_outputs
            .contains_key(&StepId("step1.result".to_string())));
        assert!(flow_instance
            .step_outputs
            .contains_key(&StepId("step2.result".to_string())));

        let step1_output = flow_instance
            .step_outputs
            .get(&StepId("step1.result".to_string()))
            .unwrap();
        let step2_output = flow_instance
            .step_outputs
            .get(&StepId("step2.result".to_string()))
            .unwrap();

        assert_eq!(step1_output.as_value()["componentName"], "TestComponentA");
        assert_eq!(step2_output.as_value()["componentName"], "TestComponentB");
    }
}

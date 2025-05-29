# Cascade Core

> ⚠️ **API Migration in Progress**: This library is undergoing a significant refactoring to improve architecture and modularity. The infrastructure module is now deprecated, and new trait-based interfaces have been introduced. See the [MIGRATION.md](./MIGRATION.md) document for details and migration guide.

The core runtime engine for the Cascade Platform, providing a minimal, abstract, and highly extensible runtime for executing reactive workflows defined by the Cascade DSL.

## Architecture

Cascade Core follows Domain-Driven Design (DDD) principles and is structured into the following key areas:

### Domain Layer

The heart of the system with core business logic:

- **Flow Instance**: The main aggregate representing the state of a flow execution.
- **Flow Definition**: The structure that defines a flow's steps, connections, and behavior.
- **Step Execution**: Logic for executing individual steps within a flow.
- **Domain Events**: Event objects for tracking flow lifecycle changes.

### Application Layer

Coordinates the application tasks and delegates to domain objects:

- **Flow Execution Service**: Orchestrates the execution of a flow instance.
- **Flow Definition Service**: Manages the lifecycle of flow definitions.
- **Runtime Interface**: The main API exposed to external systems.

### Infrastructure Layer

Provides technical capabilities to support the layers above:

- **Repository Implementations**: Concrete storage implementations for flow state.
- **Timer Services**: Implementations for scheduling delayed actions.

## Core Concepts

- **Flow Instance**: A single execution of a flow definition, with its own state and lifecycle.
- **Component**: A reusable piece of logic that can be executed as part of a flow.
- **Step**: A single execution of a component within a flow instance.
- **Data Packet**: The unit of data that flows between steps.
- **Trigger**: An event that initiates a flow execution.
- **Condition**: Logic that determines if a step should be executed.

## Usage

```rust
// Create repositories and services
let flow_instance_repo = Arc::new(MemoryFlowInstanceRepository::new());
let flow_definition_repo = Arc::new(MemoryFlowDefinitionRepository::new());
let component_state_repo = Arc::new(MemoryComponentStateRepository::new());
let (timer_repo, _timer_rx) = MemoryTimerRepository::new();
let timer_repo = Arc::new(timer_repo);

// Create component factory
let component_factory = Arc::new(|component_type: &str| -> Result<Arc<dyn ComponentExecutor>, CoreError> {
    match component_type {
        "MyComponent" => Ok(Arc::new(MyComponent::new())),
        _ => Err(CoreError::ComponentNotFoundError(format!("Component not found: {}", component_type))),
    }
});

// Create condition evaluator
let condition_evaluator = Arc::new(DefaultConditionEvaluator);

// Create event handler
let event_handler = Arc::new(LoggingEventHandler::new());

// Create flow execution service
let flow_execution_service = Arc::new(FlowExecutionService::new(
    flow_instance_repo.clone(),
    flow_definition_repo.clone(),
    component_state_repo.clone(),
    timer_repo.clone(),
    component_factory,
    condition_evaluator,
    event_handler,
));

// Create flow definition service
let flow_definition_service = Arc::new(FlowDefinitionService::new(
    flow_definition_repo.clone(),
    flow_instance_repo.clone(),
));

// Create runtime interface
let runtime = RuntimeInterface::new(
    flow_execution_service,
    flow_definition_service,
);

// Deploy a flow definition
runtime.deploy_definition(my_flow_definition).await?;

// Trigger a flow execution
let instance_id = runtime.trigger_flow(
    flow_id,
    DataPacket::new(json!({"input": "value"}))
).await?;
```

### Alternative: Using Factory Method with PostgreSQL

For production usage, you can use the factory method with PostgreSQL persistence:

```rust
// Create component factory
let component_factory = Arc::new(|component_type: &str| -> Result<Arc<dyn ComponentExecutor>, CoreError> {
    match component_type {
        "MyComponent" => Ok(Arc::new(MyComponent::new())),
        _ => Err(CoreError::ComponentNotFoundError(format!("Component not found: {}", component_type))),
    }
});

// Create event handler
let event_handler = Arc::new(LoggingEventHandler::new());

// Create runtime with PostgreSQL repositories
let runtime = RuntimeInterface::create(
    RepositoryType::Postgres("postgres://user:password@localhost:5432/cascade".to_string()),
    component_factory,
    event_handler,
).await?;

// Use the runtime as before
runtime.deploy_definition(my_flow_definition).await?;
```

## Creating Components

Components are created by implementing the `ComponentExecutor` trait:

```rust
#[derive(Debug)]
struct MyComponent;

#[async_trait]
impl ComponentExecutor for MyComponent {
    async fn execute(&self, api: &dyn ComponentRuntimeApi) -> ExecutionResult {
        // Get input
        let input = match api.get_input("my_input").await {
            Ok(data) => data,
            Err(e) => return ExecutionResult::Failure(e),
        };
        
        // Process input
        let result = process_data(input.as_value());
        
        // Set output
        if let Err(e) = api.set_output("my_output", DataPacket::new(result)).await {
            return ExecutionResult::Failure(e);
        }
        
        ExecutionResult::Success
    }
}
```

## Testing

Cascade Core is extensively tested using multiple levels of testing:

### Unit Tests

Individual components and domain objects are tested in isolation:

```rust
#[test]
fn test_flow_instance_creation() {
    let flow_id = FlowId("test_flow".to_string());
    let trigger_data = DataPacket::new(json!({"input": "value"}));
    
    let instance = FlowInstance::new(flow_id.clone(), trigger_data.clone());
    
    assert_eq!(instance.flow_id, flow_id);
    assert_eq!(instance.status, FlowStatus::Initializing);
}
```

### Integration Tests

Tests that verify different components work correctly together:

```rust
#[tokio::test]
async fn test_flow_execution_service() {
    let (service, repos) = create_test_service();
    let flow_def = create_test_flow_definition();
    
    service.deploy_definition(flow_def.clone()).await.unwrap();
    let instance_id = service.start_flow(flow_def.id, trigger_data).await.unwrap();
    
    // Verify results
    let instance = repos.find_by_id(&instance_id).await.unwrap().unwrap();
    assert_eq!(instance.status, FlowStatus::Completed);
}
```

### BDD Tests

Behavior-Driven Development tests that verify business requirements:

```rust
#[tokio::test]
async fn scenario_successful_calculation() {
    // Given a calculator flow is deployed
    let (runtime, repos) = create_test_environment();
    let calc_flow = create_calculator_flow();
    runtime.deploy_definition(calc_flow).await.unwrap();
    
    // When I perform a calculation with valid inputs
    let instance_id = runtime.trigger_flow(
        calc_flow.id, 
        DataPacket::new(json!({"a": 5, "b": 3, "operation": "add"}))
    ).await.unwrap();
    
    // Then the calculation completes successfully
    let instance = repos.find_by_id(&instance_id).await.unwrap().unwrap();
    assert_eq!(instance.status, FlowStatus::Completed);
    
    // And the result is correct
    let result = instance.step_outputs.get(&"calculate.result".into()).unwrap();
    assert_eq!(result.as_value(), json!(8));
}
```

## Extending Cascade Core

The core is designed for extensibility:

1. **Custom Components**: Implement the `ComponentExecutor` trait.
2. **Custom Repositories**: Implement the repository interfaces in the domain layer.
3. **Custom Condition Evaluators**: Implement the `ConditionEvaluator` trait.
4. **Custom Event Handlers**: Implement the `DomainEventHandler` trait.

## State Store

The Core Runtime uses a pluggable state store system to persist flow instance state, flow definitions, component state, and timers. By default, two implementations are provided:

1. **Memory State Store**: For testing and development environments. All data is stored in memory and lost on restart.

2. **PostgreSQL State Store**: Now moved to the separate `cascade-state-postgres` crate. This provides a production-ready implementation that stores data in a PostgreSQL database.

### Using the PostgreSQL State Store

To use the PostgreSQL state store, add the `cascade-state-postgres` crate as a dependency and configure it like this:

```rust
use cascade_core::RuntimeInterface;
use cascade_state_postgres::PostgresStateStoreProvider;

// Create the PostgreSQL state store provider
let postgres_provider = PostgresStateStoreProvider::new("postgres://user:password@localhost/cascade").await?;

// Create the runtime with the PostgreSQL state store
let runtime = RuntimeInterface::create_with_repositories(
    postgres_provider.create_repositories().await?,
    component_factory,
    event_handler,
).await?;

// Use the runtime as normal
runtime.deploy_definition(flow_definition).await?;
```

This approach ensures that the Core Runtime remains database-agnostic, while providing a production-ready PostgreSQL implementation.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
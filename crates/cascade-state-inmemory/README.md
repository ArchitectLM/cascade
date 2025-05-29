# Cascade State In-Memory

An in-memory implementation of the state store interfaces for the Cascade Platform. This crate provides a thread-safe, in-memory state storage solution that is primarily designed for development and testing purposes.

## Features

- Implements all repository interfaces defined in the `cascade-core` crate:
  - `FlowInstanceRepository`
  - `FlowDefinitionRepository`
  - `ComponentStateRepository`
  - `TimerRepository`
  - `CorrelationRepository`
- Thread-safe storage using `Arc<RwLock<HashMap>>` for all repositories
- Timers that work with the Cascade Core runtime for time-based flow execution
- No external dependencies - everything is stored in memory
- Fast and deterministic behavior ideal for testing and development
- Simple API that matches the repository interfaces expected by Cascade Core

## Usage

### Basic Setup

```rust
use cascade_core::RuntimeInterface;
use cascade_state_inmemory::InMemoryStateStoreProvider;

// Create the in-memory state store provider
let inmemory_provider = InMemoryStateStoreProvider::new();

// Get the repositories
let repositories = inmemory_provider.create_repositories();

// Create the runtime with the in-memory repositories
let runtime = RuntimeInterface::create_with_repositories(
    repositories,
    component_factory,  // Your component factory
    event_handler,      // Your event handler
).await?;

// Now you can use the runtime with the in-memory state store
```

### Timer Processing

The in-memory state store includes timer functionality that can be used to schedule and trigger time-based events in flows:

```rust
// Create the provider
let mut inmemory_provider = InMemoryStateStoreProvider::new();

// Start the timer processor with a callback function
inmemory_provider.start_timer_processor(|timer_event| {
    // Handle the timer event
    println!("Timer triggered for flow: {}, step: {}", 
        timer_event.flow_instance_id.0, 
        timer_event.step_id.0);
    Ok(())
})?;
```

## Development vs. Production

This crate is primarily intended for development, testing, and local execution environments. For production use cases, consider using a persistent state store implementation with a database backend.

Benefits for development:
- No database setup required
- Fast execution
- Deterministic behavior for tests
- Simple configuration

## License

MIT 
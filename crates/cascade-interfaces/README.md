# Cascade Interfaces

This crate provides shared interface types for integration between different Cascade components, particularly focusing on the Knowledge Base and Agent systems.

## Purpose

The primary purpose of this crate is to define the contracts between the Conversational DSL Agent System (CDAS) and the Knowledge Base (KB) components of the Cascade platform. By isolating these interfaces in a separate crate, we:

1. Decouple the CDAS and KB implementations, allowing them to evolve independently
2. Provide clear contracts for implementations to adhere to
3. Enable easy mocking for testing
4. Facilitate potential third-party implementations

## Key Features

* **Knowledge Base Client Interface**: The `KnowledgeBaseClient` trait defines the contract for retrieving DSL element details, searching related artifacts, validating DSL code, and tracing data flow.
* **Structured Types**: Well-defined types like `GraphElementDetails`, `RetrievedContext`, and `ValidationErrorDetail` that form the backbone of CDAS-KB interactions.
* **Error Handling**: Standardized error types and result wrappers for KB operations.

## Usage

Add this crate as a dependency in your `Cargo.toml`:

```toml
[dependencies]
cascade-interfaces = { path = "../path/to/cascade-interfaces" }
```

Then use the interfaces in your code:

```rust
use cascade_interfaces::kb::{KnowledgeBaseClient, GraphElementDetails, RetrievedContext};

// Implement the trait for your specific KB client
#[async_trait]
impl KnowledgeBaseClient for MyKbClient {
    async fn get_element_details(&self, tenant_id: &str, entity_id: &str, version: Option<&str>) 
        -> KnowledgeResult<Option<GraphElementDetails>> {
        // Your implementation here
    }
    
    // ...other methods
}
```

## License

MIT OR Apache-2.0 
# Cascade Standard Component Library (StdLib)

Provides a curated set of pre-built, reusable component implementations for the Cascade Platform.

## Goal

Accelerate development by offering robust, composable building blocks for ~80-90% of common workflow tasks, including:

*   **Flow Control:** `MapData`, `FilterData`, `Switch`, `Fork`, `Join`, etc.
*   **Reliability:** `RetryWrapper`, `CircuitBreaker`, `IdempotentReceiver`, `DlqPublisher`.
*   **External Communication:** `HttpCall`, `KafkaPublisher`, `DatabaseQuery/Execute`, etc.
*   **Data Handling:** `JsonSchemaValidator`, `DataMapper`, `Serializer/Deserializer`.
*   **Stateful/Long-Running:** `WaitFor*`, `SagaCoordinator`, `SubFlowInvoker`, `ProcessVariableManager`.
*   **Stateful Patterns:** `ActorShell`, `ActorRouter` (primitives).
*   **Reactive Streams:** `SplitList`, `AggregateItems`, `Merge/ZipStreams`, `Debounce/Buffer/ThrottleInput`.
*   **Observability:** `Logger`, `AuditLogger`, `MetricsEmitter`.

## Usage

Components are referenced in the Cascade DSL using their `type` identifier (e.g., `StdLib:HttpCall`). The Cascade Core Runtime loads the corresponding implementation from this library (or its compiled form, e.g., WASM).

*(Add details on specific component contracts, configuration, runtime dependencies, and contribution guidelines)*
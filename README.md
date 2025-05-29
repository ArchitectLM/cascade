# Cascade Platform: The No-Code Reactive Workflow Engine for Edge & Server

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

**Unlock the Power of Reactive Workflows without Writing Code!**

Cascade is a comprehensive platform designed to simplify the creation, deployment, and monitoring of data pipelines and automation workflows. By combining edge and server-side processing with a declarative, no-code approach, Cascade empowers businesses and developers to build resilient, scalable applications faster and more efficiently.

## Key Benefits

*   **Rapid Workflow Development:** Design complex workflows using an intuitive YAML-based DSL, eliminating the need for extensive coding.
*   **Edge-Optimized Performance:** Distribute processing to edge devices for reduced latency and improved responsiveness, ideal for IoT, real-time data processing, and user-centric applications.
*   **Unmatched Scalability & Resilience:** Built-in support for critical resilience patterns like circuit breakers, bulkheads, and dead-letter queues ensures your workflows remain robust and fault-tolerant even under heavy load or during service disruptions.
*   **Seamless Integration:** Connect to a wide range of data sources, APIs, and messaging systems using pre-built components from the Standard Library, or easily extend the platform with custom components tailored to your specific needs.
*   **Proactive Monitoring & Insights:** Gain real-time visibility into your workflows with comprehensive monitoring dashboards, alerting, and logging, enabling you to identify and resolve issues quickly.
*   **AI-Powered Assistance:** (Future Feature) Leverage the integrated RAG Agent for intelligent assistance, DSL generation, and workflow optimization.

## Target Use Cases

*   **IoT Data Processing:** Collect, transform, and analyze data from IoT devices in real-time, triggering automated actions based on predefined conditions.
*   **Real-Time Data Pipelines:** Build data pipelines for processing streaming data from sources like Kafka, enabling real-time analytics and decision-making.
*   **API Orchestration:** Compose complex API interactions into automated workflows, streamlining business processes and reducing integration complexity.
*   **Event-Driven Automation:** Trigger automated actions based on events from various sources, such as database changes, message queue messages, or user interactions.
*   **Process Automation:** Automate complex business processes with Saga and Actor components, ensuring stateful and fault-tolerant execution.
*   **UI Definition & Rendering:** Create dynamic, server-rendered UIs with interactive islands defined through a declarative DSL, then efficiently served via edge workers.

## Technical Overview

Cascade is built on a robust, distributed architecture:

*   **Core Runtime (cascade-core):** A minimal, high-performance execution engine for reactive workflows.
*   **Standard Library (cascade-stdlib):** A rich set of pre-built components for common integration patterns, flow control, and data transformation.
*   **DSL (Declarative Flow Definition Language):** A YAML-based language for defining reactive workflows (Flows) and UI structures (UI DSL).
*   **Plugins (Custom Components):** Extensible architecture allowing developers to create custom components to integrate with any system. (WASM preferred for security).
*   **Server (cascade-server):** A central orchestration server managing deployments, providing APIs, and handling server-side processing.
*   **Edge (cascade-edge):** Edge devices (e.g., Cloudflare Workers) for distributed processing, bringing logic closer to the data source or end-user.
*   **Content-Addressed Storage (CAS/KV):** A central store for immutable artifacts (components, configurations, and manifests). This is an excellent design practice for hot deployment and fault tolerance.
*   **Hybrid Execution Model:** A balanced architecture with the ability to process data at the edge, or server based on available components and needs.

## Monitoring

Cascade includes a comprehensive monitoring stack:

*   **Metrics Collection:** Prometheus collects metrics from all platform components, providing detailed insights into performance and system health.
*   **Visualization:** Grafana dashboards provide a visual representation of key metrics:
    *   **Edge Metrics:** CPU utilization, memory usage, disk I/O, and communication statistics for edge devices.
    *   **Flow Execution Metrics:** End-to-end latency, throughput, error rates, and component execution times for individual workflows.
    *   **System Resilience:** Monitoring of circuit breaker states, retry counts, and DLQ activity.
    *   **Resource Usage Analysis:** Identify resource-intensive components or flows to optimize performance.
*   **Alerting:** AlertManager processes and routes alerts based on predefined thresholds, enabling proactive issue detection and resolution.
    *   Configure alerts based on metrics such as:
        - Average flow latency above a threshold
        - High error rates for specific components
        - Circuit breaker state changes
        - Resource exhaustion on edge devices

For detailed information about the monitoring setup, see [monitoring/README.md](monitoring/README.md).

## Resilience Patterns

Cascade implements several key resilience patterns to help build robust, fault-tolerant applications:

### Circuit Breaker Pattern

The circuit breaker pattern prevents cascading failures by temporarily stopping calls to failing services:

*   **States**: Closed (normal operation), Open (failing fast), Half-Open (testing recovery)
*   **Configuration**: Customizable failure thresholds, reset timeouts, and retry logic.
*   **Events**: Publishes state change events for monitoring and alerting.
*   **Implementation**: See `tests/fixtures/flows/circuit_breaker_flow.yaml` for example implementation.
*   **Benefit:** Prevents cascading failures and improves system stability.

### Bulkhead Pattern

The bulkhead pattern isolates components to prevent failures from propagating:

*   **Isolation**: Separate execution pools for different service tiers (e.g., premium vs. standard).
*   **Concurrency Control**: Configurable max concurrent requests and queue sizes.
*   **Overflow Handling**: Graceful rejection when a pool is saturated.
*   **Implementation**: See `tests/fixtures/flows/bulkhead_flow.yaml` for example implementation.
*   **Benefit:** Limits the impact of failures and ensures critical services remain available.

### Throttling Pattern

The throttling pattern limits the rate of requests to protect downstream services:

*   **Rate Limiting**: Configurable limits based on client ID or other selectors.
*   **Time Windows**: Rolling windows for accurate rate tracking.
*   **Rejection**: Provides informative responses with retry guidance when limits are exceeded.
*   **Implementation**: See `tests/fixtures/flows/throttling_flow.yaml` for example implementation.
*   **Benefit:** Protects downstream services from overload and ensures fair resource allocation.

### Dead Letter Queue (DLQ) Pattern

The DLQ pattern provides reliable error handling for asynchronous operations:

*   **Error Capture**: Routes failed operations to dedicated error queues.
*   **Data Sanitization**: Removes sensitive information before storing error details.
*   **Reprocessing**: Mechanisms for retrying failed operations with tracking.
*   **Implementation**: See `tests/fixtures/flows/dlq_flow.yaml` for example implementation.
*   **Benefit:** Ensures no data is lost and provides a mechanism for recovering from errors.

## Project Structure

The Cascade platform is organized into several core crates (managed within a monorepo):

*   **`cascade-core`**: Core runtime and execution engine. Responsible for interpreting the DSL and invoking components. Defines core interfaces (e.g., `ComponentRuntimeAPI`, `StateStore`, `TimerService`).
*   **`cascade-stdlib`**: Standard library components for common integration patterns. Implements the `ComponentExecutor` trait. Provides reusable building blocks for workflows.
*   **`cascade-dsl`**: DSL parsing, validation, and internal representation structs. Handles Flow DSL and UI DSL. Ensures DSL definitions are syntactically and semantically correct.
*   **`cascade-server`**: Server-side runtime implementation. Hosts the Core Runtime, provides the management API, handles deployments, and manages edge workers. Implements the core platform logic.
*   **`cascade-edge`**: Edge runtime implementation (WebAssembly-based). Executes workflow steps on edge devices. Implements the `ComponentExecutor` to run WASM components.
*   **`cascade-test-utils`**: Testing utilities and mocks. Provides mocks of external services. Provides tooling for testing integration and E2E pipelines
*   **`cascade-ui-renderer`**: The backend service that reads UI DSL and produces outputs
*   **`cascade-agent`**: The LLM based backend agent to help orchestrate/write/deploy apps.

## Getting Started

1.  **Prerequisites:**
    *   Docker
    *   Docker Compose
2.  **Clone the Repository:**
    ```bash
    git clone <repository_url>
    cd cascade-platform
    ```
3.  **Start the Platform:**
    ```bash
    docker-compose up -d
    ```

### Access Points

*   **Cascade API:** http://localhost:3000
    * Provides endpoints for deploying flows, triggering flows, managing UI definitions, and interacting with the system.

*   **Prometheus:** http://localhost:9090
    *  For monitoring platform metrics
*   **Grafana:** http://localhost:3000 (admin/admin)
    * Realtime visual dashboard for monitoring
*   **AlertManager:** http://localhost:9093
    * To setup custom alert metrics and notification rules

## Contributing

We welcome contributions to the Cascade platform! Please see our [CONTRIBUTING.md](CONTRIBUTING.md) file for more information.

## License

Apache 2.0
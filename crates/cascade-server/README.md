# Cascade Server

The main application server for the Cascade Platform.

## Responsibilities

*   Provides the primary **Admin API** (HTTP, Axum-based) for:
    *   DSL Deployment (`POST /api/v1/flows`, `PUT /api/v1/flows/{id}`)
    *   Flow Undeployment (`DELETE /api/v1/flows/{id}`)
    *   Listing/Querying Flows & Instances (`GET /api/v1/flows`, `GET /api/v1/flows/{id}`)
*   Provides the internal **Edge Callback API** endpoint for receiving execution requests from Edge Workers (`POST /api/v1/runtime/execute_step`).
*   Orchestrates flow deployments using `cascade-dsl`, `cascade-content-store`, and the Edge platform integration.
*   Handles configuration loading from environment variables and config files.
*   Sets up logging, tracing, metrics infrastructure.
*   Manages the lifecycle of the embedded or connected `cascade-core` runtime instance(s).
*   Executes server-only steps by invoking the `CoreRuntimeAPI`.

## Setup and Configuration

### Installation

```bash
# Clone the repository
git clone https://github.com/your-org/cascade.git
cd cascade

# Build the server
cargo build --release -p cascade-server
```

### Configuration

The Cascade Server is configured primarily through environment variables:

```bash
# Required Configuration
export SERVER_PORT=8080                         # Port to listen on
export SERVER_HOST=0.0.0.0                      # Host to bind to
export CONTENT_STORE_URL=memory://test          # URL for content store (memory, cloudflare, redis)
export EDGE_API_URL=cloudflare://your-account   # URL for edge platform API

# Authentication (highly recommended for production)
export ADMIN_API_KEY=your-secure-admin-key      # API key for admin endpoints
export EDGE_CALLBACK_JWT_SECRET=your-jwt-secret # Secret for edge callback JWT tokens
export EDGE_CALLBACK_JWT_ISSUER=cascade-server  # JWT issuer
export EDGE_CALLBACK_JWT_AUDIENCE=edge-workers  # JWT audience
export EDGE_CALLBACK_JWT_EXPIRY_SECONDS=3600    # JWT expiry in seconds

# Cloudflare Integration (required for Cloudflare KV and Workers)
export CLOUDFLARE_API_TOKEN=your-api-token      # Cloudflare API token

# Logging and Monitoring
export LOG_LEVEL=info                           # Log level (trace, debug, info, warn, error)
export LOG_FILTER=info,cascade=debug            # Detailed log filter for tracing-subscriber
```

### Content Store Options

The `CONTENT_STORE_URL` variable supports these formats:

- `memory://test` - In-memory storage, for development and testing only
- `cloudflare://{account_id}:{namespace_id}?token={token}` - Cloudflare KV storage
- `redis://{host}:{port}/{db}` - Redis storage (coming soon)

### Edge Platform Options

The `EDGE_API_URL` variable supports these formats:

- `cloudflare://{account_id}?token={token}` - Cloudflare Workers

### Running the Server

```bash
# Run directly
cargo run --release -p cascade-server

# Or run the built binary
./target/release/cascade-server
```

## API Documentation

### Admin API Endpoints

All Admin API endpoints require authentication with the `Authorization: Bearer {ADMIN_API_KEY}` header.

#### Deploy or Update a Flow

```
PUT /api/v1/flows/{flow_id}
Content-Type: application/yaml

# YAML content here
```

Response:
```json
{
  "flow_id": "my-flow",
  "status": "DEPLOYED",
  "manifest_version": "1678886400123",
  "message": "Flow deployed successfully"
}
```

#### Get Flow Information

```
GET /api/v1/flows/{flow_id}
```

Response:
```json
{
  "flow_id": "my-flow",
  "description": "My flow description",
  "status": "DEPLOYED",
  "last_deployed_at": "2023-03-15T12:00:00Z",
  "current_manifest_version": "1678886400123"
}
```

#### List Flows

```
GET /api/v1/flows
```

Response:
```json
{
  "flows": [
    {
      "flow_id": "flow-1",
      "status": "DEPLOYED",
      "last_deployed_at": "2023-03-15T12:00:00Z"
    },
    {
      "flow_id": "flow-2",
      "status": "DEPLOYED",
      "last_deployed_at": "2023-03-14T10:30:00Z"
    }
  ],
  "total": 2
}
```

#### Delete Flow

```
DELETE /api/v1/flows/{flow_id}
```

Response: `204 No Content`

### Edge Callback API

This endpoint is used by edge workers to execute server-side steps:

```
POST /api/v1/runtime/execute_step
Authorization: Bearer {JWT_TOKEN}
Content-Type: application/json

{
  "flow_id": "my-flow",
  "flow_instance_id": "instance-uuid",
  "step_id": "server-step-1",
  "inputs": {
    "input1": "value1",
    "input2": 123
  }
}
```

Response:
```json
{
  "outputs": {
    "output1": "result1",
    "output2": 456
  }
}
```

## Structure

*   `src/`: Main source code
    *   `api/`: Axum handlers, route definitions, request/response types
    *   `edge/`: Edge platform integration
    *   `server.rs`: Core server implementation
    *   `content_store.rs`: Content storage abstraction
    *   `config.rs`: Configuration handling
    *   `error.rs`: Error types and handling
    *   `lib.rs`: Library entry point and utilities
    *   `main.rs`: Binary entry point
*   `tests/`: Integration tests

## Development

### Running Tests

```bash
# Run unit tests
cargo test -p cascade-server

# Run integration tests
cargo test -p cascade-server --test '*'
```

### Adding Features

When implementing new features:

1. Add appropriate unit tests
2. Ensure integration tests are updated or added
3. Update API documentation in this README
4. Follow the DDD principles established in the codebase

## License

(Add license information here)

## Configuration

The server can be configured via environment variables or a configuration file. See the `ServerConfig` struct for details.

Example configuration:

```toml
# Server settings
bind_address = "127.0.0.1"
port = 3000
log_level = "info"

# Content store settings
content_store_url = "memory://test"
# content_store_url = "cloudflare://<account_id>:<namespace_id>?token=<token>"

# Edge API settings
edge_api_url = "cloudflare://<account_id>?token=<token>"

# Shared state settings
shared_state_url = "memory://test"
# shared_state_url = "redis://localhost:6379"
```

## Shared State Service

The Cascade Server includes a shared state service that provides distributed state management for components and server functions. This allows for coordination between multiple server instances and components.

The shared state service can be configured to use different backends:

- **In-memory**: For development and testing. Not distributed, only works within a single server instance.
- **Redis**: For production. Provides distributed state across multiple server instances.

### Usage

The shared state service is exposed through the `SharedStateService` trait:

```rust
pub trait SharedStateService: Send + Sync + 'static {
    async fn get(&self, scope: &str, key: &str) -> Result<Option<serde_json::Value>, ServerError>;
    async fn set(&self, scope: &str, key: &str, value: serde_json::Value) -> Result<(), ServerError>;
    async fn delete(&self, scope: &str, key: &str) -> Result<(), ServerError>;
    async fn list_keys(&self, scope: &str) -> Result<Vec<String>, ServerError>;
    async fn is_healthy(&self) -> bool;
}
```

## Resilience Patterns

The Cascade Server provides resilience patterns that can be used to build robust services:

### Rate Limiter

The rate limiter prevents excessive requests to a resource:

```rust
// Create a rate limiter with distributed state
let config = RateLimiterConfig {
    max_requests: 100,
    window_ms: 60_000, // 1 minute
    shared_state_scope: Some("api_limits".to_string()),
};

let rate_limiter = RateLimiter::new(config, shared_state);

// Create a key for the resource to limit
let key = RateLimiterKey::new("api_endpoint", Some("user_id"));

// Check if allowed
match rate_limiter.allow(&key).await {
    Ok(remaining) => {
        // Request is allowed, remaining is the number of requests remaining
        println!("Request allowed, remaining: {}", remaining);
    },
    Err(err) => {
        // Rate limit exceeded
        println!("Rate limit exceeded: {}", err);
    }
}
```

### Circuit Breaker

The circuit breaker prevents cascading failures by blocking requests to failing services:

```rust
// Create a circuit breaker with distributed state
let config = CircuitBreakerConfig {
    failure_threshold: 5,
    reset_timeout_ms: 30_000, // 30 seconds
    shared_state_scope: Some("circuit_breakers".to_string()),
};

let circuit_breaker = CircuitBreaker::new(config, shared_state);

// Create a key for the service to protect
let key = CircuitBreakerKey::new("database", Some("query"));

// Execute with circuit breaking
let result = circuit_breaker.execute(&key, || async {
    // Perform the operation
    call_external_service().await
}).await;
```

### API Middleware

The Cascade Server includes middleware for applying rate limiting to API endpoints:

```rust
// Add rate limiting to the API router
let app = axum::Router::new()
    .route("/api/v1/flows", get(list_flows).post(create_flow))
    .layer(RateLimiterLayer::new(
        RateLimiterConfig {
            max_requests: 100,
            window_ms: 60_000,
            shared_state_scope: Some("api_limits".to_string()),
        },
        shared_state
    ));
```

## Using Redis for Shared State

To use Redis as the shared state backend, include the Redis feature in your Cargo.toml:

```toml
[dependencies]
cascade-server = { version = "0.1.0", features = ["redis"] }
```

And configure the server to use Redis:

```
SHARED_STATE_URL=redis://localhost:6379
```
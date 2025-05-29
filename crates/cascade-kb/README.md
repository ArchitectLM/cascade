# Cascade Knowledge Base

The Cascade Knowledge Base is a graph-based storage system for component and flow definitions, documentation, and other metadata used by the Cascade platform.

## Architecture Overview

The Knowledge Base is designed as a modular, trait-based system with the following core components:

- **Data Model**: Graph-based schema for components, flows, and related metadata
- **Ingestion Service**: Processes DSL definitions and application state into the graph
- **Query Service**: Provides flexible query capabilities including lookups, semantic search, and graph traversal
- **State Store**: Abstraction over the graph database (Neo4j) for storage operations
- **Embedding Generator**: Creates vector embeddings for documentation to enable semantic search

The system uses multi-tenancy (`TenantId`) to isolate data between different projects and trace context (`TraceContext`) for end-to-end observability.

## Database Setup Options

The Knowledge Base uses Neo4j as its graph database. We provide two options for setting up Neo4j:

### Option 1: Docker (Recommended for Local Development)

The Docker setup provides a full-featured Neo4j Enterprise environment with vector search capabilities. This is the recommended option for local development and testing.

Key advantages:
- Full control over your database
- Vector search capabilities for semantic embeddings
- All APOC and Graph Data Science procedures available
- Easy setup with Docker Compose

See [README-DOCKER.md](README-DOCKER.md) for details.

### Option 2: Neo4j Aura (Cloud-Based)

Neo4j Aura is a cloud-based Neo4j service that can be used for testing. The free tier has limitations, including no vector search capabilities.

Key advantages:
- No local installation required
- Always available in the cloud
- Good for basic testing without vector search

See [README-AURA.md](README-AURA.md) for details.

## Quick Start

For a guided setup experience, run:

```bash
./setup.sh
```

This script will walk you through setting up either the Docker or Aura environment.

## Running Tests

We've significantly improved the testing infrastructure to make it more reliable and easier to use, especially for tests that require Neo4j.

### Quick Start: Single Command Test Setup

For a streamlined experience, run:

```bash
# Set up Docker environment and run all tests
./make-check.sh --all
```

This will:
1. Run unit tests that don't require external dependencies
2. Check if Docker is running with Neo4j container
3. Set up the environment if needed (including Neo4j container and seed data)
4. Run integration tests using the Docker Neo4j instance

### Setting Up the Environment

To set up the Docker environment without running tests:

```bash
# Set up only
./setup-docker-env.sh
```

This script:
- Checks for Docker and Docker Compose installation
- Downloads necessary Neo4j plugins
- Starts the Neo4j container with proper configuration
- Initializes the database schema and indexes
- Loads seed data for testing
- Sets up vector indexes for semantic search tests

### Test Types

The test suite includes several types of tests:

1. **Unit Tests**: Don't require external dependencies, run with:
   ```bash
   ./make-check.sh --unit-tests
   ```

2. **Integration Tests**: Require Neo4j Docker container, run with:
   ```bash
   ./make-check.sh --integration-tests
   ```

3. **Neo4j Connection Test**: Verifies basic connectivity to the Neo4j container:
   ```bash
   cargo test -p cascade-kb --test neo4j_connection_test -- --nocapture
   ```

4. **Semantic Search Tests**: Test the vector search functionality:
   ```bash
   cargo test -p cascade-kb --test semantic_search_test -- --nocapture
   ```

### Environment Variables

The tests respect the following environment variables:

- `USE_REAL_NEO4J=true` - Use a real Neo4j connection instead of fake store
- `USE_MOCK_EMBEDDING=true` - Use mock embedding service instead of real
- `USE_FAKE_STORE=true` - Force use of in-memory store instead of Neo4j
- `SKIP_VECTOR_TESTS=true` - Skip tests requiring vector capabilities
- `USE_PREDICTABLE_EMBEDDING=true` - Use deterministic embedding generator for tests

### Neo4j Docker Configuration

Our Neo4j Docker setup is configured with:

- Neo4j 5.18.0 Community Edition
- Port 17474 for HTTP access
- Port 17687 for Bolt protocol
- APOC plugins pre-installed
- Vector search capabilities enabled
- Memory settings optimized for testing

You can access the Neo4j Browser at http://localhost:17474 with credentials `neo4j`/`password`.

### Vector Search Support

The Docker Neo4j container is set up with vector search capabilities enabled. If vector capabilities aren't available, the tests will automatically use `MockEmbeddingService` to ensure tests can still pass.

### Troubleshooting

If you encounter issues:

1. Check Docker status:
   ```bash
   ./make-check.sh --docker-check
   ```

2. Restart Docker environment:
   ```bash
   ./setup-docker-env.sh
   ```

3. Run with verbose logging:
   ```bash
   RUST_LOG=debug cargo test -p cascade-kb
   ```

4. Examine Neo4j logs:
   ```bash
   docker logs cdas-kb-test-neo4j
   ```

### Continuous Integration

The tests are designed to work well in CI environments. In CI contexts, use:

```bash
# In CI pipeline
./setup-docker-env.sh
./run-docker-tests.sh
```

## Core Workflows

The Knowledge Base supports two primary workflows:

### 1. Ingestion Workflow

The ingestion process is responsible for:
- Parsing and validating Cascade YAML definitions
- Transforming parsed data into the graph model
- Generating embeddings for documentation chunks
- Maintaining versioning of components and flows
- Storing application state snapshots

### 2. Query Workflow

The query service provides multiple query types:
- Lookup by ID (entity lookup with versioning support)
- Semantic search (vector similarity for documentation)
- Graph traversal (relationship-based queries)

## Vector Search Capabilities

The Docker setup includes full vector search capabilities using the Neo4j Graph Data Science library. This enables semantic search using embeddings from large language models.

Vector search features:
- Vector indexing for fast nearest-neighbor lookup
- Cosine similarity for semantic matching
- 1536-dimensional vectors for OpenAI embeddings

When the Neo4j vector search plugin isn't available, the system can fall back to the `MockEmbeddingService` for testing purposes.

## Database Schema

The Knowledge Base uses a graph schema with the following key components:

- **Framework**: Singleton node representing the Cascade platform
- **DSLSpec**: Definitions of the Cascade DSL specification
- **VersionSet**: Logical entity grouping versions of components/flows
- **Version**: Specific version of a component or flow
- **ComponentDefinition**: Component definitions with inputs/outputs
- **FlowDefinition**: Flow definitions with steps, triggers, and conditions
- **DocumentationChunk**: Documentation with vector embeddings for semantic search
- **CodeExample**: Example usage of components or flows
- **ApplicationStateSnapshot**: Optional application state

Relationships between these entities (like `LATEST_VERSION`, `REFERENCES_COMPONENT_VERSION`, `RUN_AFTER`, `MAPS_INPUT`) define the connections and dependencies.

## Error Handling

The Knowledge Base uses a structured error handling approach with specific error types:
- `CoreError`: Base error type for core operations
- `StateStoreError`: Specific to graph database interactions
- `ComponentLoaderError`: For component loading issues

## For Developers

For more detailed information, see:
- E2E test documentation: [tests/e2e/README.md](tests/e2e/README.md)
- Docker setup details: [README-DOCKER.md](README-DOCKER.md)
- Neo4j Aura setup: [README-AURA.md](README-AURA.md)
- Implementation specification in the source code

## Testing

The Cascade KB crate includes a comprehensive test suite with both unit tests (using mocks) and integration tests (using Docker).

### Testing Approach

We've separated the tests into two categories:

1. **Unit Tests** - These use mocks and don't require external dependencies like Neo4j
2. **Integration Tests** - These require a running Neo4j Docker container

### Running Tests

To run the tests, you can use the `make-check.sh` script:

```bash
# Show help
./make-check.sh

# Run unit tests only (no Docker required)
./make-check.sh --unit-tests

# Run integration tests only (requires Docker)
./make-check.sh --integration-tests

# Run all tests (unit tests first, then integration tests)
./make-check.sh --all

# Check if Docker is running and Neo4j container is available
./make-check.sh --docker-check
```

Alternatively, you can run the tests directly using cargo:

```bash
# Run unit tests only
cargo test -p cascade-kb --lib --features mocks

# Run integration tests with Docker
./run-docker-tests.sh
```

### Setting Up for Development

1. **Install Docker** - Required for integration tests
2. **Start the Neo4j container**:
   ```bash
   docker-compose up -d neo4j
   ```
3. **Initialize the database**:
   ```bash
   ./data/init-docker-db.sh
   ```
4. **Load seed data**:
   ```bash
   ./load_seed_data.sh
   ```

Now you can run tests and develop against the Neo4j container. 
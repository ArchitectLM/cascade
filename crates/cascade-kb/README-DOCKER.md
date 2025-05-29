# Using Docker for Cascade Knowledge Base with Vector Search

This document explains how to use Docker to run Neo4j with vector search capabilities for the Cascade Knowledge Base.

## Docker Setup

The Docker setup includes:

1. Neo4j Community Edition (5.26.6) with native vector index support
2. APOC extensions for advanced Cypher operations
3. Proper volume configuration to persist data between restarts
4. Configuration for vector indexes

## Getting Started

### Prerequisites

- Docker and Docker Compose installed
- Basic familiarity with Neo4j
- Rust development environment (for running the tests)

### Starting the Docker Environment

To start the Neo4j instance with a single command:

```bash
docker-compose up -d
```

This will start the Neo4j container in detached mode. The database will be accessible at:

- Browser UI: http://localhost:17474
- Bolt: neo4j://localhost:17687
- Username: neo4j
- Password: password

### Initializing the Database

After starting Neo4j, you need to initialize it with the proper schema:

```bash
./data/init-docker-db.sh
```

This script will:
1. Create necessary indexes for efficient querying
2. Create vector indexes for embedding-based search
3. Initialize the base nodes required by the Knowledge Base

### Running Tests with Docker

To run all the Neo4j-dependent tests with Docker:

```bash
./run-docker-tests.sh
```

This script will:
1. Check if the Neo4j container is running, and start it if needed
2. Download required APOC plugins if not already present
3. Set the appropriate environment variables for connecting to Neo4j
4. Run the connection test to verify connectivity
5. Create vector indexes for testing
6. Run all tests with mock embedding generation

## Working with Vector Search

The Docker setup includes Neo4j 5.26.6 Community Edition which provides native vector index capabilities:

1. Vector indexes are created with 1536 dimensions (suitable for OpenAI embeddings)
2. The similarity function is set to 'cosine' for standard semantic search
3. For testing, we use mock embeddings instead of calling external APIs
4. The `embedding_neo4j_test.rs` verifies that vector search is working correctly

## How Vector Search Works in Neo4j Community Edition

In Neo4j 5.26.6 Community Edition:

1. Vector indexes are created using native Neo4j Cypher commands:
   ```cypher
   CREATE VECTOR INDEX my_index IF NOT EXISTS 
   FOR (n:Node) 
   ON (n.embedding)
   OPTIONS {indexConfig: {
     `vector.dimensions`: 1536,
     `vector.similarity_function`: 'cosine'
   }}
   ```

2. Embeddings are stored directly as properties on nodes:
   ```cypher
   CREATE (n:Document {
     id: 'doc123',
     content: 'Some text',
     embedding: [0.1, 0.2, 0.3, ...]  // Vector with 1536 dimensions
   })
   ```

3. Vector similarity search is performed using the built-in vector functions:
   ```cypher
   MATCH (n:Document)
   WHERE n.embedding IS NOT NULL
   WITH n, 
     vector.similarity.cosine(n.embedding, $query_embedding) AS score
   RETURN n, score
   ORDER BY score DESC
   LIMIT 5
   ```

For testing purposes, mock embeddings are generated without requiring external API calls.

## Using Docker for Local Development

For a complete development environment that includes both Neo4j and the Cascade KB application:

```bash
docker-compose -f docker-compose-local-dev.yml up -d
```

This will:
1. Start Neo4j with vector search capabilities
2. Build and start the application container
3. Mount your local source code for live code editing

## Cleaning Up Docker Resources

To clean up Docker resources when they're no longer needed:

```bash
./cleanup.sh
```

This script provides two options:
1. Clean containers only (preserves data volumes)
2. Clean containers and volumes (removes all data)

## Troubleshooting

If you encounter issues with the Docker setup:

1. **Neo4j not starting**: Check Docker logs with `docker logs cdas-kb-test-neo4j`
2. **Vector search not working**: Verify vector indexes are properly created:
   ```bash
   docker exec cdas-kb-test-neo4j cypher-shell -u neo4j -p password "SHOW INDEXES YIELD type WHERE type = 'VECTOR'"
   ```
3. **Connection issues**: Ensure ports 17474 and 17687 are available on your machine
4. **Plugin issues**: Ensure the APOC plugins are correctly downloaded and mounted:
   ```bash
   docker exec cdas-kb-test-neo4j ls -la /plugins
   ```
5. **Test failures**: Run with increased logging:
   ```bash
   RUST_LOG=cascade_kb=debug,neo4j_connection_test=debug,embedding_neo4j_test=debug ./run-docker-tests.sh
   ```

## Manual Testing Vector Indexes

You can manually test the vector index functionality:

```cypher
// Create a vector index
CREATE VECTOR INDEX manual_test_index IF NOT EXISTS 
FOR (n:ManualTest) 
ON (n.embedding)
OPTIONS {indexConfig: {
  `vector.dimensions`: 1536,
  `vector.similarity_function`: 'cosine'
}}

// Store a node with a vector
CREATE (n:ManualTest {
  id: 'test1',
  name: 'Test Node',
  embedding: [0.1, 0.2, 0.3, 0.4, 0.5, /* ... more values ... */]
})

// Query using vector similarity
MATCH (n:ManualTest)
WHERE n.embedding IS NOT NULL
WITH n, vector.similarity.cosine(n.embedding, [0.2, 0.3, 0.4, 0.5, 0.6, /* ... */]) AS score
RETURN n.id, n.name, score
ORDER BY score DESC
LIMIT 5
```

# Neo4j Docker Setup for Knowledge Base Testing

This document outlines how to properly set up the Neo4j Docker container for running tests against the Cascade Knowledge Base.

## Prerequisites

- Docker and Docker Compose
- An OpenAI API key for embedding functionality (optional but recommended)

## Quick Start

1. Set your OpenAI API key as an environment variable:
   ```bash
   export OPENAI_API_KEY=your_openai_api_key
   ```

2. Start the Neo4j container:
   ```bash
   cd crates/cascade-kb
   docker-compose up -d neo4j
   ```

3. Wait for the container to start (usually takes 10-20 seconds)

4. Run the initialization script:
   ```bash
   ./data/init-docker-db.sh
   ```

5. Run the tests:
   ```bash
   ./run-docker-tests.sh
   ```

## Required Neo4j Plugins

The cascade-kb tests require the following Neo4j plugins:

1. **APOC Core**: For general utility procedures
2. **Graph Data Science (GDS)**: For vector operations and similarity search
3. **APOC ML OpenAI**: For embedding generation

The docker-compose.yml file is configured to automatically load these plugins when the container starts.

## Configuration Details

The Neo4j container uses the following critical configuration:

- Neo4j Enterprise Edition (required for GDS plugin)
- APOC plugin with ML capabilities
- GDS plugin for vector search
- Proper security settings to allow procedures to run

## Troubleshooting

If tests fail with errors about missing plugins or procedures, try:

1. Ensure you're using the Enterprise edition of Neo4j
2. Check if the plugins were properly loaded:
   ```bash
   docker exec cdas-kb-test-neo4j ls -la /var/lib/neo4j/plugins
   ```
3. Verify the APOC ML procedures are available:
   ```bash
   docker exec cdas-kb-test-neo4j cypher-shell -u neo4j -p password "CALL dbms.procedures() YIELD name WHERE name CONTAINS 'apoc.ml' RETURN name"
   ```

If you see errors about OpenAI API key:

1. Make sure you've set the OPENAI_API_KEY environment variable
2. Restart the container:
   ```bash
   docker-compose down
   docker-compose up -d neo4j
   ```

## Custom Neo4j Setup

If you want to use an existing Neo4j instance, you need to ensure:

1. Neo4j Enterprise edition is installed
2. APOC and GDS plugins are installed
3. APOC ML procedures are enabled
4. OpenAI API key is configured in neo4j.conf

Then update your environment variables:
```bash
export NEO4J_URI=neo4j://your_host:your_port
export NEO4J_USERNAME=your_username
export NEO4J_PASSWORD=your_password
```

# Neo4j Docker Setup for Cascade KB with Vector Search

This document explains how to set up a Neo4j Docker container for testing the Cascade Knowledge Base (KB) system with vector search capabilities.

## Prerequisites

- Docker and Docker Compose
- Rust development environment

## Neo4j Version Information

The testing infrastructure uses Neo4j 5.26.6 Community Edition with vector index capabilities:

- **Neo4j Version**: 5.26.6-community
- **Plugins**: APOC Core
- **Vector Features**: Native vector indexes with cosine similarity

## Running Tests

1. Start the Neo4j container:
   ```bash
   cd crates/cascade-kb
   docker-compose up -d
   ```

2. Wait for Neo4j to initialize (about 30 seconds)

3. Run the tests:
   ```bash
   cd crates/cascade-kb
   ./run-docker-tests.sh
   ```

## How Vector Search Works

In Neo4j 5.26.6 Community Edition:

1. The Community Edition natively supports vector indexes
2. Vector embeddings are stored directly in Neo4j nodes
3. Vector similarity search is performed using Neo4j's vector index functionality 
4. For testing purposes, we use mock embeddings to populate the vector fields

The tests demonstrate:
- Creating vector indexes with specified dimensions and similarity functions
- Storing embedding vectors in Neo4j nodes
- Retrieving nodes based on properties and vector data

## Testing Approach

For testing the KB with vector search capabilities:

1. The test framework creates vector indexes for `TestEmbedding` and `Content` node types
2. Mock embedding vectors (1536 dimensions) are generated in-memory
3. These vectors are stored in Neo4j nodes and retrieved to verify persistence
4. No external API calls are required for embedding generation during tests

## Troubleshooting

If you encounter issues:

1. Check if the Neo4j container is running:
   ```bash
   docker ps | grep cdas-kb-test-neo4j
   ```

2. Check the Neo4j logs:
   ```bash
   docker logs cdas-kb-test-neo4j
   ```

3. Verify vector index capabilities:
   ```bash
   docker exec cdas-kb-test-neo4j cypher-shell -u neo4j -p password "SHOW INDEXES YIELD type WHERE type = 'VECTOR'"
   ```

4. Ensure Neo4j is running with sufficient memory 

## Manual Testing

You can manually test the vector index functionality:

```cypher
// Create a vector index
CREATE VECTOR INDEX manual_test_index IF NOT EXISTS 
FOR (n:ManualTest) 
ON (n.embedding)
OPTIONS {indexConfig: {
  `vector.dimensions`: 1536,
  `vector.similarity_function`: 'cosine'
}}

// Store a node with a vector
CREATE (n:ManualTest {
  id: 'test1',
  name: 'Test Node',
  embedding: [0.1, 0.2, 0.3, 0.4, 0.5]
})

// Query using basic properties
MATCH (n:ManualTest {id: 'test1'})
RETURN n.name, n.embedding
```

## Configuration

The docker-compose.yml file configures Neo4j with:

- APOC plugin for extended Cypher procedures
- Memory settings optimized for testing
- Volume mounts for data persistence
- Proper security configuration for running procedures 
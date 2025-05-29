# Using Neo4j Aura for Cascade Knowledge Base Tests

This document explains how to use Neo4j Aura as a test database for the Cascade Knowledge Base.

> **Note**: For vector search capabilities, we recommend using the Docker setup instead of Neo4j Aura Free. See [README-DOCKER.md](README-DOCKER.md) for details.

## Neo4j Aura Configuration

The tests are configured to use a Neo4j Aura Free instance with the following connection details:

```
URI: neo4j+s://dd132cc4.databases.neo4j.io
Username: neo4j
Password: 77iJ56VLGXj9jIGO5e8EZUFV2mniD9IuquuwaD0TNp0
Database: neo4j
```

> **Note**: This is a shared Neo4j Aura Free instance, which has limitations. If you need more control or resources, consider creating your own Neo4j Aura instance.

## Running Tests with Neo4j Aura

We provide a convenience script to run all the Neo4j-dependent tests:

```bash
./run-aura-tests.sh
```

This script will:
1. Set the appropriate environment variables for connecting to Neo4j Aura
2. Run the connection test to verify connectivity
3. Run the embedding test (may be skipped if Neo4j Aura Free doesn't support the GenAI plugin)
4. Run the E2E tests that require Neo4j

## Initializing the Database

If you need to initialize the Neo4j Aura instance with the necessary indexes and schema, you can run:

```bash
./data/init-aura-db.sh
```

This script will:
1. Create necessary indexes for efficient querying
2. Try to create vector indexes (requires the GenAI plugin, which may not be available in Neo4j Aura Free)
3. Create base nodes required by the Knowledge Base

> **Note**: You need to have `cypher-shell` installed to run this script. You can install it from the [Neo4j website](https://neo4j.com/docs/operations-manual/current/tools/cypher-shell/).

## Limitations of Neo4j Aura Free

The Neo4j Aura Free instance has several limitations:

1. No support for certain plugins, including the GenAI plugin required for vector search
2. Limited storage (1GB) and connections
3. The database is shared among developers, so concurrent test runs may interfere with each other
4. Neo4j Aura Free does not support certain features like APOC procedures

## Using Your Own Neo4j Aura Instance

If you need to use your own Neo4j Aura instance, you can override the connection details using environment variables:

```bash
NEO4J_URI="neo4j+s://your-instance.databases.neo4j.io" \
NEO4J_USERNAME="your-username" \
NEO4J_PASSWORD="your-password" \
NEO4J_DATABASE="your-database" \
NEO4J_POOL_SIZE=5 \
cargo test -p cascade-kb --features adapters --test e2e::neo4j_e2e_test -- --nocapture
```

## Troubleshooting

If you encounter issues with the Neo4j Aura connection:

1. **Connection timeout**: Check your internet connection and the Neo4j Aura instance status
2. **Authentication failure**: Verify the username and password
3. **Database not found**: Make sure the database name is correct
4. **Embedding test failures**: These are expected if the Neo4j Aura instance doesn't have the GenAI plugin installed
5. **Concurrent test runs**: If multiple developers are running tests simultaneously, you may encounter conflicts

For additional troubleshooting, run tests with increased logging:

```bash
RUST_LOG=cascade_kb=debug,neo4j_e2e_test=debug,neo4j_connection_test=debug \
./run-aura-tests.sh
```

## Switching Between Local and Aura Testing

You can use either Docker-based local testing or Neo4j Aura cloud testing:

1. For local Docker-based testing (with vector search): `./run-docker-tests.sh`
2. For Neo4j Aura cloud testing: `./run-aura-tests.sh`

For quick setup of either environment, run:

```bash
./setup.sh 
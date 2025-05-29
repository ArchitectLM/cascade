# E2E Tests for Cascade Knowledge Base

This directory contains end-to-end tests for the Cascade Knowledge Base, including tests that use a real Neo4j database connection.

## Running the Tests

### Standard Tests

Run all tests including E2E tests:

```bash
cargo test -p cascade-kb
```

Run only E2E tests:

```bash
cargo test -p cascade-kb --test e2e
```

### Neo4j Connection Tests

The tests are now configured to use a Neo4j Aura instance by default:

```
URI: neo4j+s://dd132cc4.databases.neo4j.io
Username: neo4j
Password: 77iJ56VLGXj9jIGO5e8EZUFV2mniD9IuquuwaD0TNp0
Database: neo4j
Pool Size: 10
```

To run the tests with this Neo4j Aura configuration, simply run:

```bash
cargo test -p cascade-kb --features adapters --test neo4j_connection_test -- --nocapture
cargo test -p cascade-kb --features adapters --test e2e::neo4j_e2e_test -- --nocapture
```

#### Using Custom Neo4j Connection

If you want to use a different Neo4j instance, you can override the connection settings using environment variables:

```bash
NEO4J_URI="neo4j+s://your-instance.databases.neo4j.io" \
NEO4J_USERNAME="your-username" \
NEO4J_PASSWORD="your-password" \
NEO4J_DATABASE="your-database" \
NEO4J_POOL_SIZE=5 \
cargo test -p cascade-kb --features adapters --test e2e::neo4j_e2e_test -- --nocapture
```

Alternatively, create a `.env` file in the crate root with these settings.

The `--nocapture` flag ensures that test output is displayed, showing the logs from the test execution.

## Test Details

### Full Workflow Test

The `full_workflow.rs` test exercises the full workflow of the KB using in-memory implementations.

### Neo4j E2E Test

The `neo4j_e2e_test.rs` test connects to a real Neo4j database and tests:

1. Ingesting a component definition
2. Verifying the component was stored correctly
3. Ingesting a flow definition that references the component
4. Verifying the flow was stored correctly
5. Testing relationships between flows and components

This test uses unique IDs for each run to avoid conflicts between test runs.

## Troubleshooting

If the Neo4j tests fail with connection errors:

1. Verify the Neo4j Aura instance is operational by checking the Aura console
2. Check that your credentials are correct
3. Ensure your firewall allows outbound connections to Neo4j Aura
4. Try running with `RUST_LOG=debug` to see more detailed logs

### Connection Issues

Common connection issues:

1. **Authentication failure**: Double check the username and password
2. **Timeout**: The Neo4j Aura instance might be unresponsive or there's a network issue
3. **Database not found**: Verify the database name is correct
4. **No route to host**: Your network might be blocking outbound connections to the Neo4j Aura instance 
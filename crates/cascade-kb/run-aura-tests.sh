#!/bin/bash
set -e

# Change to the directory containing this script
cd "$(dirname "$0")"

echo "Running tests with Neo4j Aura connection"
echo "Connection details:"
echo "  URI: neo4j+s://dd132cc4.databases.neo4j.io"
echo "  Database: neo4j"
echo "  Username: neo4j"
echo "  Password: ********"

# Set environment variables for the tests
export NEO4J_URI="neo4j+s://dd132cc4.databases.neo4j.io"
export NEO4J_USERNAME="neo4j"
export NEO4J_PASSWORD="77iJ56VLGXj9jIGO5e8EZUFV2mniD9IuquuwaD0TNp0"
export NEO4J_DATABASE="neo4j"
export NEO4J_POOL_SIZE=10

# Enable detailed logging for troubleshooting
export RUST_LOG=cascade_kb=debug,neo4j_connection_test=debug,embedding_neo4j_test=debug,neo4j_stdlib_components=debug

# Run the connection test first to verify connectivity
echo "Running Neo4j connection test..."
cargo test -p cascade-kb --features adapters --test neo4j_connection_test -- --nocapture

# Capture the exit status
CONNECTION_TEST_STATUS=$?

if [ $CONNECTION_TEST_STATUS -ne 0 ]; then
  echo "Neo4j connection test failed with status: $CONNECTION_TEST_STATUS"
  echo "Please check your internet connection and the Neo4j Aura instance status."
  exit $CONNECTION_TEST_STATUS
fi

# Run the embedding test if applicable
# Note: This may be skipped as Neo4j Aura Free may not have the GenAI plugin
echo "Running Neo4j embedding test..."
cargo test -p cascade-kb --features adapters --test embedding_neo4j_test -- --nocapture

# Run the stdlib components test
echo "Running Neo4j StdLib components test..."
cargo test -p cascade-kb --features adapters --test neo4j_stdlib_components -- --nocapture

# Capture the final exit status
TEST_STATUS=$?

echo "Neo4j Aura test run completed with status: $TEST_STATUS"
exit $TEST_STATUS 
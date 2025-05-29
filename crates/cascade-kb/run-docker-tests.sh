#!/bin/bash
set -e

# Colors for better output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Change to the directory containing this script
cd "$(dirname "$0")"

# Neo4j container configuration
NEO4J_CONTAINER="cdas-kb-test-neo4j"
NEO4J_HTTP_PORT="17474"
NEO4J_BOLT_PORT="17687"
NEO4J_URI="neo4j://localhost:${NEO4J_BOLT_PORT}"
NEO4J_USERNAME="neo4j"
NEO4J_PASSWORD="password"
NEO4J_DATABASE="neo4j"

# Check if container is running
echo -e "${BLUE}Checking for Neo4j container...${NC}"
if docker ps | grep -q ${NEO4J_CONTAINER}; then
  echo -e "${GREEN}Found existing Neo4j container. Using it.${NC}"
else
  echo -e "${YELLOW}Neo4j container not found. Attempting to start it...${NC}"
  if docker-compose up -d neo4j; then
    echo -e "${GREEN}Neo4j container started successfully.${NC}"
    # Wait for Neo4j to be ready
    echo -e "${BLUE}Waiting for Neo4j to start...${NC}"
    MAX_RETRIES=60
    RETRY_COUNT=0
    while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
      if docker logs ${NEO4J_CONTAINER} 2>&1 | grep -q "Remote interface available at"; then
        echo -e "${GREEN}Neo4j started successfully!${NC}"
        # Wait a bit more to ensure it's fully initialized
        echo -e "${YELLOW}Waiting 5 more seconds for Neo4j to fully initialize...${NC}"
        sleep 5
        break
      fi
      
      if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
        echo -e "${RED}Timed out waiting for Neo4j to start. Check logs with: docker logs ${NEO4J_CONTAINER}${NC}"
        exit 1
      fi
      
      echo -n "."
      RETRY_COUNT=$((RETRY_COUNT+1))
      sleep 1
    done
  else
    echo -e "${RED}Failed to start Neo4j container. Please run ./setup-docker-env.sh first.${NC}"
    exit 1
  fi
fi

# Check if Neo4j is responsive
echo -e "${BLUE}Checking if Neo4j is responsive...${NC}"
if ! curl -s -o /dev/null -u neo4j:password http://localhost:${NEO4J_HTTP_PORT}; then
  echo -e "${RED}Neo4j is not responsive. Container is running but service may not be ready.${NC}"
  echo -e "${YELLOW}Waiting 10 seconds for Neo4j to become responsive...${NC}"
  sleep 10
  if ! curl -s -o /dev/null -u neo4j:password http://localhost:${NEO4J_HTTP_PORT}; then
    echo -e "${RED}Neo4j is still not responsive. Please check container logs.${NC}"
    docker logs ${NEO4J_CONTAINER} | tail -n 20
    exit 1
  fi
fi
echo -e "${GREEN}Neo4j is responsive.${NC}"

# Ensure vector indexes exist
echo -e "${BLUE}Ensuring vector indexes are created...${NC}"
docker exec $NEO4J_CONTAINER bash -c '
  # Create vector index for DocumentationChunk
  /var/lib/neo4j/bin/cypher-shell -u neo4j -p password "
    CREATE VECTOR INDEX docEmbeddings IF NOT EXISTS 
    FOR (d:DocumentationChunk) 
    ON d.embedding 
    OPTIONS {indexConfig: {
      \`vector.dimensions\`: 1536,
      \`vector.similarity_function\`: \"cosine\"
    }}
  "

  # Create vector index for TestEmbedding (useful for tests)
  /var/lib/neo4j/bin/cypher-shell -u neo4j -p password "
    CREATE VECTOR INDEX test_embedding_idx IF NOT EXISTS 
    FOR (n:TestEmbedding) 
    ON n.embedding 
    OPTIONS {indexConfig: {
      \`vector.dimensions\`: 1536,
      \`vector.similarity_function\`: \"cosine\"
    }}
  "

  # Create vector index for Content
  /var/lib/neo4j/bin/cypher-shell -u neo4j -p password "
    CREATE VECTOR INDEX content_embedding_idx IF NOT EXISTS 
    FOR (n:Content) 
    ON n.embedding 
    OPTIONS {indexConfig: {
      \`vector.dimensions\`: 1536,
      \`vector.similarity_function\`: \"cosine\"
    }}
  "
'

# Add some test data with fixed-size embeddings
echo -e "${BLUE}Adding test data for vector search...${NC}"
docker exec $NEO4J_CONTAINER bash -c '
  /var/lib/neo4j/bin/cypher-shell -u neo4j -p password "
    MERGE (d:DocumentationChunk {id: \"test-doc-1\"})
    SET d.tenant_id = \"test-tenant\",
        d.text = \"Use conditional logic to route messages based on content and context.\",
        d.scope = \"General\",
        d.embedding = [0.1, 0.2, 0.3, 0.4, 0.5, 0.1, 0.2, 0.3, 0.4, 0.5, 0.1, 0.2, 0.3, 0.4, 0.5, 0.1, 0.2, 0.3, 0.4, 0.5],
        d.chunk_seq = 1,
        d.created_at = datetime()
  "
'

echo -e "${GREEN}Vector index and test data creation completed.${NC}"

# Check for Neo4j vector capabilities
echo -e "${BLUE}Checking Neo4j vector capabilities...${NC}"
if docker exec $NEO4J_CONTAINER bash -c '/var/lib/neo4j/bin/cypher-shell -u neo4j -p password "SHOW INDEXES YIELD type WHERE type = \"VECTOR\" RETURN count(*) as count"' | grep -q "0 records"; then
  echo -e "${YELLOW}Warning: No vector indexes found. Vector searches will use fallback mechanism.${NC}"
  echo -e "${YELLOW}Setting SKIP_VECTOR_TESTS=true for safer test execution.${NC}"
  export SKIP_VECTOR_TESTS=true
else
  echo -e "${GREEN}Vector indexes are available.${NC}"
  # Only set USE_MOCK_EMBEDDING if not already set
  if [ -z "$USE_MOCK_EMBEDDING" ]; then
    export USE_MOCK_EMBEDDING=true
  fi
fi

# Set environment variables for tests
export NEO4J_URI=$NEO4J_URI
export NEO4J_USERNAME=$NEO4J_USERNAME
export NEO4J_PASSWORD=$NEO4J_PASSWORD
export NEO4J_DATABASE=$NEO4J_DATABASE
export NEO4J_POOL_SIZE=10
export USE_REAL_NEO4J=true

# For simplicity in tests, we'll always use mock embedding if not set
if [ -z "$USE_MOCK_EMBEDDING" ]; then
  export USE_MOCK_EMBEDDING=true
fi

# Use predictable embeddings to ensure consistent vector search results
export USE_PREDICTABLE_EMBEDDING=true

# Show test configuration
echo -e "${BLUE}Test configuration:${NC}"
echo "  NEO4J_URI = $NEO4J_URI"
echo "  NEO4J_USERNAME = $NEO4J_USERNAME"
echo "  NEO4J_DATABASE = $NEO4J_DATABASE"
echo "  NEO4J_POOL_SIZE = $NEO4J_POOL_SIZE"
echo "  USE_REAL_NEO4J = $USE_REAL_NEO4J"
echo "  USE_MOCK_EMBEDDING = $USE_MOCK_EMBEDDING"
echo "  USE_PREDICTABLE_EMBEDDING = $USE_PREDICTABLE_EMBEDDING"
echo "  SKIP_VECTOR_TESTS = ${SKIP_VECTOR_TESTS:-false}"

# Run Neo4j connection test first
echo -e "${BLUE}Running Neo4j connection test...${NC}"
cargo test --package cascade-kb --test neo4j_connection_test || {
  echo -e "${RED}Neo4j connection test failed. Please check the error message above.${NC}"
  exit 1
}

# Run vector search test to verify vector capabilities
echo -e "${BLUE}Running vector search test...${NC}"
cargo test --package cascade-kb --test vector_search_test || {
  echo -e "${YELLOW}Vector search test had issues. Tests will continue but vector search may not work properly.${NC}"
  # Don't exit, let other tests run
}

# Run all tests
echo -e "${BLUE}Running all integration tests...${NC}"
cargo test --package cascade-kb -- --nocapture || {
  echo -e "${RED}Some tests failed. Please review the output above.${NC}"
  exit 1
}

echo -e "${GREEN}All tests completed successfully!${NC}" 
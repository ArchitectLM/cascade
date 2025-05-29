#!/bin/bash
set -e

# Colors for terminal output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Change to the directory containing this script
cd "$(dirname "$0")"

# Create plugins directory if it doesn't exist
mkdir -p plugins

# Download required Neo4j plugins for version 5.18.0
echo -e "${BLUE}Downloading required Neo4j plugins...${NC}"
if [ ! -f "plugins/apoc-5.18.0-core.jar" ]; then
  echo -e "${YELLOW}Downloading APOC Core plugin...${NC}"
  curl -L -o plugins/apoc-5.18.0-core.jar https://github.com/neo4j-contrib/neo4j-apoc-procedures/releases/download/5.18.0/apoc-5.18.0-core.jar
fi

if [ ! -f "plugins/apoc-5.18.0-extended.jar" ]; then
  echo -e "${YELLOW}Downloading APOC Extended plugin...${NC}"
  curl -L -o plugins/apoc-5.18.0-extended.jar https://github.com/neo4j-contrib/neo4j-apoc-procedures/releases/download/5.18.0/apoc-5.18.0-extended.jar
fi

# Stop any existing containers and clean up volumes
echo -e "${BLUE}Cleaning up any existing containers and volumes...${NC}"
docker-compose down -v

# Start the Neo4j container
echo -e "${BLUE}Starting Neo4j container...${NC}"
docker-compose up -d neo4j

# Wait for Neo4j to start and become healthy
echo -e "${BLUE}Waiting for Neo4j to become healthy...${NC}"
attempt=1
max_attempts=30
while [ $attempt -le $max_attempts ]; do
  if docker ps | grep cdas-kb-test-neo4j | grep -q "(healthy)"; then
    echo -e "${GREEN}Neo4j is now healthy!${NC}"
    break
  fi
  
  # If not healthy, check if container is still running
  if ! docker ps | grep -q cdas-kb-test-neo4j; then
    echo -e "${RED}Neo4j container has stopped. Checking logs:${NC}"
    docker logs cdas-kb-test-neo4j
    exit 1
  fi
  
  echo -n "."
  sleep 5
  attempt=$((attempt+1))
  
  if [ $attempt -eq $max_attempts ]; then
    echo -e "${RED}Timed out waiting for Neo4j to become healthy.${NC}"
    exit 1
  fi
done

# Verify Neo4j connection
echo -e "${BLUE}Verifying Neo4j connection...${NC}"
if ! curl -s -u neo4j:password http://localhost:17474 > /dev/null; then
  echo -e "${RED}Unable to connect to Neo4j web interface. The container might be running but the service is not responding.${NC}"
  exit 1
fi

# Load seed data
echo -e "${BLUE}Loading seed data...${NC}"
./load_seed_data.sh

# Create vector indexes for testing
echo -e "${BLUE}Setting up vector indexes...${NC}"
docker exec cdas-kb-test-neo4j cypher-shell -u neo4j -p password "
CREATE VECTOR INDEX test_embedding_index IF NOT EXISTS 
FOR (n:TestEmbedding) 
ON (n.embedding)
OPTIONS {indexConfig: {
  \`vector.dimensions\`: 1536,
  \`vector.similarity_function\`: 'cosine'
}}
"

docker exec cdas-kb-test-neo4j cypher-shell -u neo4j -p password "
CREATE VECTOR INDEX content_embedding_index IF NOT EXISTS 
FOR (n:Content) 
ON (n.embedding)
OPTIONS {indexConfig: {
  \`vector.dimensions\`: 1536,
  \`vector.similarity_function\`: 'cosine'
}}
"

# Initialize database schema
echo -e "${BLUE}Initializing database schema...${NC}"
if [ -f "data/init-docker-db.sh" ]; then
  ./data/init-docker-db.sh
else
  echo -e "${YELLOW}Database initialization script not found, skipping schema initialization.${NC}"
fi

# Run unit tests first
echo -e "${BLUE}Running unit tests...${NC}"
RUST_BACKTRACE=1 cargo test -p cascade-kb --lib --features mocks

# Set the environment variables for integration tests
echo -e "${BLUE}Running integration tests...${NC}"
RUST_BACKTRACE=1 \
USE_MOCK_EMBEDDING=true \
NEO4J_URI=neo4j://localhost:17687 \
NEO4J_USERNAME=neo4j \
NEO4J_PASSWORD=password \
NEO4J_DATABASE=neo4j \
NEO4J_POOL_SIZE=10 \
cargo test --package cascade-kb --features="integration-tests,adapters" -- --nocapture

echo -e "${GREEN}All tests completed!${NC}" 
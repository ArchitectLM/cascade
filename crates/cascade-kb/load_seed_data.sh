#!/bin/bash
set -e

# Set up colors for terminal output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Loading Neo4j seed data...${NC}"

# Find the Neo4j container name
NEO4J_CONTAINER=$(docker ps | grep neo4j | awk '{print $NF}')
if [ -z "$NEO4J_CONTAINER" ]; then
  echo -e "${RED}No Neo4j container found running. Please start it with 'docker-compose up -d neo4j' first.${NC}"
  exit 1
fi

echo -e "${YELLOW}Using Neo4j container: ${NEO4J_CONTAINER}${NC}"

# Path to seed data
SEED_FILE="data/clean_seed.cypher"

if [ ! -f "$SEED_FILE" ]; then
  echo -e "${RED}Clean seed file not found: ${SEED_FILE}${NC}"
  exit 1
fi

# Upload seed files to container
echo -e "${YELLOW}Copying seed file to container...${NC}"
docker cp "$SEED_FILE" ${NEO4J_CONTAINER}:/var/lib/neo4j/clean_seed.cypher

# Execute Cypher script
echo -e "${YELLOW}Executing seed data...${NC}"
docker exec ${NEO4J_CONTAINER} cypher-shell -u neo4j -p password --file /var/lib/neo4j/clean_seed.cypher

echo -e "${GREEN}Seed data loaded successfully!${NC}" 
#!/bin/bash
set -e

# Change to the directory containing this script
cd "$(dirname "$0")"

# Define colors for better output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

usage() {
  echo -e "${YELLOW}Cascade KB Test Runner${NC}"
  echo ""
  echo "Usage: $(basename $0) [options]"
  echo ""
  echo "Options:"
  echo "  -u, --unit-tests        Run unit tests only (mocked, no external dependencies)"
  echo "  -i, --integration-tests Run integration tests only (requires Docker)"
  echo "  -a, --all               Run all tests (unit tests first, then integration tests)"
  echo "  -d, --docker-check      Check if Docker is running and Neo4j container is available"
  echo "  -s, --setup             Setup Docker environment for testing"
  echo "  -h, --help              Show this help message"
  echo ""
}

run_unit_tests() {
  echo -e "${YELLOW}Running unit tests (no external dependencies)...${NC}"
  RUST_BACKTRACE=1 cargo test -p cascade-kb --lib --features mocks
  echo -e "${GREEN}Unit tests completed successfully!${NC}"
}

check_docker() {
  echo -e "${YELLOW}Checking Docker status...${NC}"
  
  # Check if Docker is running
  if ! docker info > /dev/null 2>&1; then
    echo -e "${RED}Docker is not running. Please start Docker first.${NC}"
    return 1
  fi
  
  # Check for Neo4j container
  NEO4J_CONTAINER=$(docker ps | grep neo4j | awk '{print $NF}')
  if [ -z "$NEO4J_CONTAINER" ]; then
    echo -e "${RED}No Neo4j container found. Please start the Neo4j container first.${NC}"
    echo -e "${YELLOW}You can start it with:${NC}"
    echo -e "    ./setup-docker-env.sh"
    return 1
  fi
  
  echo -e "${GREEN}Docker is running and Neo4j container is available: ${NEO4J_CONTAINER}${NC}"
  
  # Check if Neo4j is responsive
  NEO4J_HTTP_PORT="17474"
  if ! curl -s -o /dev/null -u neo4j:password http://localhost:${NEO4J_HTTP_PORT}; then
    echo -e "${RED}Neo4j is not responsive. The container is running but the service may not be ready.${NC}"
    return 1
  fi
  
  echo -e "${GREEN}Neo4j is responsive and ready for tests!${NC}"
  return 0
}

setup_docker() {
  echo -e "${YELLOW}Setting up Docker environment for testing...${NC}"
  ./setup-docker-env.sh
  echo -e "${GREEN}Docker environment setup complete!${NC}"
}

run_integration_tests() {
  echo -e "${YELLOW}Running integration tests (using Docker Neo4j)...${NC}"
  
  # Check Docker status
  if ! check_docker; then
    echo -e "${YELLOW}Attempting to set up Docker environment...${NC}"
    setup_docker
    
    # Check again after setup
    if ! check_docker; then
      echo -e "${RED}Integration tests cannot run without Docker Neo4j.${NC}"
      return 1
    fi
  fi
  
  # Run integration tests with proper environment
  ./run-docker-tests.sh
  
  echo -e "${GREEN}Integration tests completed successfully!${NC}"
}

run_all_tests() {
  run_unit_tests
  run_integration_tests
}

# Process command line arguments
case "$1" in
  -u|--unit-tests)
    run_unit_tests
    ;;
  -i|--integration-tests)
    run_integration_tests
    ;;
  -a|--all)
    run_all_tests
    ;;
  -d|--docker-check)
    check_docker
    ;;
  -s|--setup)
    setup_docker
    ;;
  -h|--help|*)
    usage
    ;;
esac

exit 0 
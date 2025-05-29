#!/bin/bash
# WebSocket Integration Test Runner
# This script provides a convenient way to run the WebSocket tests

# Default values
WEBSOCKET_URL="ws://localhost:8080/api/v1/ws"
TOKEN="demo.test_user.test_tenant"
TEST_TYPES="basic,error,performance"
VERBOSE=false
TIMEOUT=10

# Colors for better output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print header
echo -e "${BLUE}=========================================================${NC}"
echo -e "${BLUE}        Cascade WebSocket Integration Test Runner        ${NC}"
echo -e "${BLUE}=========================================================${NC}"

# Function to display usage information
function show_help {
    echo -e "${YELLOW}Usage:${NC} $0 [options]"
    echo ""
    echo "Options:"
    echo "  -u, --url URL       WebSocket server URL (default: $WEBSOCKET_URL)"
    echo "  -t, --token TOKEN   Authentication token (default: $TOKEN)"
    echo "  -T, --test-types    Types of tests to run (default: $TEST_TYPES)"
    echo "  -v, --verbose       Enable verbose output"
    echo "  -o, --timeout SEC   Set operation timeout in seconds (default: $TIMEOUT)"
    echo "  -h, --help          Show this help message"
    echo ""
    echo "Test Types:"
    echo "  basic               Basic functionality tests (connection, auth, messaging)"
    echo "  error               Error handling tests"
    echo "  performance         Performance and load tests"
    echo ""
    echo "Examples:"
    echo "  $0                                   # Run all tests with defaults"
    echo "  $0 --test-types basic                # Run only basic tests"
    echo "  $0 -u ws://dev:8090/ws -v            # Custom URL with verbose output"
    echo "  $0 -T basic,error -o 20              # Run basic and error tests with longer timeout"
    echo -e "${BLUE}=========================================================${NC}"
}

# Check for Python and dependencies
function check_dependencies {
    echo -e "${BLUE}Checking dependencies...${NC}"
    
    # Check Python
    if ! command -v python3 &> /dev/null; then
        echo -e "${RED}Error: Python 3 is required but not installed.${NC}"
        exit 1
    fi
    
    # Check websockets package
    if ! python3 -c "import websockets" &> /dev/null; then
        echo -e "${YELLOW}Warning: Python 'websockets' package not found. Installing...${NC}"
        python3 -m pip install websockets
        
        # Check if install succeeded
        if ! python3 -c "import websockets" &> /dev/null; then
            echo -e "${RED}Error: Failed to install 'websockets' package.${NC}"
            echo "Try installing manually with: pip install websockets"
            exit 1
        fi
    fi
    
    echo -e "${GREEN}All dependencies satisfied.${NC}"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        -u|--url)
            WEBSOCKET_URL="$2"
            shift 2
            ;;
        -t|--token)
            TOKEN="$2"
            shift 2
            ;;
        -T|--test-types)
            TEST_TYPES="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -o|--timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            show_help
            exit 1
            ;;
    esac
done

# Check dependencies
check_dependencies

# Get absolute path to the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Move to script directory
cd "$SCRIPT_DIR"

# Display run information
echo -e "${BLUE}Test configuration:${NC}"
echo -e "  Server URL: ${YELLOW}$WEBSOCKET_URL${NC}"
echo -e "  Test Types: ${YELLOW}$TEST_TYPES${NC}"
echo -e "  Timeout:    ${YELLOW}$TIMEOUT${NC} seconds"
echo -e "  Verbose:    ${YELLOW}$VERBOSE${NC}"
echo ""

# Build command
CMD="python3 websocket_integration_test.py --url \"$WEBSOCKET_URL\" --token \"$TOKEN\" --test-types \"$TEST_TYPES\""

# Add verbose flag if needed
if [ "$VERBOSE" = true ]; then
    CMD="$CMD --verbose"
fi

echo -e "${BLUE}Running tests...${NC}"
echo -e "Command: ${YELLOW}$CMD${NC}"
echo ""

# Run the command
eval "$CMD"
EXIT_CODE=$?

# Report result
if [ $EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}All tests passed successfully!${NC}"
else
    echo -e "${RED}Some tests failed. Check the log for details.${NC}"
fi

exit $EXIT_CODE 
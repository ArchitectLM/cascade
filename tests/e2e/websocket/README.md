# Cascade WebSocket Testing Tools

This directory contains tools for testing the WebSocket interface of the Cascade agent application. These tools provide comprehensive testing capabilities for all aspects of the WebSocket API, including authentication, chat functionality, streaming responses, and error handling.

## Tools Overview

1. **WebSocket Client** (`websocket_client.py`): A flexible client for connecting to and interacting with the Cascade WebSocket API.
2. **Integration Tests** (`websocket_integration_test.py`): A test suite that verifies the functionality, robustness, and performance of the WebSocket interface.

## WebSocket Client Features

The WebSocket client supports the following functionality:

- **Authentication**: Connect and authenticate with JWT tokens
- **Chat**: Send chat messages with streaming or non-streaming responses
- **DSL Operations**: Analyze DSL code, explain DSL elements, and trace data flows
- **Feedback**: Submit feedback on agent responses
- **Ping/Pong**: Test connection health with ping/pong messages
- **Error Handling**: Robust error handling and timeout management

## Integration Test Features

The integration test suite provides:

- **Basic Functionality Tests**: Validates core functionality like connections, authentication, messaging
- **Error Handling Tests**: Verifies proper handling of error conditions and edge cases
- **Performance Tests**: Measures throughput, response times, and scaling behavior

## Prerequisites

- Python 3.7+
- `websockets` package

Install dependencies with:

```bash
pip install websockets
```

## Usage Examples

### WebSocket Client

The WebSocket client can be used in two ways:
1. As a command-line tool for manual testing
2. As a library imported into other Python scripts

#### Command-line Usage:

```bash
# Simple chat message
python websocket_client.py chat "Hello, world"

# Streaming chat
python websocket_client.py chat "Generate a paragraph about WebSockets" --stream

# Analyze DSL code (from a file)
python websocket_client.py analyze "$(cat flow.dsl)"

# Ping test
python websocket_client.py ping

# Custom server URL and token
python websocket_client.py --url ws://dev-server:8090/api/v1/ws --token "my.jwt.token" chat "Hello"
```

#### Library Usage:

```python
import asyncio
from websocket_client import CascadeWebSocketClient

async def test_function():
    client = CascadeWebSocketClient("ws://localhost:8080/api/v1/ws", "your.jwt.token")
    await client.connect()
    await client.authenticate()
    
    # Send a chat message
    response = await client.chat("Hello from Python")
    print(response)
    
    await client.close()

# Run the async function
asyncio.run(test_function())
```

### Integration Tests

Run the entire test suite:

```bash
python websocket_integration_test.py
```

Run specific test types:

```bash
# Only run basic functionality tests
python websocket_integration_test.py --test-types basic

# Run error handling and performance tests
python websocket_integration_test.py --test-types error,performance

# Custom server and verbose output
python websocket_integration_test.py --url ws://dev-server:8090/api/v1/ws --token "my.jwt.token" --verbose
```

## CI/CD Integration

These tools can be integrated into CI/CD pipelines to ensure WebSocket functionality remains stable.

### Example GitHub Action:

```yaml
name: WebSocket API Tests

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  websocket-tests:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v2
    
    - name: Set up Python
      uses: actions/setup-python@v2
      with:
        python-version: '3.9'
    
    - name: Install dependencies
      run: |
        python -m pip install --upgrade pip
        pip install websockets
    
    - name: Start Cascade server
      run: |
        # Commands to start the Cascade server for testing
        cd /path/to/cascade
        cargo run --bin cascade-agent -- --config test_config.json &
        sleep 10  # Wait for server to start
    
    - name: Run WebSocket tests
      run: |
        cd tests/e2e/websocket
        python websocket_integration_test.py --url ws://localhost:8080/api/v1/ws
    
    - name: Upload test logs
      if: always()
      uses: actions/upload-artifact@v2
      with:
        name: websocket-test-logs
        path: tests/e2e/websocket/websocket_test_results_*.log
```

### Jenkins Pipeline Example:

```groovy
pipeline {
    agent any
    
    stages {
        stage('Setup') {
            steps {
                sh 'python -m pip install websockets'
            }
        }
        
        stage('Start Test Server') {
            steps {
                sh 'cargo run --bin cascade-agent -- --config test_config.json &'
                sh 'sleep 10'  // Wait for server to start
            }
        }
        
        stage('WebSocket Tests') {
            steps {
                sh 'cd tests/e2e/websocket && python websocket_integration_test.py'
            }
        }
    }
    
    post {
        always {
            archiveArtifacts artifacts: 'tests/e2e/websocket/websocket_test_results_*.log', allowEmptyArchive: true
        }
    }
}
```

## Test Output and Reporting

The integration tests generate detailed logs that include:

- Pass/fail status for each test
- Timing information for performance tracking
- Detailed error messages for failed tests
- Summary statistics for the entire test run

Example output:

```
============================================================
TEST RESULTS: 18/20 passed (90.0%)
  ✅ Passed: 18
  ❌ Failed: 2
  ⚠️ Skipped: 0
------------------------------------------------------------
Failed tests:
  - test_invalid_message_type: Expected error response, got None
  - test_streaming_performance: Should receive multiple chunks for a long response
------------------------------------------------------------
Test timings:
  - test_chat_streaming: 12.34s
  - test_streaming_performance: 10.21s
  - test_conversation_continuity: 8.76s
  - test_rapid_messages: 6.54s
  - test_chat_basic: 3.21s
============================================================
```

## Troubleshooting

If you encounter issues when running the tests:

1. **Connection Errors**: Verify the server URL is correct and the server is running
2. **Authentication Failures**: Ensure the JWT token is valid and not expired
3. **Timeout Errors**: Increase the client timeout with `--timeout` parameter
4. **Import Errors**: Make sure you're running the scripts from the correct directory

## Extending the Tests

To add new test cases:

1. For basic API tests, add a new test method to one of the existing test classes
2. For new functionality, create a new test class that inherits from `TestSuite`
3. To test new message types, extend the WebSocket client with new methods

## Maintenance

To keep these tools up to date:

1. Update the client when new WebSocket message types are added
2. Add tests for new features as they are implemented
3. Review and update timeouts and performance thresholds periodically 
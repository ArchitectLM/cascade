# Cascade Platform Tests

This directory contains the test suite for the Cascade Platform, organized into different types of tests.

## Test Organization

The test suite is organized as follows:

1. **E2E Tests** (`e2e/`): End-to-end tests that verify the complete system behavior
2. **Integration Tests** (`integrations/`): Tests that verify the integration between different components
3. **BDD Tests** (`bdd/`): Behavior-driven tests written in Gherkin syntax

## Running Tests

### Running All Tests

To run all tests:

```bash
cd tests
cargo test
```

### Running Specific Test Types

To run only a specific type of test:

```bash
# Run E2E tests
cargo test --package cascade-e2e-tests

# Run Integration tests
cargo test --package cascade-integration-tests

# Run BDD tests
cargo test --test bdd --package cascade-bdd-tests
```

### Running Individual Tests

To run a specific test:

```bash
# Run a specific E2E test
cargo test --package cascade-e2e-tests --test http_flow_e2e

# Run a specific integration test
cargo test --package cascade-integration-tests --test core_example

# Run a specific BDD feature
cargo run --package cascade-bdd-tests --bin basic-order-test
```

## Test Structure

### E2E Tests

E2E tests are designed to test the system from an external perspective, typically through the HTTP API. These tests:

- Deploy flows
- Trigger flows
- Verify responses and side effects

### Integration Tests

Integration tests verify that different parts of the system work together correctly. These tests:

- Test component interactions
- Validate data flows
- Check error handling between subsystems

### BDD Tests

BDD tests are written in Gherkin syntax and test the system from a business requirements perspective. These tests:

- Define business scenarios
- Test acceptance criteria
- Validate business rules

## Mock Reduction Strategy

We're now following a "real components first" approach that strategically reduces mocks to improve test reliability and maintainability. Key principles include:

- Using real implementations whenever practical
- Only mocking external dependencies
- Using appropriate test doubles (stubs, mocks, fakes) for different situations
- Prioritizing real implementations in integration and E2E tests

For detailed guidance, see the [Mock Reduction Guide](MOCK-REDUCTION-GUIDE.md).

## Adding New Tests

When adding new tests, follow these guidelines:

1. Place the test in the appropriate directory based on its type
2. Follow the existing naming conventions
3. Ensure the test can run independently
4. Add any necessary documentation
5. Follow the mock reduction guidelines
6. Prefer real implementations over mocks where appropriate

## Documentation & Plans

- [Test Improvement Plan](TEST-IMPROVEMENT-PLAN.md): Details on planned improvements to the test suite
- [Mock Reduction Guide](MOCK-REDUCTION-GUIDE.md): Strategic approach to reducing unnecessary mocks 
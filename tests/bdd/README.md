# Cascade BDD Tests

This directory contains Behavior-Driven Development (BDD) tests for the Cascade platform.

## Quick Start

Run all BDD tests with a single command:

```bash
cargo test --test bdd
```

For more detailed logging:

```bash
RUST_LOG=debug cargo test --test bdd
```

## Structure

- `features/` - Gherkin feature files
- `steps/` - Step definition implementations for the main CascadeWorld
- `bin/` - Test runners and utilities
- `main.rs` - Main test runner that automatically runs all feature files

## Running Tests

We've simplified the testing approach to use a single test runner that automatically scans the `features/` directory and runs all feature files it finds. This means:

1. No need to register new feature files manually
2. All tests use the same `CascadeWorld` and step definitions
3. Test results are immediately visible in the console

See the [bin/README.md](bin/README.md) for more details.

## Guidelines

1. **Avoid Tables**: Use simple parameters instead of tables
2. **Default Trait**: World must have a proper Default implementation
3. **No Context**: Use explicit parameters in step definitions
4. **Async Steps**: Mark async steps with the async keyword
5. **Mock Network Calls**: Use mock implementations for API calls during testing
6. **Response Handling**: Never store Response objects directly; extract data before consumption

## Best Practices

1. Extract data from Response objects immediately (status, headers)
2. Only then consume the Response with methods like `json()`
3. Store extracted data in the World object
4. Use mocking to avoid actual API calls
5. Prefer explicit parameter types over Context
6. Use descriptive step names that match the feature files

## Next Steps

1. ✅ Fixed table-based step definitions
2. ✅ Updated main test runner to support feature filtering
3. ✅ Improved handling of Response objects
4. ✅ Simplified test runner approach for easier test execution
5. Add more comprehensive feature files for broader test coverage
6. Integrate BDD tests with CI/CD pipeline
7. Implement more detailed logging for debugging test failures

## Alternative Test Pattern

We've created several new working test patterns that use a different approach:

- **Working Minimal Test**: `cargo run --bin working-minimal-test` - A minimal test with inline step definitions
- **Working Simple Order Test**: `cargo run --bin working-simple-order-test` - A test for creating and validating orders 
- **Working Circuit Breaker Test**: `cargo run --bin working-circuit-breaker-test` - A test for the circuit breaker pattern
- **Working Standard Order Test**: `cargo run --bin working-standard-order-test` - A comprehensive order processing test

These tests define step definitions in the same file as the test runner, which avoids step discovery issues.

## Additional Guidelines

For creating more reliable tests:

1. **Define steps in the same file**: Include step definitions in the same file as your test runner
2. **Create dedicated World types**: Each test should have its own World implementation
3. **Self-contained features**: Generate feature files programmatically within the tests
4. **Use simple mocks**: Don't rely on external services for BDD tests 
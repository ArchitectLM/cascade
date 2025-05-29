# Cascade BDD Test Runners

This directory contains the BDD test runners for the Cascade project.

## Simplified Testing Approach

We've consolidated our BDD testing approach to use a single test runner in `main.rs` that directly runs all feature files found in the `features/` directory. This provides several advantages:

1. **Simpler Execution**: Tests can be run with a single command: `cargo test --test bdd`
2. **Consistent Environment**: All tests run in the same environment and share step definitions
3. **Automatic Discovery**: No need to register new feature files - they're found automatically
4. **Direct Output**: Test results are immediately visible in the console

## Running the Tests

```bash
# Run all BDD tests
cargo test --test bdd

# Run with logging for more details
RUST_LOG=debug cargo test --test bdd
```

## BDD Test Structure

- `features/` - Gherkin feature files
- `steps/` - Step definition implementations
- `main.rs` - Main test runner that loads all feature files
- `run_all_tests.sh` - Shell script to run all BDD tests

## Legacy Files

The `unified_bdd_runner.rs` file contains implementations of various test worlds that were previously used. We've kept this file for reference but the main test runner now uses `CascadeWorld` from `steps/world.rs` for all feature files.

## Adding New Tests

To add a new test:

1. Create a new `.feature` file in the `features/` directory
2. Add step definitions in the `steps/` directory if needed
3. Run all tests with `cargo test --test bdd`

No need to update any registration logic or bin targets - new feature files are automatically discovered. 
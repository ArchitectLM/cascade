# Test-Driven Development in Cascade

This guide outlines the Test-Driven Development (TDD) approach used in the Cascade project.

## Introduction to TDD

Test-Driven Development is a software development process that relies on the repetition of a very short development cycle:

1. Write a failing test that defines a desired improvement or new function
2. Run the test to verify it fails
3. Write the minimum amount of code necessary to make the test pass
4. Run the test to verify it passes
5. Refactor the code
6. Repeat

This approach ensures that all code is covered by tests and helps developers focus on implementing only what is necessary to satisfy requirements.

## TDD in Cascade

In the Cascade project, we use multiple levels of testing following TDD principles:

### Unit Tests

Unit tests focus on testing individual components or functions in isolation. These tests are typically located alongside the code they're testing:

```
crates/cascade-core/src/component.rs
crates/cascade-core/src/component_test.rs
```

When implementing a new feature, start by writing unit tests that verify the expected behavior of individual components.

### Integration Tests

Integration tests verify that different components work together correctly. These tests are located in the `tests/integrations` directory, organized by feature area:

```
tests/integrations/core_example.rs
tests/integrations/edge_resilience_example.rs
tests/integrations/monitoring_example.rs
```

After unit tests pass, write integration tests that verify the feature works correctly when integrated with other components.

### BDD Tests

BDD tests describe the behavior of the system from an end-user perspective. These tests are located in the `tests/bdd` directory:

```
tests/bdd/features/order_processing.feature
tests/bdd/features/circuit_breaker.feature
```

After integration tests pass, write BDD tests that verify the feature meets the business requirements.

## How to Apply TDD in Cascade

### Step 1: Understand the Requirement

Before writing any code, understand the requirement thoroughly. This might involve discussing with stakeholders, reviewing documentation, or clarifying ambiguities.

### Step 2: Write a Failing Test

Write a failing test that describes the expected behavior:

```rust
#[test]
fn test_circuit_breaker_opens_after_failures() {
    let circuit_breaker = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_ms: 500,
        half_open_max_calls: 1,
    });

    // First three calls fail
    assert_eq!(circuit_breaker.get_state(), CircuitState::Closed);
    
    circuit_breaker.record_failure();
    assert_eq!(circuit_breaker.get_state(), CircuitState::Closed);
    
    circuit_breaker.record_failure();
    assert_eq!(circuit_breaker.get_state(), CircuitState::Closed);
    
    circuit_breaker.record_failure();
    assert_eq!(circuit_breaker.get_state(), CircuitState::Open);
}
```

### Step 3: Write Minimal Implementation

Write the minimum code necessary to make the test pass:

```rust
impl CircuitBreaker {
    fn record_failure(&self) {
        let mut failures = self.consecutive_failures.lock().unwrap();
        *failures += 1;
        
        if *failures >= self.config.failure_threshold {
            *self.state.lock().unwrap() = CircuitState::Open;
        }
    }
}
```

### Step 4: Refactor

Once the test passes, refactor the code to improve its structure while keeping the tests passing:

```rust
impl CircuitBreaker {
    fn record_failure(&self) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.failure_count += 1;
        
        let mut failures = self.consecutive_failures.lock().unwrap();
        *failures += 1;
        
        if *failures >= self.config.failure_threshold {
            let mut state = self.state.lock().unwrap();
            *state = CircuitState::Open;
            metrics.open_transitions += 1;
            
            // Schedule transition to half-open
            self.schedule_half_open_transition();
        }
    }
}
```

### Step 5: Extend with More Tests

Add more tests to cover additional aspects of the feature:

```rust
#[test]
fn test_circuit_breaker_half_open_after_timeout() {
    let circuit_breaker = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_ms: 100, // Short timeout for testing
        half_open_max_calls: 1,
    });
    
    // Trip the circuit
    for _ in 0..3 {
        circuit_breaker.record_failure();
    }
    assert_eq!(circuit_breaker.get_state(), CircuitState::Open);
    
    // Wait for timeout
    std::thread::sleep(std::time::Duration::from_millis(200));
    
    // Should transition to half-open
    assert_eq!(circuit_breaker.get_state(), CircuitState::HalfOpen);
}
```

## Integration with BDD

For a complete TDD cycle, integrate with BDD tests:

1. Start with a feature file describing the desired behavior
2. Implement unit tests for the components needed for the feature
3. Implement the components to make the unit tests pass
4. Implement integration tests to verify components work together
5. Ensure the BDD tests pass, verifying the feature meets business requirements

## Tools and Frameworks

The Cascade project uses the following tools for TDD:

- **Rust's built-in test framework** for unit and integration tests
- **Cucumber** for BDD tests
- **cascade-test-utils** for common testing utilities
- **CircleCI** for running tests in CI/CD pipeline

## Best Practices

1. **Write tests first**: Always write tests before implementing functionality
2. **Keep tests focused**: Each test should verify a single behavior
3. **Use descriptive test names**: Test names should describe what they're testing
4. **Refactor regularly**: Improve code structure while keeping tests passing
5. **Run tests regularly**: Run relevant tests after each change
6. **Maintain test coverage**: Aim for high test coverage of critical code paths
7. **Mock external dependencies**: Use mocks to isolate tests from external dependencies

## Example TDD Workflow in Cascade

### User Story

"As a service consumer, I want to use the circuit breaker pattern to prevent cascading failures when calling external services."

### BDD Scenario

```gherkin
Feature: Circuit Breaker Pattern
  As a platform user
  I want to use the circuit breaker pattern
  So that my system can handle external service failures gracefully

  Scenario: Circuit breaker opens after consecutive failures
    Given an external service that fails on every call
    When I make 3 consecutive calls to the service
    Then the circuit breaker should transition to "OPEN" state
```

### Unit Tests

1. Write tests for the CircuitBreaker component
2. Write tests for the state transitions
3. Write tests for the timer behavior

### Implementation

1. Implement the CircuitBreaker component
2. Implement the state management
3. Implement the timer functionality

### Integration Tests

1. Test the CircuitBreaker with a mock HTTP service
2. Test failure scenarios and recovery

### BDD Implementation

1. Implement the step definitions for the BDD scenario
2. Run the BDD tests to verify the feature works as expected

## Conclusion

Test-Driven Development is a powerful approach that helps ensure code quality, maintainability, and correctness. By following TDD principles in the Cascade project, we can build a robust and reliable platform that meets user requirements. 
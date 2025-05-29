Feature: Circuit Breaker
  As a developer
  I want to use a circuit breaker
  So that my system is resilient to failures

  Background:
    Given the Cascade server is running
    And the circuit breaker system is configured with failure threshold 3 and timeout 5000

  Scenario: Circuit breaker opens after failures
    Given an external service that fails on every call
    When I make 3 consecutive calls to the service
    Then the circuit breaker should transition to "OPEN" state
    And subsequent calls should fail fast without calling the service

  Scenario: Circuit breaker resets after timeout
    Given an external service that fails on every call
    When I make 3 consecutive calls to the service
    And I wait for 5000 milliseconds
    Then the circuit breaker should transition to "HALF_OPEN" state
    
  Scenario: Circuit breaker closes after successful call
    Given an external service with intermittent failures
    When I make 3 consecutive calls to the service
    And I wait for 5000 milliseconds
    And I make a call to the service
    Then the circuit breaker should transition to "CLOSED" state 
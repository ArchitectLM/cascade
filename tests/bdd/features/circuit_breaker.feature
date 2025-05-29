Feature: Circuit Breaker Pattern
  As a Cascade platform user
  I want to use the circuit breaker pattern
  So that my system can gracefully handle external service failures

  Background:
    Given the Cascade server is running
    And the circuit breaker system is configured with:
      | failure_threshold | 3            |
      | reset_timeout_ms  | 500          |
      | half_open_max     | 1            |

  Scenario: Circuit breaker opens after consecutive failures
    Given an external service that fails on every call
    When I make 3 consecutive calls to the service
    Then the circuit breaker should transition to "OPEN" state
    And additional calls should fail fast
    And I should receive a circuit open error

  Scenario: Circuit breaker transitions to half-open after timeout
    Given an external service that fails on every call
    When I make 3 consecutive calls to the service
    Then the circuit breaker should transition to "OPEN" state
    When I wait for 600 milliseconds
    Then the circuit breaker should transition to "HALF_OPEN" state

  Scenario: Circuit breaker closes after successful call in half-open state
    Given an external service that fails on every call
    When I make 3 consecutive calls to the service
    Then the circuit breaker should transition to "OPEN" state
    When I wait for 600 milliseconds
    Then the circuit breaker should transition to "HALF_OPEN" state
    When the external service starts working correctly
    And I make a call to the service
    Then the circuit breaker should transition to "CLOSED" state
    And subsequent calls should succeed

  Scenario: Circuit breaker returns to open state after failure in half-open state
    Given an external service that fails on every call
    When I make 3 consecutive calls to the service
    Then the circuit breaker should transition to "OPEN" state
    When I wait for 600 milliseconds
    Then the circuit breaker should transition to "HALF_OPEN" state
    When I make a call to the service
    Then the circuit breaker should transition to "OPEN" state
    And I should receive a service failure error

  Scenario: Circuit breaker metrics are tracked correctly
    Given an external service with intermittent failures
    When I execute the following sequence of calls:
      | call_number | outcome  |
      | 1           | success  |
      | 2           | failure  |
      | 3           | failure  |
      | 4           | failure  |
      | 5           | fast_fail|
      | 6           | fast_fail|
    And I wait for 600 milliseconds
    And I execute the following sequence of calls:
      | call_number | outcome  |
      | 7           | success  |
    Then the circuit breaker metrics should show:
      | metric            | value |
      | failure_count     | 3     |
      | success_count     | 2     |
      | open_count        | 1     |
      | half_open_count   | 1     |
      | closed_count      | 1     | 
Feature: Edge-to-Server Communication Patterns
  As a developer using the Cascade Platform
  I want to implement resilience patterns in edge-server communications
  So that my distributed applications are robust in the face of failures

  Background:
    Given the Cascade Server is running
    And the Edge environment is available

  Scenario: Circuit Breaker Pattern Prevents Cascading Failures
    Given I have deployed a flow with a circuit breaker pattern
    When I trigger a server component failure
    And I make multiple requests to the flow
    Then the circuit should open after failure threshold is reached
    And subsequent requests should fail fast with circuit breaker error
    And the circuit should attempt to transition to half-open after reset timeout
    And success should close the circuit again

  Scenario: Bulkhead Pattern Isolates Component Failures
    Given I have deployed a flow with bulkhead isolation
    When I send concurrent requests to the "standard" tier
    Then the standard tier requests should be processed with expected concurrency limits
    When I send concurrent requests to the "premium" tier
    Then the premium tier requests should be processed with higher concurrency limits
    When I overload the standard tier with too many concurrent requests
    Then excess requests should be rejected with pool exhausted error
    And premium tier should still function normally

  Scenario: Throttling Pattern Limits Request Rates
    Given I have deployed a flow with throttling configured
    When I send requests at a rate within the throttling limit
    Then all requests should be processed successfully
    When I send requests at a rate exceeding the throttling limit
    Then excess requests should be rejected with rate limit error
    And the response should include retry-after information
    And the rate limiting counters should reset after the time window

  Scenario: Dead Letter Queue Pattern Handles Failed Operations
    Given I have deployed a flow with dead letter queue pattern
    When I trigger an operation that will fail to process
    Then the operation should be routed to the DLQ
    And the DLQ should contain sanitized error details
    And the operation should be retried after the retry interval
    And the retry metrics should be tracked 
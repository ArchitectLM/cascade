Feature: Resilience Patterns
  As a developer using the Cascade Platform
  I want to use resilience patterns with shared state
  So that I can build reliable and fault-tolerant systems

  Background:
    Given the Cascade Server is running
    And shared state is configured

  Scenario: Rate limiting prevents excessive requests
    Given a flow with a rate limiter configured for 3 requests per second
    When I send 3 requests within 1 second
    Then all requests should succeed
    When I send another request within the same second
    Then the request should be rate limited
    When I wait for 1 second
    And I send another request
    Then the request should succeed

  Scenario: Circuit breaker prevents cascading failures
    Given a flow with a circuit breaker configured for 3 failures
    When I send 3 failed requests
    Then the circuit should be open
    When I send another request
    Then the request should be rejected due to open circuit
    When I wait for the circuit reset timeout
    Then the circuit should be half-open
    When I send a successful request
    Then the circuit should be closed

  Scenario: Bulkhead limits concurrent requests
    Given a flow with a bulkhead configured for 2 concurrent requests
    When I send 2 concurrent requests
    Then both requests should be processing
    When I send a third concurrent request
    Then the third request should be queued
    When the first request completes
    Then the third request should start processing 
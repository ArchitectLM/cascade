Feature: Platform Reliability Testing
  As a platform administrator
  I want to ensure the Cascade platform is resilient to failures
  So that applications built on it can operate reliably in real-world conditions

  Background:
    Given the Cascade server is running
    And the test environment is reset
    And reliability monitoring is enabled

  # System Restart Scenarios
  
  Scenario: Recover flow execution after system restart
    Given a long-running flow with persistent state is deployed
    And the flow has started execution and reached checkpoint "2"
    When the system is restarted
    And the system recovers
    Then the flow execution should resume from checkpoint "2"
    And the flow should complete successfully
    And all flow outputs should be consistent with a non-interrupted execution

  Scenario: Persist queued messages during restart
    Given a flow that processes messages from a queue is deployed
    And 20 messages have been enqueued
    And 5 messages have been processed
    When the system is restarted
    And the system recovers
    Then the remaining 15 messages should be processed correctly
    And no messages should be lost or duplicated

  Scenario: Maintain scheduled timers across restart
    Given a flow with multiple timers is deployed
    And the flow has scheduled executions at various future times
    When the system is restarted
    And the system recovers
    Then all scheduled timers should fire at their original scheduled times
    And no timer executions should be lost

  # Network Failure Scenarios

  Scenario: Handle network partition between server components
    Given a distributed flow spanning multiple server components is deployed
    When a network partition occurs between the components
    Then the system should detect the partition
    And affected flows should be paused or fail gracefully
    And when the network is restored, flow executions should resume
    And affected flows should complete successfully

  Scenario: Reconnect to edge nodes after network outage
    Given a flow with edge components is deployed to multiple edge nodes
    When a network outage occurs between the server and edge nodes
    Then the system should detect the network outage
    And pending interactions should be queued
    And when connectivity is restored, the system should automatically reconnect
    And queued operations should be processed in order
    And edge deployments should be reconciled with the server state

  Scenario: Handle database connection loss
    Given a flow that interacts with the database is deployed
    When the database connection is lost
    Then the system should retry operations with backoff
    And affected flows should be paused
    And when the database connection is restored
    Then flow executions should resume
    And the system should recover to a consistent state

  # External Service Failure Scenarios

  Scenario: Handle API dependency failure with circuit breaker
    Given a flow that depends on an external API is deployed
    And the circuit breaker is configured with a threshold of 3 failures
    When the external API starts returning 500 errors
    Then the circuit breaker should open after 3 failed attempts
    And subsequent requests should fail fast without calling the API
    And when the API recovers, the circuit should move to half-open state
    And after successful test requests, the circuit should close
    And the flow should resume normal operation

  Scenario: Buffer messages when downstream service is unavailable
    Given a flow that publishes messages to a downstream service is deployed
    When the downstream service becomes unavailable
    Then the system should buffer messages
    And retries should follow an exponential backoff pattern
    And when the downstream service recovers
    Then all buffered messages should be delivered
    And message order should be preserved

  Scenario: Handle partial system failure with bulkhead pattern
    Given multiple flows using different resource pools are deployed
    When one resource pool becomes exhausted
    Then only flows using that resource pool should be affected
    And flows using other resource pools should continue to operate normally
    And the affected resource pool should reject new requests
    And when the resource pool recovers
    Then normal operation should resume for all flows 
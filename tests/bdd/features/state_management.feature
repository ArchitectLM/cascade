Feature: Flow State Management
  As a developer using the Cascade platform
  I want to have reliable state persistence in flows
  So that I can build stateful applications with long-running processes

  Background:
    Given the Cascade server is running
    And a clean test environment

  # State Persistence Scenarios
  
  Scenario: Persist state between flow steps
    Given a flow with state persistence is deployed
    When I trigger the flow with input:
      """
      {
        "user": "test-user",
        "action": "start-process",
        "data": {
          "processId": "proc-12345",
          "timestamp": "2023-10-25T12:00:00Z"
        }
      }
      """
    Then the flow should complete successfully
    And the flow state should contain:
      | key           | value                   |
      | processId     | proc-12345              |
      | currentStep   | completed               |
      | processCount  | 1                       |
      | lastUpdated   | 2023-10-25T12:00:00Z    |
    And each step should have access to the state from previous steps

  Scenario: Resume flow execution from saved state
    Given a flow with checkpoints is deployed
    And the flow has been partially executed with state:
      """
      {
        "processId": "proc-12345",
        "currentStep": "step-2",
        "data": {
          "item1": "processed",
          "item2": "pending",
          "item3": "pending"
        },
        "progress": 33
      }
      """
    When I resume the flow with token "proc-12345"
    Then the flow should continue from "step-2"
    And the flow should complete successfully
    And the final state should show all items as "processed"
    And the progress should be 100

  Scenario: Clean up state after flow completion
    Given a flow with state persistence is deployed
    When I trigger the flow with input:
      """
      {
        "processId": "cleanup-test",
        "action": "full-process"
      }
      """
    And the flow completes successfully
    And I wait for the state TTL to expire
    Then the flow state should be removed from storage
    And attempting to resume the flow should fail with "ERR_FLOW_STATE_NOT_FOUND"

  # Timer Component Scenarios

  Scenario: Schedule delayed execution with timer component
    Given a flow with timer component is deployed
    When I trigger the flow with delay "5 seconds"
    Then the flow should acknowledge the scheduling
    And the timer component should schedule execution after "5 seconds"
    And after waiting "6 seconds" the delayed steps should execute
    And the flow should complete successfully

  Scenario: Cancel a scheduled timer
    Given a flow with timer component is deployed
    When I trigger the flow with delay "30 seconds"
    Then the flow should acknowledge the scheduling
    When I cancel the timer with the returned token
    Then the timer cancellation should succeed
    And after waiting "31 seconds" the delayed steps should not execute

  Scenario: Update an existing timer
    Given a flow with timer component is deployed
    When I trigger the flow with delay "30 seconds"
    Then the flow should acknowledge the scheduling
    When I update the timer to "5 seconds" with the returned token
    Then the timer update should succeed
    And after waiting "6 seconds" the delayed steps should execute
    And the flow should complete successfully

  Scenario: Persist timer across system restart
    Given a flow with timer component is deployed
    When I trigger the flow with delay "15 seconds"
    Then the flow should acknowledge the scheduling
    When the system is restarted
    And I wait for the system to recover
    And I wait for a total of "16 seconds" from the original scheduling
    Then the delayed steps should execute despite the restart
    And the flow should complete successfully 
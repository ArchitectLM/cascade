Feature: Flow Deployment and Edge Communication
  As a developer using the Cascade Platform
  I want to deploy flows from server to edge
  So that distributed processing can occur seamlessly

  Background:
    Given the Cascade Server is running
    And the Edge environment is available

  Scenario: Successfully deploy a valid flow
    When I deploy the following flow:
      """
      dsl_version: "1.0"
      definitions:
        components:
          - name: echo
            type: StdLib:Echo
            inputs:
              - name: input
                schema_ref: "schema:any"
            outputs:
              - name: output
                schema_ref: "schema:any"
        flows:
          - name: echo-flow
            description: "Simple echo flow"
            trigger:
              type: HttpEndpoint
              config:
                path: "/echo"
                method: "POST"
            steps:
              - step_id: echo-step
                component_ref: echo
                inputs_map:
                  input: "trigger.body"
      """
    Then the deployment response status should be 201
    And the deployment response should contain a flow_id
    And the deployment response should contain a version
    And the flow "echo-flow" should be deployed to the Edge
    And component manifests should be stored in the content store

  Scenario: Reject invalid flow DSL
    When I deploy the following flow:
      """
      This is not valid YAML
          - invalid: structure
      """
    Then the deployment response status should be 400
    And the deployment response should contain an error
    And the error details should include error code "ERR_DSL_PARSING_INVALID_YAML"

  Scenario: Update an existing flow
    Given I have deployed a flow with id "update-flow"
    When I update the flow "update-flow" with:
      """
      dsl_version: "1.0"
      definitions:
        components:
          - name: echo
            type: StdLib:Echo
            inputs:
              - name: input
                schema_ref: "schema:any"
            outputs:
              - name: output
                schema_ref: "schema:any"
          - name: transform
            type: StdLib:Transform
            inputs:
              - name: input
                schema_ref: "schema:any"
            outputs:
              - name: output
                schema_ref: "schema:any"
        flows:
          - name: update-flow
            description: "Updated flow with transform"
            trigger:
              type: HttpEndpoint
              config:
                path: "/echo-transform"
                method: "POST"
            steps:
              - step_id: echo-step
                component_ref: echo
                inputs_map:
                  input: "trigger.body"
              - step_id: transform-step
                component_ref: transform
                inputs_map:
                  input: "steps.echo-step.outputs.output"
                run_after: [echo-step]
      """
    Then the update response status should be 200
    And the flow version should be updated
    And the Edge deployment should include both steps

  Scenario: Process data with Edge-to-Server communication
    Given I have deployed a flow that implements Edge-to-Server processing
    When I send a request to the Edge endpoint
    Then the Edge should process the request
    And the Edge should send the processed data to the Server
    And the Server should return a successful response
    And the response should contain the combined processing results

  Scenario: Handle errors with standardized error codes
    Given I have deployed a flow with validation components
    When I send an invalid payload to the Edge endpoint
    Then the Edge should return a 400 response
    And the response should include error details
    And the error details should contain a standardized error code
    And the error should be logged for monitoring

  Scenario: Verify content addressing and reuse
    Given I have deployed a flow with reusable components
    When I deploy another flow using the same components
    Then the content store should reuse existing component hashes
    And the Edge deployment manifest should reference the same content hashes
    And both flows should function correctly 
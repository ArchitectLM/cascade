Feature: Authentication and Security
  As a platform administrator
  I want to ensure secure access to the Cascade platform
  So that only authorized users can access and modify resources

  Background:
    Given the Cascade server is running
    And the authentication system is properly configured

  # JWT Authentication Tests
  
  Scenario: Authenticate with valid credentials
    When I request an authentication token with valid credentials
    Then I should receive a valid JWT token
    And the token should have the correct user claims
    And the token should have appropriate expiration time

  Scenario: Authenticate with invalid credentials
    When I request an authentication token with invalid credentials
    Then I should receive an error response with status 401
    And the error code should be "ERR_AUTH_INVALID_CREDENTIALS"

  Scenario: Access API with valid token
    Given I have a valid authentication token
    When I make a request to a protected endpoint with the token
    Then the request should succeed with status 200
    And I should get the expected response data

  Scenario: Access API with expired token
    Given I have an expired authentication token
    When I make a request to a protected endpoint with the token
    Then I should receive an error response with status 401
    And the error code should be "ERR_AUTH_TOKEN_EXPIRED"
    And the error message should suggest token renewal

  Scenario: Access API with invalid token signature
    Given I have a token with invalid signature
    When I make a request to a protected endpoint with the token
    Then I should receive an error response with status 401
    And the error code should be "ERR_AUTH_INVALID_TOKEN"

  Scenario: Access API without token
    When I make a request to a protected endpoint without a token
    Then I should receive an error response with status 401
    And the error code should be "ERR_AUTH_MISSING_TOKEN"

  # Role-Based Access Control

  Scenario: Admin user accesses admin endpoint
    Given I have a valid authentication token with "admin" role
    When I make a request to an admin-only endpoint with the token
    Then the request should succeed with status 200

  Scenario: Regular user attempts to access admin endpoint
    Given I have a valid authentication token with "user" role
    When I make a request to an admin-only endpoint with the token
    Then I should receive an error response with status 403
    And the error code should be "ERR_AUTH_INSUFFICIENT_PERMISSIONS"

  # Secret Management Tests

  Scenario: Flow using environment variables
    Given I have a valid authentication token
    And a flow that uses environment variables is deployed
    When I execute the flow with the token
    Then the flow should complete successfully
    And the environment variables should be correctly resolved
    And the environment variables should not appear in logs

  Scenario: Flow using sensitive secrets
    Given I have a valid authentication token
    And a flow that uses sensitive secrets is deployed
    When I execute the flow with the token
    Then the flow should complete successfully
    And the sensitive secrets should be correctly resolved
    And the sensitive secrets should not appear in logs or responses
    And the secrets should be masked in debug outputs

  # Component Sandboxing Tests

  Scenario: WASM component with authorized resource access
    Given I have a valid authentication token
    And a flow with a WASM component using allowed resources is deployed
    When I execute the flow with the token
    Then the flow should complete successfully
    And the WASM component should have access to the allowed resources

  Scenario: WASM component with unauthorized resource access
    Given I have a valid authentication token
    And a flow with a WASM component attempting to access restricted resources is deployed
    When I execute the flow with the token
    Then the WASM sandbox should prevent the unauthorized access
    And the flow should fail with error code "ERR_COMPONENT_SANDBOX_VIOLATION"
    And the error details should include the specific violation 
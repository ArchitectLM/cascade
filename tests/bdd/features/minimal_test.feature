Feature: Basic Server Test
  As a user
  I want to ensure the server is running
  So that I can proceed with more complex tests

  Scenario: Server starts successfully
    Given the Cascade Server is running
    Then the server should be ready 
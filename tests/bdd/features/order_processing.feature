Feature: Order Processing Workflows
  As a customer
  I want to place orders through the Cascade platform
  So that I can purchase products with proper inventory and payment handling

  Background:
    Given the Cascade server is running
    And the order processing system is initialized
    And the inventory system is available
    And the payment system is available

  # Happy Path Scenarios
  
  Scenario: Process a basic order with a single item
    Given I have a customer with ID "cust-123"
    And product "prod-001" is in stock with quantity "50" and price "10.99"
    When I create an order with the following details:
      | customerId | productId | quantity | price  |
      | cust-123   | prod-001  | 2        | 10.99  |
    Then I should receive a successful order response
    And the order should be saved in the database
    And the inventory for product "prod-001" should be reduced by "2"
    And the payment should be processed for amount "21.98"
    And a confirmation message should be published

  Scenario: Process a multi-item order with different products
    Given I have a customer with ID "cust-123"
    And product "prod-001" is in stock with quantity "50" and price "10.99"
    And product "prod-002" is in stock with quantity "20" and price "24.99"
    And product "prod-003" is in stock with quantity "100" and price "5.50"
    When I create an order with the following details:
      | customerId | productId | quantity | price  |
      | cust-123   | prod-001  | 2        | 10.99  |
      | cust-123   | prod-002  | 1        | 24.99  |
      | cust-123   | prod-003  | 3        | 5.50   |
    Then I should receive a successful order response
    And the order should be saved in the database
    And the inventory for product "prod-001" should be reduced by "2"
    And the inventory for product "prod-002" should be reduced by "1"
    And the inventory for product "prod-003" should be reduced by "3"
    And the payment should be processed for amount "63.47"
    And a confirmation message should be published

  Scenario: Process an order with zero-quantity items
    Given I have a customer with ID "cust-123"
    And product "prod-001" is in stock with quantity "50" and price "10.99"
    And product "prod-002" is in stock with quantity "20" and price "24.99"
    When I create an order with the following details:
      | customerId | productId | quantity | price  |
      | cust-123   | prod-001  | 2        | 10.99  |
      | cust-123   | prod-002  | 0        | 24.99  |
    Then I should receive a successful order response
    And the order should be saved in the database
    And the inventory for product "prod-001" should be reduced by "2"
    And the inventory for product "prod-002" should not change
    And the payment should be processed for amount "21.98"
    And a confirmation message should be published

  Scenario: Process an order with free (zero price) items
    Given I have a customer with ID "cust-123"
    And product "prod-001" is in stock with quantity "50" and price "10.99"
    And product "prod-002" is in stock with quantity "20" and price "0.00"
    When I create an order with the following details:
      | customerId | productId | quantity | price  |
      | cust-123   | prod-001  | 1        | 10.99  |
      | cust-123   | prod-002  | 2        | 0.00   |
    Then I should receive a successful order response
    And the order should be saved in the database
    And the inventory for product "prod-001" should be reduced by "1"
    And the inventory for product "prod-002" should be reduced by "2"
    And the payment should be processed for amount "10.99"
    And a confirmation message should be published

  # Inventory Error Scenarios

  Scenario: Attempt to order with insufficient inventory
    Given I have a customer with ID "cust-123"
    And product "prod-001" is in stock with quantity "5" and price "10.99"
    When I create an order with the following details:
      | customerId | productId | quantity | price  |
      | cust-123   | prod-001  | 10       | 10.99  |
    Then I should receive an error response with status 409
    And the error code should be "ERR_COMPONENT_INVENTORY_INSUFFICIENT"
    And the inventory for product "prod-001" should not change
    And no order should be saved in the database
    And no payment should be processed

  Scenario: Attempt to order with partial inventory availability
    Given I have a customer with ID "cust-123"
    And product "prod-001" is in stock with quantity "50" and price "10.99"
    And product "prod-002" is in stock with quantity "2" and price "24.99"
    When I create an order with the following details:
      | customerId | productId | quantity | price  |
      | cust-123   | prod-001  | 2        | 10.99  |
      | cust-123   | prod-002  | 5        | 24.99  |
    Then I should receive an error response with status 409
    And the error code should be "ERR_COMPONENT_INVENTORY_INSUFFICIENT"
    And the error details should include product "prod-002"
    And the inventory for product "prod-001" should not change
    And the inventory for product "prod-002" should not change
    And no order should be saved in the database
    And no payment should be processed

  Scenario: Attempt to order a non-existent product
    Given I have a customer with ID "cust-123"
    When I create an order with the following details:
      | customerId | productId | quantity | price  |
      | cust-123   | non-exist | 1        | 10.99  |
    Then I should receive an error response with status 404
    And the error code should be "ERR_COMPONENT_PRODUCT_NOT_FOUND"
    And no order should be saved in the database
    And no payment should be processed

  # Payment Error Scenarios

  Scenario: Order with payment declined
    Given I have a customer with ID "cust-123"
    And product "prod-001" is in stock with quantity "50" and price "10.99"
    And the payment system will decline the next transaction
    When I create an order with the following details:
      | customerId | productId | quantity | price  |
      | cust-123   | prod-001  | 2        | 10.99  |
    Then I should receive an error response with status 402
    And the error code should be "ERR_COMPONENT_PAYMENT_DECLINED"
    And the inventory for product "prod-001" should not change
    And an order should be saved with status "PAYMENT_FAILED"

  Scenario: Order with payment gateway timeout
    Given I have a customer with ID "cust-123"
    And product "prod-001" is in stock with quantity "50" and price "10.99"
    And the payment system will time out
    When I create an order with the following details:
      | customerId | productId | quantity | price  |
      | cust-123   | prod-001  | 2        | 10.99  |
    Then I should receive an error response with status 504
    And the error should contain timeout details
    And retry logic should be triggered
    And after retries fail, the inventory for product "prod-001" should not change
    And no order should be saved in the database

  Scenario: Order with invalid payment details
    Given I have a customer with ID "cust-123"
    And product "prod-001" is in stock with quantity "50" and price "10.99"
    When I create an order with invalid payment token
    Then I should receive an error response with status 400
    And the error code should be "ERR_VALIDATION_PAYMENT_DETAILS"
    And the inventory for product "prod-001" should not change
    And no order should be saved in the database

  # Validation Error Scenarios

  Scenario: Order with missing required fields
    Given I have a customer with ID "cust-123"
    When I create an order with missing customer ID
    Then I should receive an error response with status 400
    And the error code should be "ERR_VALIDATION_REQUIRED_FIELD"
    And the error details should mention "customerId"
    
  Scenario: Order with invalid field values
    Given I have a customer with ID "cust-123"
    When I create an order with the following invalid details:
      | customerId | productId | quantity | price  |
      | cust-123   | prod-001  | -2       | 10.99  |
    Then I should receive an error response with status 400
    And the error code should be "ERR_VALIDATION_INVALID_VALUE"
    And the error details should mention "quantity"

  Scenario: Order with schema validation errors
    Given I have a customer with ID "cust-123"
    When I create an order with invalid JSON format
    Then I should receive an error response with status 400
    And the error code should be "ERR_VALIDATION_SCHEMA" 
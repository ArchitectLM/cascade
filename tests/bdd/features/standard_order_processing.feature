Feature: Standard Order Processing
  As a business
  I want to process customer orders
  So that I can fulfill customer requests and generate revenue

  Background:
    Given the Cascade server is running
    And the order processing system is initialized
    And the inventory system is available
    And the payment system is available
    And I have a customer with ID "customer-123"
    And product "prod-001" is in stock with quantity "10" and price "19.99"
    And a flow "order-processing" is deployed with DSL:
    """
    flow "order-processing" {
      start {
        validate_order()
        check_inventory()
        process_payment()
        update_inventory()
        confirm_order()
      }
    }
    """

  Scenario: Successfully process a valid order
    When I create an order with product prod-001 and quantity 2
    Then I should receive a successful order response
    And the order should be saved in the database
    And the inventory for product "prod-001" should be reduced by "2"
    And the payment should be processed for amount "39.98"
    
  Scenario: Reject order with invalid data
    When I create an order with missing customer ID
    Then I should receive an error response with status 400
    And the error code should be "INVALID_ORDER"
    And no order should be saved in the database 
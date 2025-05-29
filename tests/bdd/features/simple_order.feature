Feature: Simple Order
  As a customer
  I want to create an order
  So that I can get the products I need

  Scenario: Create a simple order
    Given I have a customer with ID "test-customer"
    When I create an order with product prod-001 and quantity 2
    Then I should receive a successful order response 
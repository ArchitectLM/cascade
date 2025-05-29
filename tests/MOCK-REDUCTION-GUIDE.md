# Strategic Mock Reduction Guide

This guide outlines practical strategies for reducing unnecessary mocks in the Cascade test suite, improving test reliability, readability, and maintenance.

## Why Reduce Mocks?

Excessive mocking leads to several problems:

1. **Brittle Tests**: Tests coupled to implementation details break when internal implementation changes
2. **False Confidence**: Tests pass but don't accurately verify real system behavior
3. **Maintenance Burden**: Complex mock setups require significant maintenance
4. **Difficult Refactoring**: Heavy mocking makes codebase refactoring risky
5. **Poor Documentation**: Heavily mocked tests don't document how components should work together

## Analyzing Current Mocks

Before reducing mocks, categorize existing mocks in the test suite:

### Essential Mocks (Keep)

- External APIs (HTTP services, databases, etc.)
- File system operations
- Time-dependent operations that would make tests slow or flaky
- Resource-intensive operations

### Non-Essential Mocks (Reduce/Replace)

- Simple internal utilities
- Pure functions
- In-process component interactions
- Easily initializable components

## Test Double Hierarchy

Use the appropriate test double for each situation:

1. **Stub**: Returns fixed values, no verification. Best for:
   - Providing input data to the system under test
   - Replacing slow components where behavior isn't being tested

2. **Mock**: Verifies interactions. Best for:
   - Verifying critical interactions with external systems
   - Confirming side effects that can't be directly observed

3. **Fake**: Lightweight implementation of the real thing. Best for:
   - Databases (in-memory implementations)
   - File systems (in-memory implementations)
   - Time (controlled time service)

4. **Real Implementation**: Actual component. Best for:
   - Core business logic
   - In-memory state management
   - Component composition
   - Data transformations

## Practical Implementation

### Step 1: Identify Test Types

For each test type, define the appropriate mock reduction strategy:

#### Unit Tests
- Replace fine-grained mocks with coarser-grained stubs where possible
- Use real implementations for pure functions and simple utilities
- Keep mocks for external dependencies

#### Integration Tests
- Use real implementations for all components being integrated
- Use fakes for slow or external dependencies
- Restrict mocks to boundary interactions

#### E2E Tests
- Use real implementations for the entire system where possible
- Use controlled test environments for external services
- Only mock components that can't be reasonably included in tests

### Step 2: Create Useful Test Doubles

Invest in creating high-quality test doubles that can be reused across tests:

1. **In-Memory Database**: Faster than a real DB but maintains real behavior
2. **Test HTTP Server**: Real HTTP server with controlled responses
3. **Time Controller**: Allows advancing time without actual delays
4. **Test Event Bus**: In-memory message bus for component communication

### Step 3: Apply to Existing Tests

1. Start with integration tests - most valuable for mock reduction
2. For each test, ask:
   - What is actually being tested?
   - Which mocks could be replaced with real implementations?
   - Which mocks are essential for test isolation?

3. Replace one mock at a time, ensuring tests still pass

## Examples

### Bad: Over-mocking
```rust
#[test]
fn test_order_processing() {
    // Mock the inventory service
    let inventory = mock_inventory_service();
    inventory.expect_check_availability().returns(true);

    // Mock the payment service
    let payment = mock_payment_service();
    payment.expect_process_payment().returns(Ok(()));

    // Mock the notification service
    let notification = mock_notification_service();
    notification.expect_send_notification().returns(Ok(()));

    // Test the order service using all mocks
    let order_service = OrderService::new(inventory, payment, notification);
    let result = order_service.process_order(test_order());
    
    assert!(result.is_ok());
}
```

### Good: Strategic Mocking
```rust
#[test]
fn test_order_processing() {
    // Use real inventory implementation with in-memory storage
    let inventory = InMemoryInventoryService::new();
    inventory.add_item("item-1", 10);
    
    // Use real payment implementation with test mode
    let payment = PaymentService::new_test_mode();
    
    // Only mock the notification service (external dependency)
    let notification = mock_notification_service();
    notification.expect_send_notification().returns(Ok(()));

    // Test the order service with more real components
    let order_service = OrderService::new(inventory, payment, notification);
    let result = order_service.process_order(test_order());
    
    assert!(result.is_ok());
    // Can also verify real side effects in inventory
    assert_eq!(inventory.get_quantity("item-1"), 9);
}
```

## Implementation Timeline

Based on the TEST-IMPROVEMENT-PLAN.md, follow this timeline to gradually reduce mocks:

1. **Week 1-2**: Audit mock usage, create mock reduction plan
2. **Week 3-4**: Create core test doubles (in-memory implementations)
3. **Week 5-6**: Apply to integration tests
4. **Week 7-8**: Apply to E2E tests
5. **Week 9-10**: Apply to BDD tests

## Measurement and Success Criteria

Track these metrics to measure success:

1. **Mock Ratio**: Number of mocked components vs. real implementations
2. **Test Reliability**: Number of flaky tests (should decrease)
3. **Test Maintainability**: Time spent fixing broken tests after refactoring
4. **Test Coverage**: Overall code coverage (should remain stable or increase)

Aim to reduce non-essential mocks by 50% in integration and E2E tests within the implementation timeline. 
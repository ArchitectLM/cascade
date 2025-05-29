Feature: Cascade Platform Performance Testing
  As a platform operator
  I want to verify the performance characteristics of the Cascade platform
  So that I can ensure it meets required performance SLAs

  Background:
    Given the Cascade Server is running
    And the Edge environment is available
    And the performance test data is initialized

  @api_latency
  Scenario Outline: API endpoints respond within acceptable latency thresholds
    When I make <request_count> requests to the "<endpoint>" endpoint
    Then the average response time should be less than <avg_threshold> ms
    And the 95th percentile response time should be less than <p95_threshold> ms
    And the 99th percentile response time should be less than <p99_threshold> ms
    And no individual response should exceed <max_threshold> ms

    Examples:
      | endpoint            | request_count | avg_threshold | p95_threshold | p99_threshold | max_threshold |
      | /api/flows/list     | 100           | 50            | 100           | 150           | 300           |
      | /api/flows/status   | 100           | 100           | 200           | 300           | 500           |
      | /api/edges/list     | 100           | 50            | 100           | 150           | 300           |
      | /api/content/list   | 100           | 200           | 400           | 600           | 1000          |
      | /health             | 100           | 20            | 50            | 100           | 200           |

  @throughput
  Scenario Outline: Server handles concurrent requests with acceptable throughput
    When I send <concurrent_users> concurrent requests for <duration> seconds
    Then the system should handle at least <min_throughput> requests per second
    And the error rate should be less than <max_error_rate> percent
    And the average CPU utilization should be less than <max_cpu> percent
    And the average memory utilization should be less than <max_memory> MB

    Examples:
      | concurrent_users | duration | min_throughput | max_error_rate | max_cpu | max_memory |
      | 10               | 30       | 100            | 0.1            | 60      | 1024       |
      | 50               | 30       | 300            | 0.5            | 70      | 1536       |
      | 100              | 30       | 500            | 1.0            | 80      | 2048       |

  @edge_performance
  Scenario: Edge nodes efficiently process flow executions
    Given a test flow with 10 components is deployed to all edge nodes
    When I execute the flow simultaneously on 5 edge nodes
    Then each edge node should process at least 50 executions per second
    And the average execution time per component should be less than 10 ms
    And the CPU utilization per edge node should be less than 70 percent
    And the memory utilization per edge node should be less than 1024 MB

  @deployment_performance
  Scenario Outline: Flow deployment completes within acceptable time frames
    Given I have a flow definition with <component_count> components
    When I deploy the flow to <edge_count> edge nodes
    Then the deployment should complete in less than <max_seconds> seconds
    And the server CPU utilization should not exceed 80 percent during deployment
    And the edge CPU utilization should not exceed 70 percent during deployment

    Examples:
      | component_count | edge_count | max_seconds |
      | 5               | 3          | 2           |
      | 20              | 3          | 5           |
      | 50              | 3          | 10          |
      | 100             | 3          | 20          |

  @content_store
  Scenario: Content store efficiently handles large data volumes
    Given the content store contains 1000 content entries
    When I perform 100 concurrent content retrieval operations
    Then the average retrieval time should be less than 50 ms
    And the 95th percentile retrieval time should be less than 200 ms
    And content store disk I/O should be less than 50 MB/s
    And the memory utilization should not increase by more than 100 MB

  @database_performance
  Scenario: Database handles flow state operations efficiently
    Given 100 flows with active state are in the database
    When I perform 1000 state update operations over 60 seconds
    Then the average state update operation should take less than 20 ms
    And the database connection pool should not exceed 80% utilization
    And the database CPU usage should be less than 70%
    And the database query cache hit rate should be above 80%

  @scaling
  Scenario: System performance scales with additional server instances
    Given the Cascade Server is running with 1 instance
    When I increase the server instances to 3
    And I run a throughput test with 200 concurrent users for 30 seconds
    Then the system throughput should increase by at least 150%
    And the average response time should decrease by at least 30%
    And the load should be evenly distributed across all instances
    And no individual instance CPU usage should exceed 70%

  @resource_efficiency
  Scenario: System efficiently uses resources during idle periods
    Given the Cascade platform has been running for 30 minutes with no traffic
    Then the server CPU utilization should be less than 5%
    And the server memory utilization should be stable within 50 MB
    And the edge CPU utilization should be less than 2%
    And the edge memory utilization should be stable within 30 MB
    And no memory leaks should be detected in any component 
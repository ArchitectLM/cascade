//! Generators for creating test DSL code.

/// Creates a minimal valid flow DSL.
///
/// # Returns
///
/// A string containing a minimal valid flow DSL.
pub fn create_minimal_flow_dsl() -> String {
    r#"
flow:
  id: minimal-flow
  components:
    - id: http-component
      type: HttpCall
      config:
        url: "https://example.com/api"
        method: GET
  steps:
    - id: http-step
      component: http-component
      deployment_location: server
"#.to_string()
}

/// Creates a flow DSL with the specified ID.
///
/// # Arguments
///
/// * `flow_id` - The ID to use for the flow
///
/// # Returns
///
/// A string containing a valid flow DSL with the specified ID.
pub fn create_flow_dsl_with_id(flow_id: &str) -> String {
    format!(r#"
flow:
  id: {}
  components:
    - id: http-component
      type: HttpCall
      config:
        url: "https://example.com/api"
        method: GET
  steps:
    - id: http-step
      component: http-component
      deployment_location: server
"#, flow_id)
}

/// Creates a flow DSL with HTTP call component.
///
/// # Arguments
///
/// * `flow_id` - The ID to use for the flow
/// * `url` - The URL to use for the HTTP call
/// * `method` - The HTTP method to use (GET, POST, etc.)
///
/// # Returns
///
/// A string containing a valid flow DSL with an HTTP call component.
pub fn create_http_flow_dsl(flow_id: &str, url: &str, method: &str) -> String {
    format!(r#"
flow:
  id: {}
  components:
    - id: http-component
      type: HttpCall
      config:
        url: "{}"
        method: {}
  steps:
    - id: http-step
      component: http-component
      deployment_location: server
"#, flow_id, url, method)
}

/// Creates a flow DSL with a sequence of steps.
///
/// # Arguments
///
/// * `flow_id` - The ID to use for the flow
///
/// # Returns
///
/// A string containing a valid flow DSL with a sequence of steps.
pub fn create_sequential_flow_dsl(flow_id: &str) -> String {
    format!(r#"
flow:
  id: {}
  components:
    - id: first-http
      type: HttpCall
      config:
        url: "https://example.com/api/first"
        method: GET
    - id: second-http
      type: HttpCall
      config:
        url: "https://example.com/api/second"
        method: POST
    - id: third-http
      type: HttpCall
      config:
        url: "https://example.com/api/third"
        method: PUT
  steps:
    - id: step1
      component: first-http
      deployment_location: server
    - id: step2
      component: second-http
      deployment_location: server
      run_after:
        - step1
    - id: step3
      component: third-http
      deployment_location: server
      run_after:
        - step2
"#, flow_id)
}

/// Creates a flow DSL with a parallel steps.
///
/// # Arguments
///
/// * `flow_id` - The ID to use for the flow
///
/// # Returns
///
/// A string containing a valid flow DSL with parallel steps.
pub fn create_parallel_flow_dsl(flow_id: &str) -> String {
    format!(r#"
flow:
  id: {}
  components:
    - id: trigger-http
      type: HttpCall
      config:
        url: "https://example.com/api/trigger"
        method: GET
    - id: parallel-http-1
      type: HttpCall
      config:
        url: "https://example.com/api/parallel/1"
        method: GET
    - id: parallel-http-2
      type: HttpCall
      config:
        url: "https://example.com/api/parallel/2"
        method: GET
    - id: join-http
      type: HttpCall
      config:
        url: "https://example.com/api/join"
        method: POST
  steps:
    - id: trigger-step
      component: trigger-http
      deployment_location: server
    - id: parallel-step-1
      component: parallel-http-1
      deployment_location: server
      run_after:
        - trigger-step
    - id: parallel-step-2
      component: parallel-http-2
      deployment_location: server
      run_after:
        - trigger-step
    - id: join-step
      component: join-http
      deployment_location: server
      run_after:
        - parallel-step-1
        - parallel-step-2
"#, flow_id)
}

/// Creates a flow DSL with a conditional branch (switch).
///
/// # Arguments
///
/// * `flow_id` - The ID to use for the flow
///
/// # Returns
///
/// A string containing a valid flow DSL with a conditional branch.
pub fn create_conditional_flow_dsl(flow_id: &str) -> String {
    format!(r#"
flow:
  id: {}
  components:
    - id: input-http
      type: HttpCall
      config:
        url: "https://example.com/api/input"
        method: GET
    - id: switch-component
      type: Switch
      config:
        cases:
          - condition: "$.status == 'success'"
            output: "success"
          - condition: "$.status == 'error'"
            output: "error"
          - condition: "true"
            output: "unknown"
    - id: success-http
      type: HttpCall
      config:
        url: "https://example.com/api/success"
        method: POST
    - id: error-http
      type: HttpCall
      config:
        url: "https://example.com/api/error"
        method: POST
    - id: unknown-http
      type: HttpCall
      config:
        url: "https://example.com/api/unknown"
        method: POST
  steps:
    - id: input-step
      component: input-http
      deployment_location: server
    - id: switch-step
      component: switch-component
      deployment_location: server
      run_after:
        - input-step
      inputs:
        input:
          from: input-step.output
    - id: success-step
      component: success-http
      deployment_location: server
      run_after:
        - switch-step
      condition: "switch-step.output == 'success'"
    - id: error-step
      component: error-http
      deployment_location: server
      run_after:
        - switch-step
      condition: "switch-step.output == 'error'"
    - id: unknown-step
      component: unknown-http
      deployment_location: server
      run_after:
        - switch-step
      condition: "switch-step.output == 'unknown'"
"#, flow_id)
}

/// Creates a flow DSL with a retry pattern.
///
/// # Arguments
///
/// * `flow_id` - The ID to use for the flow
///
/// # Returns
///
/// A string containing a valid flow DSL with a retry pattern.
pub fn create_retry_flow_dsl(flow_id: &str) -> String {
    format!(r#"
flow:
  id: {}
  components:
    - id: http-component
      type: HttpCall
      config:
        url: "https://example.com/api/flaky"
        method: GET
    - id: retry-wrapper
      type: RetryWrapper
      config:
        maxAttempts: 3
        initialDelay: 1000
        backoffMultiplier: 2.0
  steps:
    - id: retry-step
      component: retry-wrapper
      deployment_location: server
      wrapped_step:
        component: http-component
"#, flow_id)
}

/// Creates a flow DSL with edge and server components.
///
/// # Arguments
///
/// * `flow_id` - The ID to use for the flow
///
/// # Returns
///
/// A string containing a valid flow DSL with edge and server components.
pub fn create_hybrid_flow_dsl(flow_id: &str) -> String {
    format!(r#"
flow:
  id: {}
  components:
    - id: edge-component
      type: HttpCall
      config:
        url: "https://example.com/api/edge"
        method: GET
    - id: server-component
      type: HttpCall
      config:
        url: "https://example.com/api/server"
        method: POST
    - id: edge-transform
      type: Transform
      config:
        expression: "{{ input | json_encode }}"
  steps:
    - id: edge-step
      component: edge-component
      deployment_location: edge
    - id: transform-step
      component: edge-transform
      deployment_location: edge
      run_after:
        - edge-step
      inputs:
        input:
          from: edge-step.output
    - id: server-step
      component: server-component
      deployment_location: server
      run_after:
        - transform-step
      inputs:
        body:
          from: transform-step.output
"#, flow_id)
}

/// Creates a flow DSL with circuit breaker pattern.
///
/// # Arguments
///
/// * `flow_id` - The ID to use for the flow
///
/// # Returns
///
/// A string containing a valid flow DSL with a circuit breaker pattern.
pub fn create_circuit_breaker_flow_dsl(flow_id: &str) -> String {
    format!(r#"
flow:
  id: {}
  components:
    - id: http-component
      type: HttpCall
      config:
        url: "https://example.com/api/protected"
        method: GET
    - id: circuit-breaker
      type: CircuitBreaker
      config:
        failureThreshold: 3
        resetTimeout: 30000
        fallbackValue: {{ {{ "status": "fallback" }} }}
  steps:
    - id: protected-call
      component: circuit-breaker
      deployment_location: server
      wrapped_step:
        component: http-component
"#, flow_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_minimal_flow_dsl() {
        let dsl = create_minimal_flow_dsl();
        assert!(dsl.contains("flow:"));
        assert!(dsl.contains("id: minimal-flow"));
        assert!(dsl.contains("type: HttpCall"));
    }
    
    #[test]
    fn test_create_flow_dsl_with_id() {
        let flow_id = "test-flow-123";
        let dsl = create_flow_dsl_with_id(flow_id);
        assert!(dsl.contains(&format!("id: {}", flow_id)));
    }
    
    #[test]
    fn test_create_http_flow_dsl() {
        let flow_id = "http-flow";
        let url = "https://test.example.com";
        let method = "POST";
        
        let dsl = create_http_flow_dsl(flow_id, url, method);
        assert!(dsl.contains(&format!("id: {}", flow_id)));
        assert!(dsl.contains(&format!("url: \"{}\"", url)));
        assert!(dsl.contains(&format!("method: {}", method)));
    }
    
    #[test]
    fn test_create_sequential_flow_dsl() {
        let flow_id = "sequential-flow";
        let dsl = create_sequential_flow_dsl(flow_id);
        
        assert!(dsl.contains(&format!("id: {}", flow_id)));
        assert!(dsl.contains("step1"));
        assert!(dsl.contains("step2"));
        assert!(dsl.contains("step3"));
        assert!(dsl.contains("run_after:"));
    }
    
    #[test]
    fn test_create_parallel_flow_dsl() {
        let flow_id = "parallel-flow";
        let dsl = create_parallel_flow_dsl(flow_id);
        
        assert!(dsl.contains(&format!("id: {}", flow_id)));
        assert!(dsl.contains("parallel-step-1"));
        assert!(dsl.contains("parallel-step-2"));
        assert!(dsl.contains("join-step"));
        
        // Join step should have two dependencies
        assert!(dsl.contains("  run_after:\n        - parallel-step-1\n        - parallel-step-2"));
    }
    
    #[test]
    fn test_create_conditional_flow_dsl() {
        let flow_id = "conditional-flow";
        let dsl = create_conditional_flow_dsl(flow_id);
        
        assert!(dsl.contains(&format!("id: {}", flow_id)));
        assert!(dsl.contains("Switch"));
        assert!(dsl.contains("condition:"));
        assert!(dsl.contains("success-step"));
        assert!(dsl.contains("error-step"));
    }
    
    #[test]
    fn test_create_retry_flow_dsl() {
        let flow_id = "retry-flow";
        let dsl = create_retry_flow_dsl(flow_id);
        
        assert!(dsl.contains(&format!("id: {}", flow_id)));
        assert!(dsl.contains("RetryWrapper"));
        assert!(dsl.contains("maxAttempts: 3"));
        assert!(dsl.contains("wrapped_step:"));
    }
    
    #[test]
    fn test_create_hybrid_flow_dsl() {
        let flow_id = "hybrid-flow";
        let dsl = create_hybrid_flow_dsl(flow_id);
        
        assert!(dsl.contains(&format!("id: {}", flow_id)));
        assert!(dsl.contains("deployment_location: edge"));
        assert!(dsl.contains("deployment_location: server"));
    }
    
    #[test]
    fn test_create_circuit_breaker_flow_dsl() {
        let flow_id = "circuit-breaker-flow";
        let dsl = create_circuit_breaker_flow_dsl(flow_id);
        
        assert!(dsl.contains(&format!("id: {}", flow_id)));
        assert!(dsl.contains("CircuitBreaker"));
        assert!(dsl.contains("failureThreshold:"));
        assert!(dsl.contains("fallbackValue:"));
    }
} 
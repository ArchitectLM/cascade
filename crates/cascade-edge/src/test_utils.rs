#![cfg(feature = "test_mockall")]

//! Simple test utilities for cascade-edge
//! This module is primarily for demonstration purposes and tests

use crate::resilience::{StateStore, mocks::MockStateStore};
use std::sync::Arc;
use serde_json::json;

/// Example test showing how to use the MockStateStore for resilience pattern testing
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mock_state_store_example() {
        println!("Starting test_mock_state_store_example");
        
        // Create a mock state store
        let state_store = MockStateStore::new();
        
        // Set some test state for a component
        let test_component_id = "test-component";
        let test_state = json!({"test_value": 42});
        state_store.set_component_state(test_component_id, test_state.clone()).await.unwrap();
        
        // Now retrieve the state
        let state_result = state_store.get_component_state(test_component_id).await;
        println!("get_component_state result: {:?}", state_result);
        
        let state = state_result.unwrap();
        assert!(state.is_some(), "State should be returned from the mock");
        
        let value = state.unwrap();
        println!("State value: {:?}", value);
        assert_eq!(value["test_value"], 42);
        
        // Test flow instance state
        let flow_id = "test-flow";
        let instance_id = "test-instance";
        let flow_state = json!({"flow_state": true});
        
        state_store.set_flow_instance_state(flow_id, instance_id, flow_state.clone()).await.unwrap();
        
        let retrieved_flow_state = state_store.get_flow_instance_state(flow_id, instance_id).await.unwrap();
        assert!(retrieved_flow_state.is_some());
        assert_eq!(retrieved_flow_state.unwrap()["flow_state"], true);
        
        // Clear all state and verify it's gone
        state_store.clear().await;
        
        let empty_state = state_store.get_component_state(test_component_id).await.unwrap();
        assert!(empty_state.is_none());
    }
} 
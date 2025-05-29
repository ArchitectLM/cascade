//! Manual test script for order processing

use reqwest::Client;
use serde_json::json;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a reqwest client
    let client = Client::new();
    
    // Step 1: Deploy the order-processing flow
    println!("Deploying order-processing flow...");
    let flow_dsl = json!({
        "id": "order-processing",
        "name": "Order Processing Flow",
        "description": "A flow for processing customer orders",
        "steps": [
            {
                "id": "validate-order",
                "component_ref": "StdLib:JsonValidator",
                "description": "Validate the order format"
            },
            {
                "id": "check-inventory",
                "component_ref": "StdLib:InventoryCheck",
                "description": "Check if items are in stock"
            },
            {
                "id": "process-payment",
                "component_ref": "StdLib:PaymentProcessor",
                "description": "Process payment for the order"
            },
            {
                "id": "update-inventory",
                "component_ref": "StdLib:InventoryUpdate",
                "description": "Update inventory levels"
            },
            {
                "id": "confirm-order",
                "component_ref": "StdLib:OrderConfirmation",
                "description": "Send confirmation to customer"
            }
        ]
    });
    
    let deploy_response = client
        .post("http://localhost:3000/api/flows")
        .json(&flow_dsl)
        .send()
        .await?;
    
    println!("Flow deployment status: {}", deploy_response.status());
    
    // Step 2: Initialize inventory
    println!("Initializing inventory...");
    let inventory_data = json!({
        "product_id": "prod-001",
        "quantity": 10,
        "price": 19.99
    });
    
    let inventory_response = client
        .post("http://localhost:3000/api/inventory")
        .json(&inventory_data)
        .send()
        .await?;
    
    println!("Inventory initialization status: {}", inventory_response.status());
    
    // Step 3: Create a test customer
    println!("Creating test customer...");
    let customer_data = json!({
        "customer_id": "customer-123",
        "name": "Test Customer",
        "email": "test@example.com"
    });
    
    let customer_response = client
        .post("http://localhost:3000/api/customers")
        .json(&customer_data)
        .send()
        .await?;
    
    println!("Customer creation status: {}", customer_response.status());
    
    // Step 4: Submit an order
    println!("Submitting order...");
    let order_data = json!({
        "customerId": "customer-123",
        "items": [
            {
                "productId": "prod-001",
                "quantity": 2
            }
        ]
    });
    
    let order_response = client
        .post("http://localhost:3000/api/flows/order-processing/trigger")
        .json(&order_data)
        .send()
        .await?;
    
    println!("Order submission status: {}", order_response.status());
    
    if order_response.status().is_success() {
        // Parse the response to get the flow instance ID
        let response_body = order_response.json::<serde_json::Value>().await?;
        println!("Order response: {}", serde_json::to_string_pretty(&response_body)?);
        
        if let Some(instance_id) = response_body.get("instanceId").and_then(|v| v.as_str()) {
            // Step 5: Check the flow instance status
            println!("Waiting for flow completion...");
            tokio::time::sleep(Duration::from_secs(2)).await;
            
            let status_response = client
                .get(&format!("http://localhost:3000/api/flows/instances/{}", instance_id))
                .send()
                .await?;
            
            println!("Flow instance status: {}", status_response.status());
            
            if status_response.status().is_success() {
                let status_body = status_response.json::<serde_json::Value>().await?;
                println!("Flow instance details: {}", serde_json::to_string_pretty(&status_body)?);
            }
        }
    }
    
    Ok(())
} 
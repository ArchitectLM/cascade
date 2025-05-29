use cascade_test_utils::{config::TestConfig, client::TestClient, server::TestServer, builders::TestServerHandles};
use cucumber::World;
use reqwest::{Response, StatusCode};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::net::SocketAddr;

/// Mock database for tracking orders in BDD tests
#[derive(Default, Clone, Debug)]
pub struct MockOrderDatabase {
    #[allow(dead_code)]
    orders: Arc<Mutex<HashMap<String, Value>>>,
    inventory: Arc<Mutex<HashMap<String, (u32, f64)>>>, // product_id -> (quantity, price)
}

impl MockOrderDatabase {
    pub fn new() -> Self {
        Self {
            orders: Arc::new(Mutex::new(HashMap::new())),
            inventory: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn set_inventory(&self, product_id: &str, quantity: u32, price: f64) {
        self.inventory.lock().unwrap().insert(product_id.to_string(), (quantity, price));
    }

    pub fn get_inventory(&self, product_id: &str) -> Option<(u32, f64)> {
        self.inventory.lock().unwrap().get(product_id).copied()
    }

    #[allow(dead_code)]
    pub fn reduce_inventory(&self, product_id: &str, quantity: u32) -> Result<(), String> {
        let mut inventory = self.inventory.lock().unwrap();
        if let Some((current_qty, _price)) = inventory.get_mut(product_id) {
            if *current_qty >= quantity {
                *current_qty -= quantity;
                Ok(())
            } else {
                Err(format!("Insufficient inventory for product {}", product_id))
            }
        } else {
            Err(format!("Product not found: {}", product_id))
        }
    }

    #[allow(dead_code)]
    pub fn insert_order(&self, order_id: &str, order_data: Value) {
        self.orders.lock().unwrap().insert(order_id.to_string(), order_data);
    }

    #[allow(dead_code)]
    pub fn get_order(&self, order_id: &str) -> Option<Value> {
        self.orders.lock().unwrap().get(order_id).cloned()
    }

    #[allow(dead_code)]
    pub fn get_all_orders(&self) -> HashMap<String, Value> {
        self.orders.lock().unwrap().clone()
    }
}

/// Configuration for payment service behaviors
#[derive(Default, Clone, Debug)]
pub struct PaymentServiceConfig {
    pub decline_next: bool,
    pub timeout_next: bool,
}

/// World struct that holds state across step definitions
#[derive(World)]
#[world(init = Self::default)]
pub struct CascadeWorld {
    // Test server and related components
    pub test_server: Option<TestServerHandles>,
    
    // HTTP request/response data
    pub last_response: Option<Response>,
    pub response_body: Option<Value>,
    pub response_status: Option<StatusCode>,
    pub response: Option<Response>,
    pub error: Option<String>,
    
    // Flow execution data
    pub flow_id: Option<String>,
    pub instance_id: Option<String>,
    
    // Test state
    pub db: MockOrderDatabase,
    pub payment_config: PaymentServiceConfig,
    pub customer_id: Option<String>,
    pub error_details: Option<String>,
    pub custom_state: HashMap<String, Value>,
    pub config: TestConfig,
    #[allow(dead_code)]
    pub server: Option<TestServer>,
    #[allow(dead_code)]
    pub client: Option<TestClient>,
    pub server_addr: Option<SocketAddr>,
    pub order: Option<Value>,
    pub order_id: Option<String>,
    pub order_status: Option<String>,
}

impl CascadeWorld {
    /// Helper method to get a typed context object from custom_state
    #[allow(dead_code)]
    pub fn get_context<T>(&self, key: &str) -> Option<T> 
    where 
        T: serde::de::DeserializeOwned
    {
        self.custom_state.get(key)
            .and_then(|value| serde_json::from_value::<T>(value.clone()).ok())
    }

    /// Check if a feature flag is enabled
    pub fn is_feature_enabled(&self, feature_name: &str) -> bool {
        self.custom_state.get("feature_flags")
            .and_then(|flags| flags.get(feature_name))
            .and_then(|flag| flag.as_bool())
            .unwrap_or(false)
    }

    /// Set a feature flag
    pub fn set_feature_flag(&mut self, feature_name: &str, enabled: bool) {
        let mut feature_flags = self.custom_state
            .get("feature_flags")
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        
        feature_flags.insert(feature_name.to_string(), Value::Bool(enabled));
        self.custom_state.insert("feature_flags".to_string(), Value::Object(feature_flags));
    }
}

impl Default for CascadeWorld {
    fn default() -> Self {
        Self {
            test_server: None,
            last_response: None,
            response_body: None,
            response_status: None,
            response: None,
            error: None,
            flow_id: None,
            instance_id: None,
            db: MockOrderDatabase::new(),
            payment_config: PaymentServiceConfig::default(),
            customer_id: None,
            error_details: None,
            custom_state: HashMap::new(),
            config: TestConfig::default(),
            server: None,
            client: None,
            server_addr: None,
            order: None,
            order_id: None,
            order_status: None,
        }
    }
}

// Manual Debug implementation since TestServer and TestClient don't implement Debug
impl std::fmt::Debug for CascadeWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CascadeWorld")
            .field("test_server", &self.test_server)
            .field("last_response", &self.last_response)
            .field("response_body", &self.response_body)
            .field("response_status", &self.response_status)
            .field("response", &self.response)
            .field("error", &self.error)
            .field("flow_id", &self.flow_id)
            .field("instance_id", &self.instance_id)
            .field("db", &self.db)
            .field("payment_config", &self.payment_config)
            .field("customer_id", &self.customer_id)
            .field("error_details", &self.error_details)
            .field("custom_state", &self.custom_state)
            .field("config", &self.config)
            .field("server", &"TestServer{...}".to_string())
            .field("client", &"TestClient{...}".to_string())
            .field("server_addr", &self.server_addr)
            .field("order", &self.order)
            .field("order_id", &self.order_id)
            .field("order_status", &self.order_status)
            .finish()
    }
} 
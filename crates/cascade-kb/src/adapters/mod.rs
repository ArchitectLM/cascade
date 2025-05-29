//! Adapters implementation for external services

pub mod neo4j_store;
pub mod remote_client;

// Re-export adapters for easier import
pub use neo4j_store::Neo4jStateStore;
pub use remote_client::{RemoteKbClient, RemoteKbClientConfig, create_remote_kb_client}; 
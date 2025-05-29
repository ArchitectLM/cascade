//! Core services for the Cascade Graph Knowledge Base

pub mod ingestion;
pub mod messages;
pub mod query;
pub mod client;

// Re-exports
pub use ingestion::service::IngestionService;
pub use query::service::QueryService;
pub use messages::{
    IngestionMessage, QueryRequest, QueryResponse, QueryResultSender,
};
pub use client::KbClient; 
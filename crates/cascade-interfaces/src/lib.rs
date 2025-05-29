//! Cascade Interfaces
//!
//! This crate provides interface types for integration between different
//! Cascade components, particularly for the Knowledge Base and Agent systems.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Knowledge Base interfaces
pub mod kb;

/// Re-export key types for convenient usage
pub use kb::{
    GraphElementDetails, RetrievedContext, ValidationErrorDetail,
    KnowledgeError, KnowledgeResult, KnowledgeBaseClient,
}; 
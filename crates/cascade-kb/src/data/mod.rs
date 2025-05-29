//! Core data structures for the Cascade Graph Knowledge Base

pub mod types;
pub mod identifiers;
pub mod trace_context;
pub mod entities;
pub mod errors;

// Re-export all common types
pub use types::{DataPacket, Scope, EntityRef, EntityMetadata, Entity, SourceType, VersionStatus, SchemaRef, DataReference};
pub use identifiers::TenantId;
pub use trace_context::TraceContext;
pub use errors::{CoreError, StateStoreError, ComponentLoaderError, ComponentApiError};
pub use entities::*;

// Note: No duplicate type definitions in this module, only re-exports 
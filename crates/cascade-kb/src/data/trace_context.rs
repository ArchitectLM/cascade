//! Trace context for request/operation tracking

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Trace context for request tracking
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_id: Option<String>,
}

impl TraceContext {
    /// Creates a new root trace context with a new trace_id and span_id.
    pub fn new_root() -> Self {
        Self {
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            parent_id: None,
        }
    }

    /// Creates a new child trace context, inheriting the trace_id but with a new span_id.
    pub fn new_child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: Uuid::new_v4().to_string(),
            parent_id: Some(self.span_id.clone()),
        }
    }

    // Adding compatibility method for tests expecting TraceContext::new()
    #[cfg(test)]
    pub fn new() -> Self {
        Self::new_root()
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new_root()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_root() {
        let root = TraceContext::new_root();
        assert_ne!(root.trace_id, Uuid::nil().to_string());
        assert_ne!(root.span_id, Uuid::nil().to_string());
        assert_ne!(root.trace_id, root.span_id);
    }

    #[test]
    fn test_new_child() {
        let root = TraceContext::new_root();
        let child = root.new_child();
        
        // Child should inherit trace_id but have new span_id
        assert_eq!(root.trace_id, child.trace_id);
        assert_ne!(root.span_id, child.span_id);
    }

    #[test]
    fn test_serialization() {
        let root = TraceContext::new_root();
        let serialized = serde_json::to_string(&root).unwrap();
        let deserialized: TraceContext = serde_json::from_str(&serialized).unwrap();
        assert_eq!(root, deserialized);
    }
} 
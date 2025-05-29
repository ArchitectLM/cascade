//! Utility traits and functions.

/// Extension trait to allow downcasting trait objects to concrete types.
pub trait AsAny {
    /// Get a reference to self as Any.
    fn as_any(&self) -> &dyn std::any::Any;
}

impl<T: std::any::Any> AsAny for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
} 
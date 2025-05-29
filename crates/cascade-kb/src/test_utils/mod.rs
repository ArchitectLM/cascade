// Re-export the fake store
pub mod fake_store;
pub use fake_store::FakeStateStore;

// Any existing exports and modules
pub mod fakes;

#[cfg(feature = "mocks")]
pub mod mocks;

// Re-export commonly used test utilities
pub use fakes::*;
pub use mocks::*; 
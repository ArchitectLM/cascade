// This is a meta-package for organizing the core test structure
// of the Cascade project. 

// Re-export tests that should be accessible
// to other test packages or tools
#[cfg(feature = "bdd")]
pub use cascade_bdd_tests;

#[cfg(feature = "e2e")]
pub use cascade_e2e_tests;

#[cfg(feature = "integrations")]
pub use cascade_integration_tests;

// Cascade Tests
//
// This is a meta-package that organizes the test structure
// It doesn't contain actual test code, just the organization of tests

/// Re-export test modules for easier access
pub mod tests {
    // These will be enabled as each test package is completed
    #[cfg(feature = "bdd")]
    pub use cascade_bdd_tests as bdd;
    
    #[cfg(feature = "e2e")]
    pub use cascade_e2e_tests as e2e;
    
    #[cfg(feature = "integrations")]
    pub use cascade_integration_tests as integrations;
}

// This package exists primarily for organizational purposes
#[cfg(test)]
mod test {
    #[test]
    fn it_works() {
        assert!(true, "Meta-package test passes");
    }
} 
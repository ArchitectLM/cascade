//! # Cascade DSL
//! 
//! The Cascade DSL is a YAML-based domain-specific language for defining reactive
//! workflows in the Cascade Platform. This crate provides functionality for parsing, 
//! validating, and representing Cascade DSL documents.
//!
//! ## Features
//!
//! * YAML-based DSL for defining workflows
//! * Component-based architecture with reusable steps
//! * Comprehensive validation of references and dependencies
//! * Schema validation for component inputs and outputs
//! * Circular dependency detection to prevent infinite loops
//!
//! ## Example
//!
//! ```
//! use cascade_dsl::parse_and_validate_flow_definition;
//!
//! // Define a simple flow
//! let yaml = r#"
//! dsl_version: "1.0"
//! definitions:
//!   components:
//!     - name: logger
//!       type: StdLib:Logger
//!       inputs:
//!         - name: message
//!           schema_ref: "schema:string"
//!       outputs:
//!         - name: success
//!           schema_ref: "schema:void"
//!   flows:
//!     - name: hello-world
//!       trigger:
//!         type: HttpEndpoint
//!         config:
//!           path: "/hello"
//!       steps:
//!         - step_id: log-hello
//!           component_ref: logger
//!           inputs_map:
//!             message: "trigger.body"
//! "#;
//!
//! let result = parse_and_validate_flow_definition(yaml);
//! assert!(result.is_ok());
//! ```

mod error;
mod parser;
mod types;
mod utils;

pub mod flow;
pub mod validation;

pub use error::DslError;
pub use flow::{
    ComponentDefinition, ConditionDefinition, FlowDefinition, InputDefinition,
    OutputDefinition, ParsedDocument, StepDefinition, TriggerDefinition
};
pub use validation::ValidationError;

/// Parse and validate a Cascade DSL YAML string.
///
/// This function performs both parsing and validation of a Cascade DSL document:
/// 1. Parses the YAML into structured data
/// 2. Validates the structure and references
/// 3. Returns a fully validated ParsedDocument or detailed errors
///
/// # Arguments
///
/// * `yaml_str` - A YAML string containing a Cascade DSL definition
///
/// # Returns
///
/// A `Result` containing either the parsed and validated `ParsedDocument` or a `DslError`
///
/// # Errors
///
/// This function can fail for several reasons:
/// * Invalid YAML syntax
/// * Unsupported DSL version
/// * Missing required fields
/// * Validation errors (circular dependencies, invalid references, schema errors)
///
/// # Examples
///
/// Basic usage with a valid document:
///
/// ```
/// use cascade_dsl::parse_and_validate_flow_definition;
///
/// let yaml = r#"
/// dsl_version: "1.0"
/// definitions:
///   components:
///     - name: logger
///       type: StdLib:Logger
///       inputs:
///         - name: message
///           schema_ref: "schema:string"
///       outputs:
///         - name: success
///           schema_ref: "schema:void"
///   flows:
///     - name: hello-world
///       trigger:
///         type: HttpEndpoint
///         config:
///           path: "/hello"
///       steps:
///         - step_id: log-hello
///           component_ref: logger
///           inputs_map:
///             message: "trigger.body"
/// "#;
///
/// let result = parse_and_validate_flow_definition(yaml);
/// assert!(result.is_ok());
/// ```
///
/// Handling validation errors:
///
/// ```
/// use cascade_dsl::parse_and_validate_flow_definition;
/// use cascade_dsl::DslError;
///
/// // This document has an invalid component reference
/// let invalid_yaml = r#"
/// dsl_version: "1.0"
/// definitions:
///   components:
///     - name: logger
///       type: StdLib:Logger
///       inputs:
///         - name: message
///           schema_ref: "schema:string"
///       outputs:
///         - name: success
///           schema_ref: "schema:void"
///   flows:
///     - name: hello-world
///       trigger:
///         type: HttpEndpoint
///         config:
///           path: "/hello"
///       steps:
///         - step_id: log-hello
///           component_ref: nonexistent-component  # This component doesn't exist
///           inputs_map:
///             message: "trigger.body"
/// "#;
///
/// let result = parse_and_validate_flow_definition(invalid_yaml);
/// assert!(result.is_err());
///
/// // You can extract detailed information from the error
/// if let Err(error) = result {
///     assert!(error.error_code().contains("INVALID_REFERENCE"));
/// }
/// ```
pub fn parse_and_validate_flow_definition(yaml_str: &str) -> Result<ParsedDocument, DslError> {
    // First parse the YAML string into our document structure
    let document = parser::parse_dsl_document(yaml_str)?;
    
    // Then run all validations
    validation::validate_document(&document)?;
    
    Ok(document)
}

/// Returns a version string for the Cascade DSL crate
///
/// # Returns
///
/// The version of the crate as defined in Cargo.toml
///
/// # Examples
///
/// ```
/// use cascade_dsl::version;
///
/// let ver = version();
/// assert!(ver.starts_with("0."));
/// ```
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_valid_document() {
        // A simple but valid document
        let yaml = r#"
        dsl_version: "1.0"
        definitions:
          components:
            - name: logger
              type: StdLib:Logger
              inputs:
                - name: message
                  schema_ref: "schema:string"
              outputs:
                - name: success
                  schema_ref: "schema:void"
          flows:
            - name: hello-world
              trigger:
                type: HttpEndpoint
                config:
                  path: "/hello"
              steps:
                - step_id: log-hello
                  component_ref: logger
                  inputs_map:
                    message: "trigger.body"
        "#;
        
        let result = parse_and_validate_flow_definition(yaml);
        assert!(result.is_ok(), "Failed to parse valid document: {:?}", result.err());
        
        // Verify some basic properties
        let doc = result.unwrap();
        assert_eq!(doc.dsl_version, "1.0");
        assert_eq!(doc.definitions.components.len(), 1);
        assert_eq!(doc.definitions.flows.len(), 1);
        assert_eq!(doc.definitions.components[0].name, "logger");
        assert_eq!(doc.definitions.flows[0].name, "hello-world");
    }
    
    #[test]
    fn test_invalid_yaml_syntax() {
        // Document with invalid YAML syntax
        let yaml = r#"
        dsl_version: "1.0"
        definitions:
          components: []
          flows: [
            - name: broken-flow  # Incorrect indentation
        "#;
        
        let result = parse_and_validate_flow_definition(yaml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DslError::YamlError(_)));
    }
    
    #[test]
    fn test_unsupported_version() {
        // Document with unsupported version
        let yaml = r#"
        dsl_version: "2.0"  # Only 1.0 is supported
        definitions:
          components: []
          flows: []
        "#;
        
        let result = parse_and_validate_flow_definition(yaml);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            DslError::UnsupportedVersion(version) => {
                assert_eq!(version, "2.0");
            },
            err => panic!("Expected UnsupportedVersion, got {:?}", err),
        }
    }
    
    #[test]
    fn test_invalid_component_reference() {
        // Document with invalid component reference
        let yaml = r#"
        dsl_version: "1.0"
        definitions:
          components:
            - name: logger
              type: StdLib:Logger
              inputs:
                - name: message
                  schema_ref: "schema:string"
              outputs:
                - name: success
                  schema_ref: "schema:void"
          flows:
            - name: invalid-flow
              trigger:
                type: HttpEndpoint
                config:
                  path: "/invalid"
              steps:
                - step_id: log-step
                  component_ref: nonexistent  # This component doesn't exist
                  inputs_map:
                    message: "trigger.body"
        "#;
        
        let result = parse_and_validate_flow_definition(yaml);
        assert!(result.is_err());
        
        let err = result.unwrap_err();
        assert!(err.error_code().contains("INVALID_REFERENCE"));
    }
    
    #[test]
    fn test_circular_dependency() {
        // Document with circular dependency
        let yaml = r#"
        dsl_version: "1.0"
        definitions:
          components:
            - name: logger
              type: StdLib:Logger
              inputs:
                - name: message
                  schema_ref: "schema:string"
              outputs:
                - name: success
                  schema_ref: "schema:void"
          flows:
            - name: circular-flow
              trigger:
                type: HttpEndpoint
                config:
                  path: "/circular"
              steps:
                - step_id: step-a
                  component_ref: logger
                  inputs_map:
                    message: "trigger.body"
                  run_after: [step-c]  # Creates a circular dependency
                - step_id: step-b
                  component_ref: logger
                  inputs_map:
                    message: "trigger.body"
                  run_after: [step-a]
                - step_id: step-c
                  component_ref: logger
                  inputs_map:
                    message: "trigger.body"
                  run_after: [step-b]
        "#;
        
        let result = parse_and_validate_flow_definition(yaml);
        assert!(result.is_err());
        
        let err = result.unwrap_err();
        assert!(err.error_code().contains("CIRCULAR_DEPENDENCY"));
    }
    
    #[test]
    fn test_duplicate_step_ids() {
        // Document with duplicate step IDs
        let yaml = r#"
        dsl_version: "1.0"
        definitions:
          components:
            - name: logger
              type: StdLib:Logger
              inputs:
                - name: message
                  schema_ref: "schema:string"
              outputs:
                - name: success
                  schema_ref: "schema:void"
          flows:
            - name: duplicate-ids-flow
              trigger:
                type: HttpEndpoint
                config:
                  path: "/duplicate"
              steps:
                - step_id: log-step  # Duplicate ID
                  component_ref: logger
                  inputs_map:
                    message: "trigger.body"
                - step_id: log-step  # Duplicate ID
                  component_ref: logger
                  inputs_map:
                    message: "trigger.headers"
        "#;
        
        let result = parse_and_validate_flow_definition(yaml);
        assert!(result.is_err());
        
        let err = result.unwrap_err();
        assert!(err.error_code().contains("DUPLICATE_ID"));
    }
    
    #[test]
    fn test_empty_document() {
        // Minimal valid document
        let yaml = r#"
        dsl_version: "1.0"
        definitions:
          components: []
          flows: []
        "#;
        
        let result = parse_and_validate_flow_definition(yaml);
        assert!(result.is_ok(), "Empty document should be valid");
    }
    
    #[test]
    fn test_version_function() {
        let ver = version();
        assert!(!ver.is_empty(), "Version string should not be empty");
        assert!(ver.contains('.'), "Version string should contain at least one dot");
    }
} 
use crate::error::DslError;
use crate::flow::ParsedDocument;

/// Parse a YAML string into a ParsedDocument.
///
/// This function handles the initial conversion from YAML text to structured data.
/// It does not perform deep validation of the structure or references - that's handled
/// separately by the validation module.
///
/// # Arguments
///
/// * `yaml_str` - A YAML string containing a Cascade DSL definition
///
/// # Returns
///
/// A `Result` containing either the parsed `ParsedDocument` or a `DslError`
pub fn parse_dsl_document(yaml_str: &str) -> Result<ParsedDocument, DslError> {
    // Parse YAML into our document structure
    let document: ParsedDocument = match serde_yaml::from_str(yaml_str) {
        Ok(doc) => doc,
        Err(err) => return Err(DslError::YamlError(err)),
    };
    
    // Verify DSL version is supported
    if document.dsl_version != "1.0" {
        return Err(DslError::UnsupportedVersion(document.dsl_version.clone()));
    }
    
    // Return the parsed document
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_minimal_valid_document() {
        let yaml = r#"
        dsl_version: "1.0"
        definitions:
          components: []
          flows: []
        "#;
        
        let result = parse_dsl_document(yaml);
        assert!(result.is_ok(), "Failed to parse valid document: {:?}", result.err());
        
        let doc = result.unwrap();
        assert_eq!(doc.dsl_version, "1.0");
        assert!(doc.definitions.components.is_empty());
        assert!(doc.definitions.flows.is_empty());
    }
    
    #[test]
    fn test_invalid_yaml_syntax() {
        let yaml = r#"
        dsl_version: "1.0"
        definitions:
          components: []
          flows: [
            - name: broken-flow  # Incorrect indentation
        "#;
        
        let result = parse_dsl_document(yaml);
        assert!(result.is_err());
        
        match result.err().unwrap() {
            DslError::YamlError(_) => {},
            err => panic!("Expected YamlError, got {:?}", err),
        }
    }
    
    #[test]
    fn test_unsupported_dsl_version() {
        let yaml = r#"
        dsl_version: "2.0"
        definitions:
          components: []
          flows: []
        "#;
        
        let result = parse_dsl_document(yaml);
        assert!(result.is_err());
        
        match result.err().unwrap() {
            DslError::UnsupportedVersion(version) => {
                assert_eq!(version, "2.0");
            },
            err => panic!("Expected UnsupportedVersion, got {:?}", err),
        }
    }
} 
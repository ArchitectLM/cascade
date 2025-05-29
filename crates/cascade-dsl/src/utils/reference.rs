use regex::Regex;
use std::collections::HashSet;
use lazy_static::lazy_static;

lazy_static! {
    // Main regex pattern for validating data references
    static ref DATA_REF_REGEX: Regex = Regex::new(
        r"^(trigger(\.([a-zA-Z0-9_\-]+))*|steps\.[a-zA-Z0-9_\-]+\.outputs\.[a-zA-Z0-9_\-]+(\.[a-zA-Z0-9_\-]+)*)$"
    ).unwrap();
    
    // Regex to directly capture step IDs from references
    static ref STEP_ID_CAPTURE_REGEX: Regex = Regex::new(
        r"^steps\.([a-zA-Z0-9_\-]+)\."
    ).unwrap();
    
    // Specialized regex for trigger references
    static ref TRIGGER_REF_REGEX: Regex = Regex::new(
        r"^trigger(\.[a-zA-Z0-9_\-]+)*$"
    ).unwrap();
    
    // Specialized regex for step references
    static ref STEP_REF_REGEX: Regex = Regex::new(
        r"^steps\.[a-zA-Z0-9_\-]+\.outputs\.[a-zA-Z0-9_\-]+(\.[a-zA-Z0-9_\-]+)*$"
    ).unwrap();
}

/// Validate if a data reference has the correct format.
///
/// Valid formats include:
/// - "trigger.body"
/// - "trigger.headers.content_type"
/// - "steps.step1.outputs.result"
/// - "steps.process-data.outputs.transformed.nested.value"
pub fn is_valid_data_reference_format(reference: &str) -> bool {
    DATA_REF_REGEX.is_match(reference)
}

/// Extract step IDs from a data reference
///
/// This extracts step IDs from patterns like:
/// - "steps.step1.outputs.result" -> "step1"
/// - "steps.process-data.outputs.transformed" -> "process-data"
#[allow(dead_code)]
pub fn extract_step_ids_from_reference(reference: &str) -> HashSet<String> {
    let mut step_ids = HashSet::new();
    
    // Use the regex with capture groups for more reliable extraction
    if let Some(captures) = STEP_ID_CAPTURE_REGEX.captures(reference) {
        if let Some(step_id) = captures.get(1) {
            step_ids.insert(step_id.as_str().to_string());
        }
    }
    
    step_ids
}

/// Check if the reference points to a trigger output
#[allow(dead_code)]
pub fn is_trigger_reference(reference: &str) -> bool {
    TRIGGER_REF_REGEX.is_match(reference)
}

/// Check if the reference points to a step output
#[allow(dead_code)]
pub fn is_step_reference(reference: &str) -> bool {
    STEP_REF_REGEX.is_match(reference)
}

/// Extract the step ID from a step reference string
///
/// Returns None if the reference is not a valid step reference
pub fn extract_step_id(reference: &str) -> Option<&str> {
    STEP_ID_CAPTURE_REGEX.captures(reference)
        .and_then(|captures| captures.get(1))
        .map(|m| m.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_valid_data_references() {
        assert!(is_valid_data_reference_format("trigger.body"));
        assert!(is_valid_data_reference_format("trigger"));
        assert!(is_valid_data_reference_format("trigger.headers.content_type"));
        assert!(is_valid_data_reference_format("steps.step1.outputs.result"));
        assert!(is_valid_data_reference_format("steps.process-data.outputs.transformed.nested.value"));
    }
    
    #[test]
    fn test_invalid_data_references() {
        assert!(!is_valid_data_reference_format("invalid"));
        assert!(!is_valid_data_reference_format("trigger."));
        assert!(!is_valid_data_reference_format("steps."));
        assert!(!is_valid_data_reference_format("steps.step1"));
        assert!(!is_valid_data_reference_format("steps.step1.output"));
        assert!(!is_valid_data_reference_format("steps.step1.outputs"));
        // Edge cases
        assert!(!is_valid_data_reference_format(""));
        assert!(!is_valid_data_reference_format("steps..outputs.result"));
        assert!(!is_valid_data_reference_format("STEPS.step1.outputs.result")); // case sensitive
        assert!(!is_valid_data_reference_format("steps.step1@invalid.outputs.result")); // invalid chars
    }
    
    #[test]
    fn test_extract_step_ids() {
        let refs = vec![
            "steps.step1.outputs.result",
            "steps.process-data.outputs.transformed",
        ];
        
        let step_ids: HashSet<_> = refs.iter()
            .flat_map(|r| extract_step_ids_from_reference(r))
            .collect();
            
        assert_eq!(step_ids.len(), 2);
        assert!(step_ids.contains("step1"));
        assert!(step_ids.contains("process-data"));
    }
    
    #[test]
    fn test_reference_types() {
        assert!(is_trigger_reference("trigger.body"));
        assert!(is_step_reference("steps.step1.outputs.result"));
        assert!(!is_trigger_reference("steps.step1.outputs.result"));
        assert!(!is_step_reference("trigger.body"));
    }
    
    #[test]
    fn test_extract_step_id() {
        assert_eq!(extract_step_id("steps.step1.outputs.result"), Some("step1"));
        assert_eq!(extract_step_id("steps.complex-id.outputs.data"), Some("complex-id"));
        assert_eq!(extract_step_id("trigger.body"), None);
        assert_eq!(extract_step_id("invalid"), None);
    }
    
    #[test]
    fn test_edge_cases() {
        // Empty string
        assert!(!is_valid_data_reference_format(""));
        
        // Very long references
        let long_ref = format!("steps.step1.outputs.{}", "a".repeat(100));
        assert!(is_valid_data_reference_format(&long_ref));
        
        // Unicode characters (not allowed)
        assert!(!is_valid_data_reference_format("steps.αβγ.outputs.result"));
        
        // Special characters (only - and _ allowed)
        assert!(is_valid_data_reference_format("steps.valid_step-id.outputs.result"));
        assert!(!is_valid_data_reference_format("steps.invalid*id.outputs.result"));
    }
} 
//! Core identifier types for the Cascade Graph Knowledge Base

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::fmt;

/// Tenant identifier type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(pub Uuid);

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TenantId {
    pub fn new_v4() -> Self {
        Self(Uuid::new_v4())
    }

    // Adding compatibility method for tests expecting TenantId::new() or TenantId::new(String)
    #[cfg(test)]
    pub fn new(_tenant_str: Option<&str>) -> Self {
        Self::new_v4()
    }
}

/// Represents a unique identifier for a VersionSet (logical entity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionSetId(pub Uuid);

impl fmt::Display for VersionSetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl VersionSetId {
    pub fn new_v4() -> Self {
        VersionSetId(Uuid::new_v4())
    }
}

/// Represents a unique identifier for a specific Version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionId(pub Uuid);

impl fmt::Display for VersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl VersionId {
    pub fn new_v4() -> Self {
        VersionId(Uuid::new_v4())
    }
}

/// Represents a unique identifier for a specific ComponentDefinition state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComponentDefinitionId(pub Uuid);

impl fmt::Display for ComponentDefinitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ComponentDefinitionId {
    pub fn new_v4() -> Self {
        ComponentDefinitionId(Uuid::new_v4())
    }
}

/// Represents a unique identifier for a specific FlowDefinition state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlowDefinitionId(pub Uuid);

impl fmt::Display for FlowDefinitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FlowDefinitionId {
    pub fn new_v4() -> Self {
        FlowDefinitionId(Uuid::new_v4())
    }
}

/// Represents a unique identifier for a DocumentationChunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentationChunkId(pub Uuid);

impl fmt::Display for DocumentationChunkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl DocumentationChunkId {
    pub fn new_v4() -> Self {
        DocumentationChunkId(Uuid::new_v4())
    }
}

/// Represents a unique identifier for a CodeExample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CodeExampleId(pub Uuid);

impl fmt::Display for CodeExampleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl CodeExampleId {
    pub fn new_v4() -> Self {
        CodeExampleId(Uuid::new_v4())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_tenant_id_new_v4() {
        let id1 = TenantId::new_v4();
        let id2 = TenantId::new_v4();
        assert_ne!(id1, id2, "Generated UUIDs should be unique");
    }

    #[test]
    fn test_tenant_id_serialization() {
        let id = TenantId::new_v4();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: TenantId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }

    // Similar tests for other ID types...
    #[test]
    fn test_version_set_id_new_v4() {
        let id1 = VersionSetId::new_v4();
        let id2 = VersionSetId::new_v4();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_version_id_new_v4() {
        let id1 = VersionId::new_v4();
        let id2 = VersionId::new_v4();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_component_definition_id_new_v4() {
        let id1 = ComponentDefinitionId::new_v4();
        let id2 = ComponentDefinitionId::new_v4();
        assert_ne!(id1, id2);
    }
} 
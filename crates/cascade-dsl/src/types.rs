/// Represents a string that contains a component type identifier (e.g., "StdLib:HttpCall")
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentTypeIdentifier(pub String);

impl From<String> for ComponentTypeIdentifier {
    fn from(s: String) -> Self {
        ComponentTypeIdentifier(s)
    }
}

impl From<&str> for ComponentTypeIdentifier {
    fn from(s: &str) -> Self {
        ComponentTypeIdentifier(s.to_string())
    }
}

impl AsRef<str> for ComponentTypeIdentifier {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Represents a string that contains a schema reference (e.g., "schema:string")
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SchemaTypeIdentifier(pub String);

impl From<String> for SchemaTypeIdentifier {
    fn from(s: String) -> Self {
        SchemaTypeIdentifier(s)
    }
}

impl From<&str> for SchemaTypeIdentifier {
    fn from(s: &str) -> Self {
        SchemaTypeIdentifier(s.to_string())
    }
}

impl AsRef<str> for SchemaTypeIdentifier {
    fn as_ref(&self) -> &str {
        &self.0
    }
} 
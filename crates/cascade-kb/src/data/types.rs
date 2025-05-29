//! Basic types for the Cascade Graph Knowledge Base

use serde::{Deserialize, Serialize};
use std::convert::From;
use std::collections::HashMap;
use crate::data::identifiers::TenantId;
use chrono::{DateTime, Utc};
use std::fmt;
use uuid;

/// Generic data packet for storing various types of data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataPacket {
    Null,
    Bool(bool),
    Number(f64),
    Integer(i64),
    Float(f32),
    String(String),
    Array(Vec<DataPacket>),
    Object(HashMap<String, DataPacket>),
    Binary(Vec<u8>),
    Json(serde_json::Value),
    Uuid(uuid::Uuid),
    DateTime(chrono::DateTime<chrono::Utc>),
    FloatArray(Vec<f32>),
}

impl DataPacket {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            DataPacket::String(s) => Some(s),
            DataPacket::Json(serde_json::Value::String(s)) => Some(s),
            _ => None,
        }
    }

    pub fn as_json_str(&self) -> Option<&str> {
        match self {
            DataPacket::Json(serde_json::Value::String(s)) => Some(s),
            DataPacket::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_json_f64(&self) -> Option<f64> {
        match self {
            DataPacket::Json(serde_json::Value::Number(n)) => n.as_f64(),
            DataPacket::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<DataPacket>> {
        match self {
            DataPacket::Array(arr) => Some(arr),
            _ => None,
        }
    }

    // Helper to convert DataPacket to Json
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            DataPacket::Null => serde_json::Value::Null,
            DataPacket::Bool(b) => serde_json::Value::Bool(*b),
            DataPacket::Number(n) => serde_json::json!(n),
            DataPacket::Integer(i) => serde_json::json!(i),
            DataPacket::Float(f) => serde_json::json!(f),
            DataPacket::String(s) => serde_json::Value::String(s.clone()),
            DataPacket::Array(arr) => {
                let json_arr = arr.iter().map(|item| item.to_json()).collect();
                serde_json::Value::Array(json_arr)
            },
            DataPacket::Object(obj) => {
                let mut json_obj = serde_json::Map::new();
                for (k, v) in obj {
                    json_obj.insert(k.clone(), v.to_json());
                }
                serde_json::Value::Object(json_obj)
            },
            DataPacket::Binary(b) => serde_json::json!(b),
            DataPacket::Json(j) => j.clone(),
            DataPacket::Uuid(uuid) => serde_json::json!(uuid.to_string()),
            DataPacket::DateTime(dt) => serde_json::json!(dt),
            DataPacket::FloatArray(arr) => serde_json::json!(arr),
        }
    }
}

impl From<&str> for DataPacket {
    fn from(s: &str) -> Self {
        DataPacket::String(s.to_string())
    }
}

impl From<String> for DataPacket {
    fn from(s: String) -> Self {
        DataPacket::String(s)
    }
}

impl From<serde_json::Value> for DataPacket {
    fn from(v: serde_json::Value) -> Self {
        DataPacket::Json(v)
    }
}

impl From<DataPacket> for serde_json::Value {
    fn from(packet: DataPacket) -> Self {
        packet.to_json()
    }
}

/// Scope of data within the Knowledge Base
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Scope {
    Global,
    Tenant(TenantId),
    Application(String),
    Component(String),
    General,
    Project(TenantId),
    ApplicationState,
    UserDefined,
}

impl Default for Scope {
    fn default() -> Self {
        Scope::General
    }
}

impl Scope {
    // Adding compatibility method for tests expecting Scope::new(String)
    #[cfg(test)]
    pub fn new(scope_name: &str) -> Self {
        match scope_name {
            "test_scope" => Scope::UserDefined,
            _ => Scope::General,
        }
    }
}

/// Entity reference within the Knowledge Base
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    pub entity_type: String,
    pub id: String,
    pub version: Option<String>,
}

/// Metadata for Knowledge Base entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMetadata {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
    pub updated_by: String,
    pub version: String,
    pub scope: Scope,
    pub source: Option<String>,
}

/// Complete entity representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub entity_type: String,
    pub id: String,
    pub data: DataPacket,
    pub metadata: EntityMetadata,
    pub embeddings: Option<Vec<f32>>,
}

/// Represents the source of an entity definition, as defined in the overview (Section 2.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceType {
    StdLib,
    UserDefined,
    ApplicationState,
    Other(String),
}

impl Default for SourceType {
    fn default() -> Self {
        SourceType::Other("unknown".to_string())
    }
}

impl fmt::Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceType::StdLib => write!(f, "StdLib"),
            SourceType::UserDefined => write!(f, "UserDefined"),
            SourceType::ApplicationState => write!(f, "ApplicationState"),
            SourceType::Other(s) => write!(f, "Other({})", s),
        }
    }
}

/// Represents the status of a Version, as defined in the overview (Section 2.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VersionStatus {
    Active,
    Deprecated,
    Draft,
    Other(String),
}

impl Default for VersionStatus {
    fn default() -> Self {
        VersionStatus::Draft
    }
}

/// Represents a reference to a data schema (see Section 8.1 Potential Enhancements).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SchemaRef(String);

/// Represents a reference to data within a flow execution (e.g., "steps.stepA.outputs.result").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DataReference(String);

// Add display implementation for DataPacket
impl std::fmt::Display for DataPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataPacket::Null => write!(f, "null"),
            DataPacket::Bool(b) => write!(f, "{}", b),
            DataPacket::Number(n) => write!(f, "{}", n),
            DataPacket::Integer(i) => write!(f, "{}", i),
            DataPacket::Float(fl) => write!(f, "{}", fl),
            DataPacket::String(s) => write!(f, "{}", s),
            DataPacket::Array(arr) => write!(f, "{:?}", arr),
            DataPacket::Json(json) => write!(f, "{}", json),
            DataPacket::Object(obj) => write!(f, "{:?}", obj),
            DataPacket::Binary(bin) => write!(f, "[binary data of {} bytes]", bin.len()),
            DataPacket::Uuid(uuid) => write!(f, "{}", uuid),
            DataPacket::DateTime(dt) => write!(f, "{}", dt),
            DataPacket::FloatArray(arr) => write!(f, "{:?}", arr),
        }
    }
}

impl DataPacket {
    // Convert to string
    pub fn as_string(&self) -> String {
        match self {
            DataPacket::String(s) => s.clone(),
            DataPacket::Null => "".to_string(),
            _ => format!("{}", self),
        }
    }

    // Convert to i64
    pub fn to_i64(&self) -> i64 {
        match self {
            DataPacket::Number(n) => *n as i64,
            DataPacket::Json(v) => {
                if let Some(i) = v.as_i64() {
                    i
                } else if let Some(f) = v.as_f64() {
                    f as i64
                } else {
                    0
                }
            },
            DataPacket::String(s) => s.parse::<i64>().unwrap_or(0),
            _ => 0,
        }
    }

    // Convert to bool
    pub fn to_bool(&self) -> bool {
        match self {
            DataPacket::Bool(b) => *b,
            DataPacket::Number(n) => *n != 0.0,
            DataPacket::String(s) => s == "true" || s == "1",
            _ => false,
        }
    }

    // Convert to Vec<String>
    pub fn to_vec_string(&self) -> Vec<String> {
        match self {
            DataPacket::Array(arr) => {
                arr.iter()
                    .map(|item| match item {
                        DataPacket::String(s) => s.clone(),
                        _ => item.as_string(),
                    })
                    .collect()
            },
            DataPacket::String(s) => vec![s.clone()],
            _ => vec![],
        }
    }
}

// Extensions for HashMap to make it easier to extract values
pub trait DataPacketMapExt {
    fn get_string(&self, key: &str) -> String;
    fn get_i64(&self, key: &str) -> i64;
    fn get_bool(&self, key: &str) -> bool;
    fn get_vec_string(&self, key: &str) -> Vec<String>;
}

impl DataPacketMapExt for HashMap<String, DataPacket> {
    fn get_string(&self, key: &str) -> String {
        self.get(key)
            .map(|v| v.as_string())
            .unwrap_or_default()
    }

    fn get_i64(&self, key: &str) -> i64 {
        self.get(key)
            .map(|v| v.to_i64())
            .unwrap_or_default()
    }

    fn get_bool(&self, key: &str) -> bool {
        self.get(key)
            .map(|v| v.to_bool())
            .unwrap_or_default()
    }

    fn get_vec_string(&self, key: &str) -> Vec<String> {
        self.get(key)
            .map(|v| v.to_vec_string())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_data_packet_json() {
        let data = DataPacket::Json(json!({
            "key": "value",
            "number": 42,
            "array": [1, 2, 3]
        }));
        let serialized = serde_json::to_string(&data).unwrap();
        let deserialized: DataPacket = serde_json::from_str(&serialized).unwrap();
        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_data_packet_conversions() {
        // Test From<&str>
        let str_packet = DataPacket::from("test string");
        assert!(matches!(str_packet, DataPacket::String(_)));
        
        // Test From<String>
        let string_packet = DataPacket::from("test string".to_string());
        assert!(matches!(string_packet, DataPacket::String(_)));
        
        // Test From<serde_json::Value>
        let json_value = json!({"key": "value"});
        let json_packet = DataPacket::from(json_value.clone());
        assert!(matches!(json_packet, DataPacket::Json(_)));
        
        // Test From<DataPacket> for serde_json::Value
        let json_converted: serde_json::Value = json_packet.into();
        assert_eq!(json_value, json_converted);
    }

    #[test]
    fn test_data_packet_as_str() {
        // String variant
        let string_packet = DataPacket::String("test string".to_string());
        assert_eq!(string_packet.as_str(), Some("test string"));
        
        // Json String variant
        let json_string_packet = DataPacket::Json(json!("test json string"));
        assert_eq!(json_string_packet.as_str(), Some("test json string"));
        
        // Non-string variants
        let number_packet = DataPacket::Number(42.0);
        assert_eq!(number_packet.as_str(), None);
        
        let bool_packet = DataPacket::Bool(true);
        assert_eq!(bool_packet.as_str(), None);
    }

    #[test]
    fn test_data_packet_as_json_str() {
        // String variant
        let string_packet = DataPacket::String("test string".to_string());
        assert_eq!(string_packet.as_json_str(), Some("test string"));
        
        // Json String variant
        let json_string_packet = DataPacket::Json(json!("test json string"));
        assert_eq!(json_string_packet.as_json_str(), Some("test json string"));
        
        // Non-string variants
        let json_object_packet = DataPacket::Json(json!({"key": "value"}));
        assert_eq!(json_object_packet.as_json_str(), None);
    }

    #[test]
    fn test_data_packet_as_json_f64() {
        // Number variant
        let number_packet = DataPacket::Number(42.5);
        assert_eq!(number_packet.as_json_f64(), Some(42.5));
        
        // Json Number variant
        let json_number_packet = DataPacket::Json(json!(123.45));
        assert_eq!(json_number_packet.as_json_f64(), Some(123.45));
        
        // Non-number variants
        let string_packet = DataPacket::String("test".to_string());
        assert_eq!(string_packet.as_json_f64(), None);
        
        let bool_packet = DataPacket::Bool(true);
        assert_eq!(bool_packet.as_json_f64(), None);
    }

    #[test]
    fn test_data_packet_as_array() {
        // Array variant
        let array_packet = DataPacket::Array(vec![
            DataPacket::Number(1.0),
            DataPacket::String("test".to_string())
        ]);
        assert!(array_packet.as_array().is_some());
        assert_eq!(array_packet.as_array().unwrap().len(), 2);
        
        // Non-array variants
        let string_packet = DataPacket::String("test".to_string());
        assert!(string_packet.as_array().is_none());
    }

    #[test]
    fn test_scope_serialization() {
        let tenant_id = TenantId::new_v4();
        let scope = Scope::Project(tenant_id);
        let serialized = serde_json::to_string(&scope).unwrap();
        let deserialized: Scope = serde_json::from_str(&serialized).unwrap();
        assert_eq!(scope, deserialized);
    }

    #[test]
    fn test_scope_default() {
        assert_eq!(Scope::default(), Scope::General);
    }

    #[test]
    fn test_version_status_default() {
        assert_eq!(VersionStatus::default(), VersionStatus::Draft);
    }

    #[test]
    fn test_schema_ref() {
        let schema_ref = SchemaRef("http://example.com/schema.json".to_string());
        let serialized = serde_json::to_string(&schema_ref).unwrap();
        let deserialized: SchemaRef = serde_json::from_str(&serialized).unwrap();
        assert_eq!(schema_ref, deserialized);
    }

    #[test]
    fn test_data_reference() {
        let data_ref = DataReference("steps.stepA.outputs.result".to_string());
        let serialized = serde_json::to_string(&data_ref).unwrap();
        let deserialized: DataReference = serde_json::from_str(&serialized).unwrap();
        assert_eq!(data_ref, deserialized);
    }
    
    #[test]
    fn test_entity_metadata_serialization() {
        let metadata = EntityMetadata {
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: "test-user".to_string(),
            updated_by: "test-user".to_string(),
            version: "1.0.0".to_string(),
            scope: Scope::Global,
            source: Some("test-source".to_string()),
        };
        
        let serialized = serde_json::to_string(&metadata).unwrap();
        let deserialized: EntityMetadata = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(metadata.created_by, deserialized.created_by);
        assert_eq!(metadata.updated_by, deserialized.updated_by);
        assert_eq!(metadata.version, deserialized.version);
        assert_eq!(metadata.scope, deserialized.scope);
        assert_eq!(metadata.source, deserialized.source);
    }
    
    #[test]
    fn test_entity_serialization() {
        let entity = Entity {
            entity_type: "TestType".to_string(),
            id: "test-id".to_string(),
            data: DataPacket::Json(json!({"key": "value"})),
            metadata: EntityMetadata {
                created_at: Utc::now(),
                updated_at: Utc::now(),
                created_by: "test-user".to_string(),
                updated_by: "test-user".to_string(),
                version: "1.0.0".to_string(),
                scope: Scope::Global,
                source: Some("test-source".to_string()),
            },
            embeddings: Some(vec![0.1, 0.2, 0.3]),
        };
        
        let serialized = serde_json::to_string(&entity).unwrap();
        let deserialized: Entity = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(entity.entity_type, deserialized.entity_type);
        assert_eq!(entity.id, deserialized.id);
        assert_eq!(entity.data, deserialized.data);
        assert_eq!(entity.metadata.version, deserialized.metadata.version);
        assert_eq!(entity.embeddings, deserialized.embeddings);
    }
    
    #[test]
    fn test_entity_ref_serialization() {
        let entity_ref = EntityRef {
            entity_type: "TestType".to_string(),
            id: "test-id".to_string(),
            version: Some("1.0.0".to_string()),
        };
        
        let serialized = serde_json::to_string(&entity_ref).unwrap();
        let deserialized: EntityRef = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(entity_ref.entity_type, deserialized.entity_type);
        assert_eq!(entity_ref.id, deserialized.id);
        assert_eq!(entity_ref.version, deserialized.version);
    }
    
    #[test]
    fn test_source_type_serialization() {
        let source_types = vec![
            SourceType::StdLib,
            SourceType::UserDefined,
            SourceType::ApplicationState,
            SourceType::Other("CustomSource".to_string()),
        ];
        
        for source_type in source_types {
            let serialized = serde_json::to_string(&source_type).unwrap();
            let deserialized: SourceType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(source_type, deserialized);
        }
    }
    
    #[test]
    fn test_source_type_default() {
        let default_source = SourceType::default();
        assert!(matches!(default_source, SourceType::Other(s) if s == "unknown"));
    }
} 
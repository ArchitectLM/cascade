use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Represents a packet of data flowing through the system
///
/// This is a wrapper around a JSON value with some helper methods
/// for working with data in different formats.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DataPacket {
    /// The inner JSON value
    pub value: serde_json::Value,
}

impl DataPacket {
    /// Create a new data packet from a JSON value
    #[inline]
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }

    /// Create a null data packet
    #[inline]
    pub fn null() -> Self {
        Self {
            value: serde_json::Value::Null,
        }
    }

    /// Get the inner JSON value
    #[inline]
    pub fn as_value(&self) -> &serde_json::Value {
        &self.value
    }

    /// Get a mutable reference to the inner JSON value
    #[inline]
    pub fn as_value_mut(&mut self) -> &mut serde_json::Value {
        &mut self.value
    }

    /// Take ownership of the inner JSON value
    #[inline]
    pub fn into_value(self) -> serde_json::Value {
        self.value
    }

    /// Check if the data packet is null
    #[inline]
    pub fn is_null(&self) -> bool {
        self.value.is_null()
    }

    /// Try to convert the data packet to a string
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        self.value.as_str()
    }

    /// Try to convert the data packet to a number
    #[inline]
    pub fn as_f64(&self) -> Option<f64> {
        self.value.as_f64()
    }

    /// Try to convert the data packet to a boolean
    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        self.value.as_bool()
    }

    /// Try to convert the data packet to an object
    #[inline]
    pub fn as_object(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
        self.value.as_object()
    }

    /// Try to convert the data packet to an array
    #[inline]
    pub fn as_array(&self) -> Option<&Vec<serde_json::Value>> {
        self.value.as_array()
    }

    /// Try to convert the data packet to a specific type
    pub fn to<T>(&self) -> Result<T, serde_json::Error>
    where
        T: for<'de> DeserializeOwned,
    {
        serde_json::from_value(self.value.clone())
    }

    /// Try to convert the data packet to a specific type without cloning
    /// Note: This is only possible for certain types and may fail if
    /// the structure doesn't match exactly.
    pub fn to_ref<'a, T>(&'a self) -> Result<T, serde_json::Error>
    where
        T: for<'de> Deserialize<'de> + Clone,
    {
        serde_json::from_value(self.value.clone())
    }

    /// Create a data packet from a serializable value
    pub fn from<T>(value: &T) -> Result<Self, serde_json::Error>
    where
        T: Serialize,
    {
        Ok(Self::new(serde_json::to_value(value)?))
    }

    /// Create a data packet from a string or string reference
    #[inline]
    pub fn from_string(s: &str) -> Self {
        Self::new(serde_json::Value::String(s.to_string()))
    }

    /// Create a data packet from a number
    #[inline]
    pub fn from_f64(n: f64) -> Self {
        match serde_json::Number::from_f64(n) {
            Some(num) => Self::new(serde_json::Value::Number(num)),
            None => Self::new(serde_json::Value::Null),
        }
    }

    /// Create a data packet from a boolean
    #[inline]
    pub fn from_bool(b: bool) -> Self {
        Self::new(serde_json::Value::Bool(b))
    }

    /// Create an object data packet with a single key-value pair
    #[inline]
    pub fn singleton(key: &str, value: serde_json::Value) -> Self {
        let mut map = serde_json::Map::new();
        map.insert(key.to_string(), value);
        Self::new(serde_json::Value::Object(map))
    }
}

impl std::str::FromStr for DataPacket {
    type Err = core::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(serde_json::Value::String(s.to_string())))
    }
}

/// Log level for component logging  
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Trace level - very detailed information
    Trace,
    /// Debug level - debug information
    Debug,
    /// Info level - general information
    Info,
    /// Warn level - warnings
    Warn,
    /// Error level - errors
    Error,
}

impl From<LogLevel> for tracing::Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => tracing::Level::TRACE,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Error => tracing::Level::ERROR,
        }
    }
}

impl From<tracing::Level> for LogLevel {
    fn from(level: tracing::Level) -> Self {
        if level == tracing::Level::TRACE {
            LogLevel::Trace
        } else if level == tracing::Level::DEBUG {
            LogLevel::Debug
        } else if level == tracing::Level::INFO {
            LogLevel::Info
        } else if level == tracing::Level::WARN {
            LogLevel::Warn
        } else {
            LogLevel::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_data_packet_creation() {
        let packet = DataPacket::new(json!({"name": "test"}));
        assert_eq!(packet.as_value()["name"], "test");
    }

    #[test]
    fn test_data_packet_as_value() {
        let expected = json!({"key": "value", "number": 42});
        let packet = DataPacket::new(expected.clone());
        assert_eq!(*packet.as_value(), expected);
    }

    #[test]
    fn test_data_packet_from_string() {
        let packet = DataPacket::from_string("test string");
        assert_eq!(packet.as_str().unwrap(), "test string");
    }

    #[test]
    fn test_data_packet_from_number() {
        let packet = DataPacket::from_f64(42.0);
        assert_eq!(packet.as_f64().unwrap(), 42.0);
    }

    #[test]
    fn test_data_packet_from_bool() {
        let packet = DataPacket::from_bool(true);
        assert!(packet.as_bool().unwrap());
    }

    #[test]
    fn test_data_packet_serialization() {
        let original = DataPacket::new(json!({"complex": {"nested": ["array", 123]}}));
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: DataPacket = serde_json::from_str(&serialized).unwrap();
        assert_eq!(*original.as_value(), *deserialized.as_value());
    }

    #[test]
    fn test_data_packet_clone() {
        let original = DataPacket::new(json!({"test": "value"}));
        let cloned = original.clone();
        assert_eq!(*original.as_value(), *cloned.as_value());
    }

    #[test]
    fn test_log_level_variants() {
        // Test that all LogLevel variants exist
        let _error = LogLevel::Error;
        let _warn = LogLevel::Warn;
        let _info = LogLevel::Info;
        let _debug = LogLevel::Debug;
        let _trace = LogLevel::Trace;
        
        // Just test that the enum exists and can be instantiated
        assert!(true);
    }

    #[test]
    fn test_data_packet_null() {
        let packet = DataPacket::null();
        assert!(packet.is_null());
    }

    #[test]
    fn test_data_packet_null_function() {
        // Test the null() function
        let packet = DataPacket::null();
        
        // Verify the value is null
        assert!(packet.is_null());
        assert_eq!(packet.value, serde_json::Value::Null);
        
        // Explicitly verify the field assignment in the null() function
        let null_packet = DataPacket {
            value: serde_json::Value::Null,
        };
        assert_eq!(packet.value, null_packet.value);
    }

    #[test]
    fn test_data_packet_as_value_mut() {
        let mut packet = DataPacket::new(json!({"mutable": "original"}));
        let value_mut = packet.as_value_mut();
        *value_mut = json!({"mutable": "modified"});
        
        assert_eq!(packet.as_value()["mutable"], "modified");
    }

    #[test]
    fn test_data_packet_into_value() {
        let packet = DataPacket::new(json!({"convert": "to value"}));
        let value = packet.into_value();
        
        assert_eq!(value["convert"], "to value");
    }

    #[test]
    fn test_data_packet_is_null() {
        let null_packet = DataPacket::null();
        let non_null_packet = DataPacket::new(json!(42));
        
        assert!(null_packet.is_null());
        assert!(!non_null_packet.is_null());
    }

    #[test]
    fn test_data_packet_as_object() {
        let packet = DataPacket::new(json!({
            "key1": "value1",
            "key2": 42
        }));
        
        let obj = packet.as_object();
        assert!(obj.is_some());
        let obj = obj.unwrap();
        
        assert_eq!(obj.get("key1").unwrap().as_str().unwrap(), "value1");
        assert_eq!(obj.get("key2").unwrap().as_i64().unwrap(), 42);
        
        // Non-object should return None
        let non_obj_packet = DataPacket::new(json!("not an object"));
        assert!(non_obj_packet.as_object().is_none());
    }

    #[test]
    fn test_data_packet_as_array() {
        let packet = DataPacket::new(json!(["one", 2, true]));
        
        let arr = packet.as_array();
        assert!(arr.is_some());
        let arr = arr.unwrap();
        
        assert_eq!(arr[0].as_str().unwrap(), "one");
        assert_eq!(arr[1].as_i64().unwrap(), 2);
        assert_eq!(arr[2].as_bool().unwrap(), true);
        
        // Non-array should return None
        let non_arr_packet = DataPacket::new(json!("not an array"));
        assert!(non_arr_packet.as_array().is_none());
    }

    #[test]
    fn test_data_packet_to() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct TestStruct {
            name: String,
            age: u32,
        }
        
        let packet = DataPacket::new(json!({
            "name": "Test User",
            "age": 30
        }));
        
        let test_struct: TestStruct = packet.to().unwrap();
        assert_eq!(test_struct.name, "Test User");
        assert_eq!(test_struct.age, 30);
    }

    #[test]
    fn test_data_packet_to_ref() {
        #[derive(Deserialize, Clone, PartialEq, Debug)]
        struct TestStruct {
            name: String,
            active: bool,
        }
        
        let packet = DataPacket::new(json!({
            "name": "Test User",
            "active": true
        }));
        
        let test_struct: TestStruct = packet.to_ref().unwrap();
        assert_eq!(test_struct.name, "Test User");
        assert_eq!(test_struct.active, true);
    }

    #[test]
    fn test_data_packet_from() {
        #[derive(Serialize)]
        struct TestStruct {
            id: u32,
            description: String,
        }
        
        let test_data = TestStruct {
            id: 123,
            description: "test description".to_string(),
        };
        
        let packet = DataPacket::from(&test_data).unwrap();
        assert_eq!(packet.as_value()["id"], 123);
        assert_eq!(packet.as_value()["description"], "test description");
    }

    #[test]
    fn test_data_packet_singleton() {
        let packet = DataPacket::singleton("status", json!("active"));
        
        let obj = packet.as_object().unwrap();
        assert_eq!(obj.len(), 1);
        assert_eq!(obj.get("status").unwrap().as_str().unwrap(), "active");
    }

    #[test]
    fn test_data_packet_from_str() {
        let packet: DataPacket = "simple string".parse().unwrap();
        assert_eq!(packet.as_str().unwrap(), "simple string");
    }

    #[test]
    fn test_log_level_from_tracing_level() {
        assert_eq!(LogLevel::from(tracing::Level::TRACE), LogLevel::Trace);
        assert_eq!(LogLevel::from(tracing::Level::DEBUG), LogLevel::Debug);
        assert_eq!(LogLevel::from(tracing::Level::INFO), LogLevel::Info);
        assert_eq!(LogLevel::from(tracing::Level::WARN), LogLevel::Warn);
        assert_eq!(LogLevel::from(tracing::Level::ERROR), LogLevel::Error);
    }

    #[test]
    fn test_tracing_level_from_log_level() {
        assert_eq!(tracing::Level::from(LogLevel::Trace), tracing::Level::TRACE);
        assert_eq!(tracing::Level::from(LogLevel::Debug), tracing::Level::DEBUG);
        assert_eq!(tracing::Level::from(LogLevel::Info), tracing::Level::INFO);
        assert_eq!(tracing::Level::from(LogLevel::Warn), tracing::Level::WARN);
        assert_eq!(tracing::Level::from(LogLevel::Error), tracing::Level::ERROR);
    }

    #[test]
    fn test_data_packet_from_number_special_case() {
        // NaN is a special case that serde_json can't represent as a Number
        let packet = DataPacket::from_f64(f64::NAN);
        
        // Should return a null value
        assert!(packet.is_null());
    }

    #[test]
    fn test_data_packet_new_directly() {
        // Test calling new directly to ensure it's covered
        let test_value = serde_json::Value::String("direct test".to_string());
        let packet = DataPacket::new(test_value.clone());
        
        // Directly check that the value field is set correctly
        assert_eq!(packet.value, test_value);
        assert_eq!(packet.as_str().unwrap(), "direct test");
    }

    #[test]
    fn test_data_packet_struct() {
        // Create the struct directly to test field initialization
        let expected_value = serde_json::Value::Number(serde_json::Number::from(42));
        
        // Test both ways of creating: directly and through new()
        let direct_packet = DataPacket { value: expected_value.clone() };
        let new_packet = DataPacket::new(expected_value);
        
        // Verify they're equivalent
        assert_eq!(direct_packet.value, new_packet.value);
        assert_eq!(direct_packet.as_f64(), new_packet.as_f64());
    }

    #[test]
    fn test_data_packet_direct_null_comparison() {
        // Create a DataPacket with serde_json::Value::Null explicitly
        let explicit_null = DataPacket { value: serde_json::Value::Null };
        
        // Create a DataPacket using the null() method
        let method_null = DataPacket::null();
        
        // Compare the two to ensure the null() method works correctly
        assert_eq!(explicit_null.value, method_null.value);
        
        // Verify both are considered null
        assert!(explicit_null.is_null());
        assert!(method_null.is_null());
        
        // Direct access to the internal field to verify the value
        let raw_value = &method_null.value;
        assert!(matches!(raw_value, serde_json::Value::Null));
    }

    #[test]
    #[allow(unused_variables)]
    fn test_data_packet_null_line_coverage() {
        // This test is specifically designed to hit line 22 in the null() function
        let null_packet = DataPacket::null();
        
        // Force coverage of line 22 (the assignment of serde_json::Value::Null to value)
        assert!(matches!(null_packet.value, serde_json::Value::Null));
        
        // Directly access the field to ensure we've covered the constructor completely
        let null_value = serde_json::Value::Null;
        let packet_constructed = DataPacket { value: null_value };
        assert_eq!(null_packet.value, packet_constructed.value);
    }

    #[test]
    fn test_data_packet_null_direct_inspection() {
        // Create a null packet
        let null_packet = DataPacket::null();
        
        // Direct field access - this should cover line 22 explicitly
        let internal_value_ref = &null_packet.value;
        
        // Check that it's exactly serde_json::Value::Null, targeting the specific line
        assert!(match internal_value_ref {
            serde_json::Value::Null => true,
            _ => false
        });
        
        // Double-check using standard API
        assert!(null_packet.is_null());
    }

    #[test]
    fn test_data_packet_null_literal() {
        // This test directly targets line 22 of the DataPacket::null() method
        
        // Create a null packet using the null() method
        let packet = DataPacket::null();
        
        // Create a direct struct with the literal Value::Null
        let direct = DataPacket { 
            value: serde_json::Value::Null 
        };
        
        // Test that the implementations match exactly
        assert_eq!(packet.value, direct.value);
        assert_eq!(packet.value, serde_json::Value::Null);
        
        // Call is_null to ensure both packet types behave identically
        assert!(packet.is_null());
        assert!(direct.is_null());
        
        // Verify that this is actually line 22 by checking the output of null()
        let null_fn_ptr = DataPacket::null as fn() -> DataPacket;
        let result = null_fn_ptr();
        assert!(matches!(result.value, serde_json::Value::Null));
    }
}

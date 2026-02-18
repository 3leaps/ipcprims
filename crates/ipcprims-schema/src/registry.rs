use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use ipcprims_frame::{Frame, COMMAND, CONTROL, DATA, ERROR, TELEMETRY};
use jsonschema::Validator;
use serde_json::{Map, Value};

use crate::config::RegistryConfig;
use crate::error::{Result, SchemaError};
use crate::validator::validate_payload;

/// Channel-keyed registry of compiled JSON Schema validators.
pub struct SchemaRegistry {
    validators: HashMap<u16, Validator>,
    config: RegistryConfig,
}

impl SchemaRegistry {
    /// Create an empty registry with default config.
    pub fn new() -> Self {
        Self::with_config(RegistryConfig::default())
    }

    /// Create an empty registry with explicit config.
    pub fn with_config(config: RegistryConfig) -> Self {
        Self {
            validators: HashMap::new(),
            config,
        }
    }

    /// Register a schema for a channel from a JSON string.
    pub fn register(&mut self, channel: u16, schema_json: &str) -> Result<()> {
        let schema: Value = serde_json::from_str(schema_json)?;
        self.register_value(channel, &schema)
    }

    /// Register a schema for a channel from JSON value.
    pub fn register_value(&mut self, channel: u16, schema: &Value) -> Result<()> {
        let mut schema_to_compile = schema.clone();
        if self.config.strict_mode {
            apply_strict_mode(&mut schema_to_compile);
        }

        let compiled = jsonschema::validator_for(&schema_to_compile)
            .map_err(|err| SchemaError::CompileFailed(err.to_string()))?;

        self.validators.insert(channel, compiled);
        Ok(())
    }

    /// Load schemas from a directory.
    pub fn from_directory(path: &Path) -> Result<Self> {
        Self::from_directory_with_config(path, RegistryConfig::default())
    }

    /// Load schemas from a directory with explicit config.
    pub fn from_directory_with_config(path: &Path, config: RegistryConfig) -> Result<Self> {
        let mut registry = Self::with_config(config);
        let mut loaded_schema_count = 0usize;

        let entries = std::fs::read_dir(path)
            .map_err(|err| SchemaError::LoadFailed(format!("{}: {err}", path.display())))?;

        for entry in entries {
            let entry = entry.map_err(|err| SchemaError::LoadFailed(err.to_string()))?;
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            let is_schema_file = file_name.ends_with(".schema.json");
            let entry_path = entry.path();
            let path_metadata = std::fs::symlink_metadata(&entry_path)
                .map_err(|err| SchemaError::LoadFailed(err.to_string()))?;
            let file_type = path_metadata.file_type();

            if file_type.is_symlink() {
                if is_schema_file {
                    return Err(SchemaError::LoadFailed(format!(
                        "refusing to load schema symlink: {file_name}"
                    )));
                }
                continue;
            }
            if !file_type.is_file() {
                continue;
            }

            let channel = match resolve_channel_from_file_name(&file_name) {
                Some(channel) => channel,
                None => {
                    if is_schema_file {
                        return Err(SchemaError::LoadFailed(format!(
                            "unrecognized schema filename: {file_name}"
                        )));
                    }
                    continue;
                }
            };

            loaded_schema_count = loaded_schema_count.saturating_add(1);
            if loaded_schema_count > registry.config.max_schemas_from_directory {
                return Err(SchemaError::LoadFailed(format!(
                    "schema count exceeds configured max ({}): {}",
                    registry.config.max_schemas_from_directory, loaded_schema_count
                )));
            }

            let file = std::fs::File::open(&entry_path).map_err(|err| {
                SchemaError::LoadFailed(format!(
                    "failed opening schema {}: {err}",
                    entry_path.display()
                ))
            })?;
            let opened_metadata = file
                .metadata()
                .map_err(|err| SchemaError::LoadFailed(err.to_string()))?;

            #[cfg(unix)]
            {
                if !same_file_identity(&path_metadata, &opened_metadata) {
                    return Err(SchemaError::LoadFailed(format!(
                        "schema file changed during load: {file_name}"
                    )));
                }
            }

            if opened_metadata.len() > registry.config.max_schema_file_size as u64 {
                return Err(SchemaError::LoadFailed(format!(
                    "schema file too large ({} bytes): {file_name}",
                    opened_metadata.len()
                )));
            }

            let max_bytes = registry.config.max_schema_file_size;
            let read_limit = u64::try_from(max_bytes.saturating_add(1)).unwrap_or(u64::MAX);
            let mut content = String::new();
            file.take(read_limit)
                .read_to_string(&mut content)
                .map_err(|err| {
                    SchemaError::LoadFailed(format!(
                        "failed reading schema {}: {err}",
                        entry_path.display()
                    ))
                })?;
            if content.len() > max_bytes {
                return Err(SchemaError::LoadFailed(format!(
                    "schema file too large while reading: {file_name}"
                )));
            }

            registry.register(channel, &content)?;
        }

        Ok(registry)
    }

    /// Load from embedded schema strings.
    pub fn from_embedded(schemas: &[(u16, &str)]) -> Result<Self> {
        let mut registry = Self::new();
        for (channel, schema) in schemas {
            registry.register(*channel, schema)?;
        }
        Ok(registry)
    }

    /// Validate channel payload against its schema.
    pub fn validate(&self, channel: u16, payload: &[u8]) -> Result<()> {
        match self.validators.get(&channel) {
            Some(validator) => validate_payload(channel, payload, validator),
            None if self.config.fail_on_missing_schema => Err(SchemaError::NoSchema(channel)),
            None => Ok(()),
        }
    }

    /// Validate a frame payload against its channel schema.
    pub fn validate_frame(&self, frame: &Frame) -> Result<()> {
        self.validate(frame.channel, frame.payload.as_ref())
    }

    /// Check if a channel has a registered schema.
    pub fn has_schema(&self, channel: u16) -> bool {
        self.validators.contains_key(&channel)
    }

    /// Get channels that have registered schemas.
    pub fn channels(&self) -> Vec<u16> {
        let mut channels: Vec<u16> = self.validators.keys().copied().collect();
        channels.sort_unstable();
        channels
    }

    /// Get registry configuration.
    pub fn config(&self) -> &RegistryConfig {
        &self.config
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn resolve_channel_from_file_name(file_name: &str) -> Option<u16> {
    let lower = file_name.to_ascii_lowercase();

    if let Some(channel) = parse_channel_pattern(&lower) {
        return Some(channel);
    }

    match lower.as_str() {
        "control.schema.json" => Some(CONTROL),
        "command.schema.json" => Some(COMMAND),
        "data.schema.json" => Some(DATA),
        "telemetry.schema.json" => Some(TELEMETRY),
        "error.schema.json" => Some(ERROR),
        _ => None,
    }
}

fn parse_channel_pattern(file_name: &str) -> Option<u16> {
    let suffix = ".schema.json";
    let prefix = "channel_";

    if !file_name.starts_with(prefix) || !file_name.ends_with(suffix) {
        return None;
    }

    let channel_str = &file_name[prefix.len()..file_name.len() - suffix.len()];
    channel_str.parse::<u16>().ok()
}

fn apply_strict_mode(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if is_object_schema(map) && !map.contains_key("additionalProperties") {
                map.insert("additionalProperties".to_string(), Value::Bool(false));
            }

            recurse_object_schema_children(map);
        }
        Value::Array(items) => {
            for item in items {
                apply_strict_mode(item);
            }
        }
        _ => {}
    }
}

fn recurse_object_schema_children(map: &mut Map<String, Value>) {
    recurse_map_schemas(map, "properties");
    recurse_map_schemas(map, "patternProperties");
    recurse_map_schemas(map, "dependentSchemas");
    recurse_map_schemas(map, "$defs");
    recurse_map_schemas(map, "definitions");

    recurse_single_schema(map, "propertyNames");
    recurse_single_schema(map, "additionalProperties");
    recurse_single_schema(map, "unevaluatedProperties");
    recurse_single_schema(map, "items");
    recurse_single_schema(map, "contains");
    recurse_single_schema(map, "additionalItems");
    recurse_single_schema(map, "unevaluatedItems");
    recurse_single_schema(map, "not");
    recurse_single_schema(map, "if");
    recurse_single_schema(map, "then");
    recurse_single_schema(map, "else");

    recurse_array_schemas(map, "prefixItems");
    recurse_array_schemas(map, "allOf");
    recurse_array_schemas(map, "anyOf");
    recurse_array_schemas(map, "oneOf");
}

fn recurse_map_schemas(map: &mut Map<String, Value>, key: &str) {
    if let Some(Value::Object(obj)) = map.get_mut(key) {
        for value in obj.values_mut() {
            apply_strict_mode(value);
        }
    }
}

fn recurse_single_schema(map: &mut Map<String, Value>, key: &str) {
    if let Some(value) = map.get_mut(key) {
        apply_strict_mode(value);
    }
}

fn recurse_array_schemas(map: &mut Map<String, Value>, key: &str) {
    if let Some(Value::Array(items)) = map.get_mut(key) {
        for item in items {
            apply_strict_mode(item);
        }
    }
}

fn is_object_schema(map: &Map<String, Value>) -> bool {
    match map.get("type") {
        Some(Value::String(kind)) => kind == "object",
        Some(Value::Array(items)) => items
            .iter()
            .any(|item| matches!(item, Value::String(kind) if kind == "object")),
        _ => is_object_keyword_schema(map),
    }
}

fn is_object_keyword_schema(map: &Map<String, Value>) -> bool {
    const OBJECT_KEYWORDS: [&str; 8] = [
        "properties",
        "patternProperties",
        "additionalProperties",
        "unevaluatedProperties",
        "required",
        "dependentRequired",
        "dependentSchemas",
        "propertyNames",
    ];

    OBJECT_KEYWORDS
        .iter()
        .any(|keyword| map.contains_key(*keyword))
}

#[cfg(unix)]
fn same_file_identity(
    path_metadata: &std::fs::Metadata,
    opened_metadata: &std::fs::Metadata,
) -> bool {
    use std::os::unix::fs::MetadataExt;
    path_metadata.dev() == opened_metadata.dev() && path_metadata.ino() == opened_metadata.ino()
}

// Windows file identity check deferred to v0.2.0 — volume_serial_number()/file_index()
// require nightly (windows_by_handle). Stable implementation via GetFileInformationByHandle
// will land with named pipe transport.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ipcprims_frame::Frame;

    use super::*;

    const OBJECT_SCHEMA: &str = r#"{
        "type": "object",
        "properties": {
            "id": { "type": "integer" },
            "name": { "type": "string" }
        },
        "required": ["id", "name"]
    }"#;

    #[test]
    fn register_and_validate() {
        let mut registry = SchemaRegistry::new();
        registry.register(1, OBJECT_SCHEMA).unwrap();

        assert!(registry.validate(1, br#"{"id":1,"name":"ok"}"#).is_ok());
        assert!(matches!(
            registry.validate(1, br#"{"id":"bad","name":"ok"}"#),
            Err(SchemaError::ValidationFailed { .. })
        ));
    }

    #[test]
    fn multiple_channels_independent_validation() {
        let mut registry = SchemaRegistry::new();
        registry
            .register(
                1,
                r#"{"type":"object","properties":{"a":{"type":"integer"}},"required":["a"]}"#,
            )
            .unwrap();
        registry
            .register(
                2,
                r#"{"type":"object","properties":{"b":{"type":"string"}},"required":["b"]}"#,
            )
            .unwrap();
        registry
            .register(3, r#"{"type":"array","items":{"type":"boolean"}}"#)
            .unwrap();

        assert!(registry.validate(1, br#"{"a":7}"#).is_ok());
        assert!(registry.validate(2, br#"{"b":"v"}"#).is_ok());
        assert!(registry.validate(3, br#"[true,false]"#).is_ok());

        assert!(registry.validate(1, br#"{"a":"x"}"#).is_err());
        assert!(registry.validate(2, br#"{"b":10}"#).is_err());
        assert!(registry.validate(3, br#"[true,1]"#).is_err());
    }

    #[test]
    fn missing_schema_permissive_passes() {
        let registry = SchemaRegistry::new();
        assert!(registry.validate(99, br#"{"any":"thing"}"#).is_ok());
    }

    #[test]
    fn missing_schema_strict_fails() {
        let registry = SchemaRegistry::with_config(RegistryConfig {
            strict_mode: false,
            fail_on_missing_schema: true,
            ..RegistryConfig::default()
        });

        assert!(matches!(
            registry.validate(99, br#"{}"#),
            Err(SchemaError::NoSchema(99))
        ));
    }

    #[test]
    fn strict_mode_rejects_additional_properties() {
        let mut permissive = SchemaRegistry::new();
        permissive.register(1, OBJECT_SCHEMA).unwrap();

        let mut strict = SchemaRegistry::with_config(RegistryConfig {
            strict_mode: true,
            fail_on_missing_schema: false,
            ..RegistryConfig::default()
        });
        strict.register(1, OBJECT_SCHEMA).unwrap();

        let payload = br#"{"id":1,"name":"ok","extra":true}"#;
        assert!(permissive.validate(1, payload).is_ok());
        assert!(matches!(
            strict.validate(1, payload),
            Err(SchemaError::ValidationFailed { .. })
        ));
    }

    #[test]
    fn invalid_json_payload_fails() {
        let mut registry = SchemaRegistry::new();
        registry.register(1, OBJECT_SCHEMA).unwrap();

        assert!(matches!(
            registry.validate(1, b"not-json"),
            Err(SchemaError::InvalidJson(_))
        ));
    }

    #[test]
    fn invalid_schema_fails_compile() {
        let mut registry = SchemaRegistry::new();
        let invalid = r#"{"type":"definitely-not-a-type"}"#;

        assert!(matches!(
            registry.register(1, invalid),
            Err(SchemaError::CompileFailed(_))
        ));
    }

    #[test]
    fn from_embedded_loads_schemas() {
        let registry = SchemaRegistry::from_embedded(&[
            (1, OBJECT_SCHEMA),
            (
                2,
                r#"{"type":"object","properties":{"x":{"type":"boolean"}},"required":["x"]}"#,
            ),
        ])
        .unwrap();

        assert!(registry.has_schema(1));
        assert!(registry.has_schema(2));
        assert_eq!(registry.channels(), vec![1, 2]);
    }

    #[test]
    fn from_directory_loads_and_validates() {
        let dir = make_temp_schema_dir("from-directory");

        write_schema(&dir, "channel_1.schema.json", OBJECT_SCHEMA);
        write_schema(
            &dir,
            "channel_2.schema.json",
            r#"{"type":"array","items":{"type":"integer"}}"#,
        );

        let registry = SchemaRegistry::from_directory(&dir).unwrap();
        assert!(registry.validate(1, br#"{"id":5,"name":"ok"}"#).is_ok());
        assert!(registry.validate(2, br#"[1,2,3]"#).is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn channel_name_resolution_and_validate_frame() {
        let dir = make_temp_schema_dir("named-files");

        write_schema(&dir, "command.schema.json", OBJECT_SCHEMA);

        let registry = SchemaRegistry::from_directory(&dir).unwrap();
        let frame = Frame::new(COMMAND, br#"{"id":10,"name":"cmd"}"#.to_vec());
        assert!(registry.validate_frame(&frame).is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn make_temp_schema_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ipcprims-schema-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_schema(dir: &Path, file_name: &str, contents: &str) {
        let path = dir.join(file_name);
        std::fs::write(path, contents.as_bytes()).unwrap();
    }

    #[test]
    fn config_access_and_directory_unknown_schema_name_errors() {
        let config = RegistryConfig {
            strict_mode: true,
            fail_on_missing_schema: true,
            max_schemas_from_directory: 256,
            max_schema_file_size: 256 * 1024,
        };
        let registry = SchemaRegistry::with_config(config);
        assert_eq!(registry.config(), &config);

        let dir = make_temp_schema_dir("unknown-name");
        write_schema(&dir, "foo.schema.json", OBJECT_SCHEMA);
        let result = SchemaRegistry::from_directory(&dir);
        assert!(matches!(result, Err(SchemaError::LoadFailed(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn strict_mode_applies_object_keywords_without_type() {
        let schema = r#"{
            "properties": {
                "id": { "type": "integer" }
            },
            "required": ["id"]
        }"#;

        let mut strict = SchemaRegistry::with_config(RegistryConfig {
            strict_mode: true,
            ..RegistryConfig::default()
        });
        strict.register(1, schema).unwrap();

        assert!(strict.validate(1, br#"{"id":1}"#).is_ok());
        assert!(matches!(
            strict.validate(1, br#"{"id":1,"extra":true}"#),
            Err(SchemaError::ValidationFailed { .. })
        ));
    }

    #[test]
    fn strict_mode_applies_nested_objects() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "nested": {
                    "type": "object",
                    "properties": {
                        "v": { "type": "integer" }
                    },
                    "required": ["v"]
                }
            },
            "required": ["nested"]
        }"#;

        let mut strict = SchemaRegistry::with_config(RegistryConfig {
            strict_mode: true,
            fail_on_missing_schema: false,
            ..RegistryConfig::default()
        });
        strict.register(1, schema).unwrap();

        assert!(strict.validate(1, br#"{"nested":{"v":1}}"#).is_ok());
        assert!(matches!(
            strict.validate(1, br#"{"nested":{"v":1,"extra":true}}"#),
            Err(SchemaError::ValidationFailed { .. })
        ));
    }

    #[test]
    fn only_recognized_extensions_are_loaded() {
        let dir = make_temp_schema_dir("extensions");
        write_schema(&dir, "command.schema.json", OBJECT_SCHEMA);
        write_schema(&dir, "ignored.json", OBJECT_SCHEMA);

        let registry = SchemaRegistry::from_directory(&dir).unwrap();
        assert_eq!(registry.channels(), vec![COMMAND]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_schema_is_rejected() {
        let dir = make_temp_schema_dir("symlink-schema");
        let target = dir.join("target.json");
        std::fs::write(&target, OBJECT_SCHEMA.as_bytes()).unwrap();
        let link = dir.join("command.schema.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let result = SchemaRegistry::from_directory(&dir);
        assert!(matches!(result, Err(SchemaError::LoadFailed(_))));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn schema_count_limit_is_enforced() {
        let dir = make_temp_schema_dir("schema-count-limit");
        write_schema(&dir, "channel_1.schema.json", OBJECT_SCHEMA);
        write_schema(&dir, "channel_2.schema.json", OBJECT_SCHEMA);

        let config = RegistryConfig {
            max_schemas_from_directory: 1,
            ..RegistryConfig::default()
        };
        let result = SchemaRegistry::from_directory_with_config(&dir, config);
        assert!(matches!(result, Err(SchemaError::LoadFailed(_))));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn schema_file_size_limit_is_enforced() {
        let dir = make_temp_schema_dir("schema-size-limit");
        write_schema(&dir, "channel_1.schema.json", OBJECT_SCHEMA);

        let config = RegistryConfig {
            max_schema_file_size: 8,
            ..RegistryConfig::default()
        };
        let result = SchemaRegistry::from_directory_with_config(&dir, config);
        assert!(matches!(result, Err(SchemaError::LoadFailed(_))));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parser_recognizes_channel_pattern() {
        assert_eq!(parse_channel_pattern("channel_1.schema.json"), Some(1));
        assert_eq!(
            parse_channel_pattern("channel_65535.schema.json"),
            Some(65535)
        );
        assert_eq!(parse_channel_pattern("channel_70000.schema.json"), None);
        assert_eq!(parse_channel_pattern("channel_x.schema.json"), None);
    }

    #[cfg(unix)]
    #[test]
    fn same_file_identity_distinguishes_replaced_file() {
        let dir = make_temp_schema_dir("identity-check");
        let first = dir.join("first.json");
        let second = dir.join("second.json");
        std::fs::write(&first, OBJECT_SCHEMA).unwrap();
        std::fs::write(&second, OBJECT_SCHEMA).unwrap();

        let first_meta = std::fs::symlink_metadata(&first).unwrap();
        let opened_first_meta = std::fs::File::open(&first).unwrap().metadata().unwrap();
        let opened_second_meta = std::fs::File::open(&second).unwrap().metadata().unwrap();

        assert!(same_file_identity(&first_meta, &opened_first_meta));
        assert!(!same_file_identity(&first_meta, &opened_second_meta));

        let _ = std::fs::remove_dir_all(&dir);
    }
}

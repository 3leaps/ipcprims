/// Controls schema validation behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistryConfig {
    /// When true, schemas reject additional properties not in the schema.
    pub strict_mode: bool,
    /// When true, channels without a schema return `SchemaError::NoSchema`.
    pub fail_on_missing_schema: bool,
    /// Maximum number of schemas loaded from a directory.
    pub max_schemas_from_directory: usize,
    /// Maximum bytes allowed per schema file loaded from a directory.
    pub max_schema_file_size: usize,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            strict_mode: false,
            fail_on_missing_schema: false,
            max_schemas_from_directory: 256,
            max_schema_file_size: 256 * 1024,
        }
    }
}

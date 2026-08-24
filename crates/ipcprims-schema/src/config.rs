/// Controls schema registration, validation, and directory-loading behavior.
///
/// The default is deliberately permissive: unregistered channels validate
/// successfully without parsing their payload. Use both [`Self::strict_mode`]
/// and [`Self::fail_on_missing_schema`] when an application requires the
/// stricter path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistryConfig {
    /// Apply the strict transform while schemas are registered or loaded.
    ///
    /// The transform adds `additionalProperties: false` only to recognized
    /// object-like schema locations that do not already specify that keyword.
    /// An explicit non-object `type` takes precedence over object-keyword
    /// detection. Explicit schema policy is preserved, and unrecognized JSON
    /// Schema constructs are not rewritten.
    pub strict_mode: bool,
    /// Reject channels without a registered schema.
    ///
    /// When false, validation for an unregistered channel succeeds without
    /// parsing the payload as JSON.
    pub fail_on_missing_schema: bool,
    /// Maximum number of recognized schemas loaded from a directory.
    pub max_schemas_from_directory: usize,
    /// Maximum bytes allowed per recognized schema file loaded from a directory.
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

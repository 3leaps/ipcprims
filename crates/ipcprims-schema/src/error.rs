/// Errors that can occur during schema validation.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    /// The schema file could not be loaded.
    #[error("failed to load schema: {0}")]
    LoadFailed(String),

    /// The schema could not be compiled.
    #[error("failed to compile schema: {0}")]
    CompileFailed(String),

    /// The payload failed schema validation.
    #[error("validation failed on channel {channel}: {message}")]
    ValidationFailed { channel: u16, message: String },

    /// The payload is not valid JSON.
    #[error("payload is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    /// No schema registered for the given channel.
    #[error("no schema registered for channel {0}")]
    NoSchema(u16),
}

pub type Result<T> = std::result::Result<T, SchemaError>;

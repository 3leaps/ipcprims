/// Errors that can occur while loading, compiling, or validating schemas.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    /// A schema directory or file violated loading policy or could not be read.
    #[error("failed to load schema: {0}")]
    LoadFailed(String),

    /// A parsed JSON schema could not be compiled.
    #[error("failed to compile schema: {0}")]
    CompileFailed(String),

    /// A payload parsed successfully but failed its channel schema.
    #[error("validation failed on channel {channel}: {message}")]
    ValidationFailed { channel: u16, message: String },

    /// Schema or payload JSON could not be parsed.
    #[error("payload is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    /// No schema is registered for the given channel while missing schemas fail.
    #[error("no schema registered for channel {0}")]
    NoSchema(u16),
}

pub type Result<T> = std::result::Result<T, SchemaError>;

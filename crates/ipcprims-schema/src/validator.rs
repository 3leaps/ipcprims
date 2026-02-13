use jsonschema::Validator;

use crate::error::{Result, SchemaError};

pub(crate) fn validate_payload(channel: u16, payload: &[u8], validator: &Validator) -> Result<()> {
    let value: serde_json::Value = serde_json::from_slice(payload)?;

    let mut errors = validator.iter_errors(&value);
    if let Some(first) = errors.next() {
        let mut message = first.to_string();
        for err in errors.take(3) {
            message.push_str("; ");
            message.push_str(&err.to_string());
        }
        return Err(SchemaError::ValidationFailed { channel, message });
    }

    Ok(())
}

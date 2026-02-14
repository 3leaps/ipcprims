use napi::{Error, Status};

pub(crate) fn to_napi_error(context: &str, err: impl std::fmt::Display) -> Error {
    Error::new(Status::GenericFailure, format!("{context}: {err}"))
}

pub(crate) fn invalid_state(message: &str) -> Error {
    Error::new(Status::InvalidArg, message.to_string())
}

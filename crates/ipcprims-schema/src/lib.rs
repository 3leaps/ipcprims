//! Optional JSON Schema validation at the IPC transport boundary.
//!
//! Validate messages against JSON Schema 2020-12 at the frame level.
//! Catch contract violations before they become bugs.
//!
//! This crate is optional — use it when you want schema-enforced
//! message contracts between peers.

pub mod config;
pub mod error;
pub mod registry;
pub mod validator;

pub use config::RegistryConfig;
pub use error::{Result, SchemaError};
pub use registry::SchemaRegistry;

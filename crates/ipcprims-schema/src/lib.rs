#![doc = include_str!("../../../docs/guides/schema-registry.md")]

pub mod config;
pub mod error;
pub mod registry;
pub mod validator;

pub use config::RegistryConfig;
pub use error::{Result, SchemaError};
pub use registry::SchemaRegistry;

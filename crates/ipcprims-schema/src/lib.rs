//! Optional JSON Schema validation at the IPC transport boundary.
//!
//! Validate messages against JSON Schema 2020-12 at the frame level.
//! Applications that do not attach a registry perform no schema validation.
//!
//! See the [Schema Registry Guide] for configuration, directory loading, and
//! peer integration details.
//!
//! [Schema Registry Guide]: https://github.com/3leaps/ipcprims/blob/main/docs/guides/schema-registry.md

pub mod config;
pub mod error;
pub mod registry;
pub mod validator;

pub use config::RegistryConfig;
pub use error::{Result, SchemaError};
pub use registry::SchemaRegistry;

//! Effect integration for postmortem validation.
//!
//! This module provides Effect-based integration with stillwater for:
//! - Schema loading from filesystem
//! - Async validation with dependency injection
//! - Environment-based configuration
//!
//! # Feature Flag
//!
//! This module is only available when the `effect` feature is enabled.
//!
//! # Example
//!
//! ```rust,ignore
//! use postmortem::{SchemaRegistry, Schema};
//! use postmortem::effect::{SchemaEnv, FileSystem};
//! use stillwater::Effect;
//!
//! // Define your environment
//! struct AppEnv {
//!     fs: MyFileSystem,
//! }
//!
//! impl SchemaEnv for AppEnv {
//!     type Fs = MyFileSystem;
//!     fn filesystem(&self) -> &Self::Fs { &self.fs }
//! }
//!
//! // Load schemas from directory
//! let registry = SchemaRegistry::new();
//! let load_effect = registry.load_dir("./schemas");
//! load_effect.run(&env)?;
//! ```

pub mod async_validator;
pub mod loading;

pub use async_validator::{AsyncStringSchema, AsyncValidator};
pub use loading::{FileSystem, SchemaEnv, SchemaLoadError};

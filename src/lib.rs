//! # Postmortem
//!
//! A validation library that accumulates ALL validation errors, providing
//! comprehensive feedback rather than short-circuiting on the first failure.
//!
//! ## Overview
//!
//! Unlike typical validation libraries that stop at the first error, postmortem
//! collects all validation errors to give users complete information about what
//! needs to be fixed. This is achieved through integration with stillwater's
//! `Validation` type for applicative error accumulation.
//!
//! ## Core Types
//!
//! - [`JsonPath`]: Represents paths to values in nested structures (e.g., `users[0].email`)
//! - [`SchemaError`]: A single validation error with context (path, message, expected/got values)
//! - [`SchemaErrors`]: A non-empty collection of validation errors
//! - [`Schema`]: Entry point for creating validation schemas
//!
//! ## Example
//!
//! ```rust
//! use postmortem::{JsonPath, Schema};
//! use serde_json::json;
//!
//! // Create a string schema with constraints
//! let schema = Schema::string()
//!     .min_len(1)
//!     .max_len(100);
//!
//! // Validate a value
//! let result = schema.validate(&json!("hello"), &JsonPath::root());
//! assert!(result.is_success());
//!
//! // Invalid values produce detailed errors
//! let result = schema.validate(&json!(""), &JsonPath::root());
//! assert!(result.is_failure());
//! ```

pub mod error;
pub mod path;
pub mod registry;
pub mod schema;
pub mod validation;

#[cfg(feature = "effect")]
pub mod effect;

pub use error::{SchemaError, SchemaErrors};
pub use path::{JsonPath, PathSegment};
pub use registry::{RegistryError, SchemaRegistry};
pub use schema::{
    ArraySchema, CombinatorSchema, IntegerSchema, ObjectSchema, RefSchema, Schema, SchemaLike,
    StringSchema, ValueValidator,
};

/// Type alias for validation results using SchemaErrors
pub type ValidationResult<T> = stillwater::Validation<T, SchemaErrors>;

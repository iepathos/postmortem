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
//!
//! ## Example
//!
//! ```rust
//! use postmortem::{JsonPath, SchemaError, SchemaErrors};
//! use stillwater::Validation;
//!
//! // Build a path to a nested value
//! let path = JsonPath::root()
//!     .push_field("users")
//!     .push_index(0)
//!     .push_field("email");
//!
//! assert_eq!(path.to_string(), "users[0].email");
//!
//! // Create a validation error
//! let error = SchemaError::new(path, "invalid email format")
//!     .with_code("invalid_email")
//!     .with_got("not-an-email")
//!     .with_expected("valid email address");
//!
//! // Wrap in SchemaErrors for use with Validation
//! let errors = SchemaErrors::single(error);
//! let result: Validation<String, SchemaErrors> = Validation::Failure(errors);
//! ```

pub mod error;
pub mod path;

pub use error::{SchemaError, SchemaErrors};
pub use path::{JsonPath, PathSegment};

/// Type alias for validation results using SchemaErrors
pub type ValidationResult<T> = stillwater::Validation<T, SchemaErrors>;

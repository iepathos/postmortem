//! Schema definitions for validation.
//!
//! This module provides schema types for validating data structures.
//! Each schema type (string, number, etc.) validates values and accumulates
//! all validation errors rather than short-circuiting on the first failure.
//!
//! # Example
//!
//! ```rust
//! use postmortem::{Schema, JsonPath};
//! use serde_json::json;
//!
//! let schema = Schema::string().min_len(1).max_len(100);
//!
//! let result = schema.validate(&json!("hello"), &JsonPath::root());
//! assert!(result.is_success());
//! ```

mod numeric;
mod string;

pub use numeric::IntegerSchema;
pub use string::StringSchema;

/// Entry point for creating validation schemas.
///
/// `Schema` provides factory methods for creating different schema types.
/// Each schema type validates specific value types and supports various
/// constraints through a builder pattern.
///
/// # Example
///
/// ```rust
/// use postmortem::Schema;
///
/// // Create a string schema with length constraints
/// let string_schema = Schema::string()
///     .min_len(1)
///     .max_len(100);
///
/// // Create a string schema with pattern validation
/// let email_schema = Schema::string()
///     .pattern(r"@")
///     .unwrap()
///     .error("must contain @");
/// ```
pub struct Schema;

impl Schema {
    /// Creates a new string schema.
    ///
    /// The returned schema validates that values are strings. Use builder
    /// methods to add constraints like minimum/maximum length or patterns.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::string().min_len(5);
    ///
    /// let result = schema.validate(&json!("hello"), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!("hi"), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn string() -> StringSchema {
        StringSchema::new()
    }

    /// Creates a new integer schema.
    ///
    /// The returned schema validates that values are integers (not floats).
    /// Use builder methods to add constraints like minimum/maximum value,
    /// range, or sign requirements.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::integer().min(0).max(100);
    ///
    /// let result = schema.validate(&json!(50), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!(-5), &JsonPath::root());
    /// assert!(result.is_failure());
    ///
    /// // Float values are rejected
    /// let result = schema.validate(&json!(1.5), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn integer() -> IntegerSchema {
        IntegerSchema::new()
    }
}

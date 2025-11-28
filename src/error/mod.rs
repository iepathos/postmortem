//! Error types for validation failures.
//!
//! This module provides types for representing validation errors with rich context
//! including paths, messages, and expected/actual values.

mod schema_error;

pub use schema_error::{SchemaError, SchemaErrors};

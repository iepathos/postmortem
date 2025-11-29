//! Interoperability with other schema formats.
//!
//! This module provides bidirectional conversion between postmortem schemas
//! and industry-standard formats like JSON Schema.

pub mod json_schema;

pub use json_schema::ToJsonSchema;

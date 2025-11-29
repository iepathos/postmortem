//! JSON Schema interoperability.
//!
//! This module provides conversion between postmortem schemas and JSON Schema format.
//! JSON Schema is the industry standard for describing JSON data structures, enabling
//! integration with existing tools and documentation systems.

use serde_json::Value;

/// Trait for converting schema types to JSON Schema format.
///
/// Implementers of this trait can be exported as JSON Schema documents
/// compatible with draft 2020-12.
pub trait ToJsonSchema {
    /// Converts this schema to a JSON Schema representation.
    ///
    /// Returns a `serde_json::Value` containing the JSON Schema object.
    /// The schema follows the JSON Schema draft 2020-12 specification.
    fn to_json_schema(&self) -> Value;
}

/// Maps postmortem Format types to JSON Schema format strings.
pub fn format_to_json_schema_format(format_name: &str) -> &str {
    match format_name {
        "Email" => "email",
        "Url" => "uri",
        "Uuid" => "uuid",
        "Date" => "date",
        "DateTime" => "date-time",
        "Ip" => "ipv4", // JSON Schema doesn't have generic ip, default to ipv4
        "Ipv4" => "ipv4",
        "Ipv6" => "ipv6",
        _ => "string",
    }
}

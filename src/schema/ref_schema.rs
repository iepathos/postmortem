//! Schema reference type for registry-based validation.
//!
//! This module provides [`RefSchema`] which represents a reference to a named schema
//! in a registry. References enable schema reuse and recursive structures.

use serde_json::Value;
use stillwater::Validation;

use crate::error::{SchemaError, SchemaErrors};
use crate::path::JsonPath;
use crate::schema::SchemaLike;
use crate::validation::ValidationContext;

/// A schema that references another schema by name.
///
/// RefSchema enables schema reuse and recursive structures by referencing
/// schemas stored in a registry. During validation, the reference is resolved
/// to the actual schema.
///
/// References can only be validated through a registry using `SchemaRegistry::validate()`.
/// Attempting to validate without a registry produces an error.
///
/// # Example
///
/// ```rust
/// use postmortem::{Schema, SchemaRegistry};
/// use serde_json::json;
///
/// let registry = SchemaRegistry::new();
///
/// // Register a base schema
/// registry.register("UserId", Schema::integer().positive()).unwrap();
///
/// // Use a reference in another schema
/// registry.register("User", Schema::object()
///     .field("id", Schema::ref_("UserId"))
///     .field("name", Schema::string())
/// ).unwrap();
///
/// let result = registry.validate("User", &json!({
///     "id": 42,
///     "name": "Alice"
/// })).unwrap();
///
/// assert!(result.is_success());
/// ```
pub struct RefSchema {
    name: String,
}

impl RefSchema {
    /// Creates a new schema reference.
    ///
    /// This is typically called via `Schema::ref_()` rather than directly.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Returns the name of the referenced schema.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl SchemaLike for RefSchema {
    type Output = Value;

    fn validate(&self, _value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        // Cannot validate reference without registry
        Validation::Failure(SchemaErrors::single(
            SchemaError::new(
                path.clone(),
                format!(
                    "reference to '{}' cannot be validated without a registry. \
                     Use SchemaRegistry::validate() instead",
                    self.name
                ),
            )
            .with_code("missing_registry"),
        ))
    }

    fn validate_to_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        self.validate(value, path)
    }

    fn validate_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<Value, SchemaErrors> {
        // Check depth before resolving to prevent infinite loops
        if context.depth() >= context.max_depth() {
            return Validation::Failure(SchemaErrors::single(
                SchemaError::new(
                    path.clone(),
                    format!(
                        "maximum reference depth {} exceeded at path '{}'",
                        context.max_depth(),
                        path
                    ),
                )
                .with_code("max_depth_exceeded"),
            ));
        }

        // Resolve reference from registry
        let schema = match context.registry().get_schema(&self.name) {
            Some(s) => s,
            None => {
                return Validation::Failure(SchemaErrors::single(
                    SchemaError::new(
                        path.clone(),
                        format!("schema '{}' not found in registry", self.name),
                    )
                    .with_code("missing_reference"),
                ))
            }
        };

        // Validate with incremented depth to track reference chain
        schema.validate_value_with_context(value, path, &context.increment_depth())
    }

    fn validate_to_value_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<Value, SchemaErrors> {
        self.validate_with_context(value, path, context)
    }

    fn collect_refs(&self, refs: &mut Vec<String>) {
        refs.push(self.name.clone());
    }
}

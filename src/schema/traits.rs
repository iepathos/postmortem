//! Traits for schema polymorphism.
//!
//! This module provides the [`SchemaLike`] trait that enables different schema types
//! (string, integer, object, etc.) to be composed together for nested validation.

use serde_json::Value;
use stillwater::Validation;

use crate::error::SchemaErrors;
use crate::path::JsonPath;

/// A trait for schema types that can validate JSON values.
///
/// `SchemaLike` enables schema polymorphism, allowing different schema types
/// to be composed together for validating nested structures. Any type that
/// implements this trait can be used as a field schema in an `ObjectSchema`.
///
/// The `Send + Sync` bounds allow schemas to be safely shared across threads
/// and used in trait objects like `Box<dyn SchemaLike>`.
///
/// # Example
///
/// ```rust
/// use postmortem::{Schema, JsonPath};
/// use serde_json::json;
///
/// // Both StringSchema and IntegerSchema implement SchemaLike,
/// // so they can be used as field schemas in an object schema.
/// let object = Schema::object()
///     .field("name", Schema::string().min_len(1))
///     .field("age", Schema::integer().positive());
/// ```
pub trait SchemaLike: Send + Sync {
    /// The output type produced by successful validation.
    type Output;

    /// Validates a value against this schema.
    ///
    /// Returns `Validation::Success` with the validated value on success,
    /// or `Validation::Failure` with accumulated errors on failure.
    fn validate(&self, value: &Value, path: &JsonPath) -> Validation<Self::Output, SchemaErrors>;

    /// Validates a value and returns the result as a `serde_json::Value`.
    ///
    /// This method allows schema types with different output types to be
    /// used uniformly in object schemas where all fields are stored as `Value`.
    fn validate_to_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors>;

    /// Validates a value with registry context for schema reference resolution.
    ///
    /// This method is used when validating with a registry that contains named schemas.
    /// The context provides access to the registry for resolving schema references
    /// and tracks depth to prevent infinite loops in circular references.
    ///
    /// The default implementation ignores the context and delegates to `validate()`,
    /// which preserves backward compatibility for schemas that don't use references.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// // Most schemas work the same with or without context
    /// let schema = Schema::string().min_len(1);
    /// let result = schema.validate(&json!("hello"), &JsonPath::root());
    /// assert!(result.is_success());
    /// ```
    fn validate_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        _context: &crate::validation::ValidationContext,
    ) -> Validation<Self::Output, SchemaErrors> {
        // Default: ignore context, delegate to existing validate()
        self.validate(value, path)
    }

    /// Validates a value with context and returns the result as a `serde_json::Value`.
    ///
    /// This combines `validate_with_context` and `validate_to_value` for use in
    /// container schemas where all fields must return `Value`.
    ///
    /// The default implementation delegates to `validate_to_value()` for backward compatibility.
    fn validate_to_value_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        _context: &crate::validation::ValidationContext,
    ) -> Validation<Value, SchemaErrors> {
        // Default: ignore context, delegate to existing validate_to_value()
        self.validate_to_value(value, path)
    }

    /// Collects all schema reference names used by this schema.
    ///
    /// This method traverses the schema structure and adds the names of all
    /// referenced schemas to the provided vector. It's used by the registry
    /// to validate that all references can be resolved.
    ///
    /// The default implementation does nothing, which is correct for schemas
    /// that don't contain references. Container schemas (Object, Array) and
    /// combinator schemas (AnyOf, AllOf, etc.) override this to traverse
    /// their children.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, SchemaLike};
    ///
    /// // Most schemas have no references
    /// let schema = Schema::string().min_len(1);
    /// let mut refs = Vec::new();
    /// schema.collect_refs(&mut refs);
    /// assert_eq!(refs.len(), 0);
    /// ```
    fn collect_refs(&self, _refs: &mut Vec<String>) {
        // Default: no references to collect
    }
}

/// A type-erased trait for schemas that validate to JSON values.
///
/// `ValueValidator` provides type erasure for schemas with different output types,
/// allowing them to be used together in combinators. Any type that implements
/// `SchemaLike` automatically implements `ValueValidator`.
///
/// This trait is primarily used by schema combinators like `one_of`, `any_of`,
/// and `all_of` which need to work with heterogeneous collections of schemas.
///
/// # Example
///
/// ```rust
/// use postmortem::{Schema, ValueValidator};
/// use serde_json::json;
///
/// // Different schema types can be used as ValueValidators
/// let validators: Vec<Box<dyn ValueValidator>> = vec![
///     Box::new(Schema::string().min_len(1)),
///     Box::new(Schema::integer().positive()),
/// ];
/// ```
pub trait ValueValidator: Send + Sync {
    /// Validates a value and returns the result as a `serde_json::Value`.
    fn validate_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors>;

    /// Validates a value with context and returns the result as a `serde_json::Value`.
    ///
    /// Default implementation delegates to `validate_value()` for backward compatibility.
    fn validate_value_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        _context: &crate::validation::ValidationContext,
    ) -> Validation<Value, SchemaErrors> {
        self.validate_value(value, path)
    }

    /// Collects schema reference names.
    ///
    /// Default implementation does nothing.
    fn collect_refs(&self, _refs: &mut Vec<String>) {
        // Most schemas have no references
    }
}

/// Blanket implementation of `ValueValidator` for all `SchemaLike` types.
///
/// This allows any schema to be used as a `ValueValidator` without additional code.
impl<S: SchemaLike> ValueValidator for S {
    fn validate_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        self.validate_to_value(value, path)
    }

    fn validate_value_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &crate::validation::ValidationContext,
    ) -> Validation<Value, SchemaErrors> {
        self.validate_to_value_with_context(value, path, context)
    }

    fn collect_refs(&self, refs: &mut Vec<String>) {
        SchemaLike::collect_refs(self, refs);
    }
}

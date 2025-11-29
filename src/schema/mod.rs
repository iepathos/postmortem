//! Schema definitions for validation.
//!
//! This module provides schema types for validating data structures.
//! Each schema type (string, number, object, etc.) validates values and accumulates
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

mod array;
mod combinators;
mod numeric;
mod object;
mod ref_schema;
mod string;
mod traits;

pub use array::ArraySchema;
pub use combinators::CombinatorSchema;
pub use numeric::IntegerSchema;
pub use object::ObjectSchema;
pub use ref_schema::RefSchema;
pub use string::StringSchema;
pub use traits::{SchemaLike, ValueValidator};

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

    /// Creates a new object schema.
    ///
    /// The returned schema validates that values are JSON objects. Use builder
    /// methods to define required fields, optional fields, default values, and
    /// control handling of additional properties.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::object()
    ///     .field("name", Schema::string().min_len(1))
    ///     .field("age", Schema::integer().positive())
    ///     .optional("email", Schema::string())
    ///     .default("role", Schema::string(), json!("user"))
    ///     .additional_properties(false);
    ///
    /// let result = schema.validate(&json!({
    ///     "name": "Alice",
    ///     "age": 30
    /// }), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// // Missing required field produces error
    /// let result = schema.validate(&json!({"name": "Bob"}), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn object() -> ObjectSchema {
        ObjectSchema::new()
    }

    /// Creates a new array schema with the given item schema.
    ///
    /// The returned schema validates that values are arrays and that each item
    /// passes validation against the provided item schema. Use builder methods
    /// to add constraints like minimum/maximum length or uniqueness.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// // Array of positive integers
    /// let schema = Schema::array(Schema::integer().positive())
    ///     .min_len(1)
    ///     .max_len(10);
    ///
    /// let result = schema.validate(&json!([1, 2, 3]), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// // Empty array fails min_len constraint
    /// let result = schema.validate(&json!([]), &JsonPath::root());
    /// assert!(result.is_failure());
    ///
    /// // Non-positive integer fails item validation
    /// let result = schema.validate(&json!([1, -2, 3]), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn array<S: SchemaLike>(item_schema: S) -> ArraySchema<S> {
        ArraySchema::new(item_schema)
    }

    /// Creates a one-of combinator schema.
    ///
    /// Exactly one of the provided schemas must match. This is ideal for
    /// discriminated unions where a value must be one of several distinct types.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, ValueValidator, SchemaLike, JsonPath};
    /// use serde_json::json;
    ///
    /// // Shape can be either a circle or rectangle
    /// let shape = Schema::one_of(vec![
    ///     Box::new(Schema::object()
    ///         .field("type", Schema::string())
    ///         .field("radius", Schema::integer().positive())) as Box<dyn ValueValidator>,
    ///     Box::new(Schema::object()
    ///         .field("type", Schema::string())
    ///         .field("width", Schema::integer().positive())
    ///         .field("height", Schema::integer().positive())) as Box<dyn ValueValidator>,
    /// ]);
    ///
    /// let result = shape.validate(&json!({
    ///     "type": "circle",
    ///     "radius": 5
    /// }), &JsonPath::root());
    /// assert!(result.is_success());
    /// ```
    pub fn one_of<I>(schemas: I) -> CombinatorSchema
    where
        I: IntoIterator<Item = Box<dyn ValueValidator>>,
    {
        use crate::schema::combinators::ValidatorFn;
        use std::sync::Arc;
        let validators: Vec<Arc<dyn ValueValidator>> = schemas
            .into_iter()
            .map(|schema| Arc::from(schema) as Arc<dyn ValueValidator>)
            .collect();
        let validator_fns: Vec<ValidatorFn> = validators
            .iter()
            .map(|validator| {
                let v = Arc::clone(validator);
                Arc::new(
                    move |value: &serde_json::Value, path: &crate::path::JsonPath| {
                        v.validate_value(value, path)
                    },
                ) as ValidatorFn
            })
            .collect();
        CombinatorSchema::OneOf {
            schemas: validator_fns,
            validators,
        }
    }

    /// Creates an any-of combinator schema.
    ///
    /// At least one of the provided schemas must match. This is more permissive
    /// than `one_of` and allows multiple matches. Validation short-circuits on
    /// the first match.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, ValueValidator, SchemaLike, JsonPath};
    /// use serde_json::json;
    ///
    /// // ID can be either a string or positive integer
    /// let id = Schema::any_of(vec![
    ///     Box::new(Schema::string().min_len(1)) as Box<dyn ValueValidator>,
    ///     Box::new(Schema::integer().positive()) as Box<dyn ValueValidator>,
    /// ]);
    ///
    /// let result = id.validate(&json!("abc-123"), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = id.validate(&json!(42), &JsonPath::root());
    /// assert!(result.is_success());
    /// ```
    pub fn any_of<I>(schemas: I) -> CombinatorSchema
    where
        I: IntoIterator<Item = Box<dyn ValueValidator>>,
    {
        use crate::schema::combinators::ValidatorFn;
        use std::sync::Arc;
        let validators: Vec<Arc<dyn ValueValidator>> = schemas
            .into_iter()
            .map(|schema| Arc::from(schema) as Arc<dyn ValueValidator>)
            .collect();
        let validator_fns: Vec<ValidatorFn> = validators
            .iter()
            .map(|validator| {
                let v = Arc::clone(validator);
                Arc::new(
                    move |value: &serde_json::Value, path: &crate::path::JsonPath| {
                        v.validate_value(value, path)
                    },
                ) as ValidatorFn
            })
            .collect();
        CombinatorSchema::AnyOf {
            schemas: validator_fns,
            validators,
        }
    }

    /// Creates an all-of combinator schema.
    ///
    /// All of the provided schemas must match. This is useful for schema
    /// composition and intersection, where a value must satisfy multiple
    /// independent constraints.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, ValueValidator, SchemaLike, JsonPath};
    /// use serde_json::json;
    ///
    /// // Entity must have both a name and a timestamp
    /// let named = Schema::object()
    ///     .field("name", Schema::string().min_len(1));
    ///
    /// let timestamped = Schema::object()
    ///     .field("created_at", Schema::string());
    ///
    /// let entity = Schema::all_of(vec![
    ///     Box::new(named) as Box<dyn ValueValidator>,
    ///     Box::new(timestamped) as Box<dyn ValueValidator>,
    /// ]);
    ///
    /// let result = entity.validate(&json!({
    ///     "name": "Alice",
    ///     "created_at": "2025-01-01"
    /// }), &JsonPath::root());
    /// assert!(result.is_success());
    /// ```
    pub fn all_of<I>(schemas: I) -> CombinatorSchema
    where
        I: IntoIterator<Item = Box<dyn ValueValidator>>,
    {
        use crate::schema::combinators::ValidatorFn;
        use std::sync::Arc;
        let validators: Vec<Arc<dyn ValueValidator>> = schemas
            .into_iter()
            .map(|schema| Arc::from(schema) as Arc<dyn ValueValidator>)
            .collect();
        let validator_fns: Vec<ValidatorFn> = validators
            .iter()
            .map(|validator| {
                let v = Arc::clone(validator);
                Arc::new(
                    move |value: &serde_json::Value, path: &crate::path::JsonPath| {
                        v.validate_value(value, path)
                    },
                ) as ValidatorFn
            })
            .collect();
        CombinatorSchema::AllOf {
            schemas: validator_fns,
            validators,
        }
    }

    /// Creates an optional combinator schema.
    ///
    /// The value can be null. Non-null values are validated against the inner schema.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, ValueValidator, SchemaLike, JsonPath};
    /// use serde_json::json;
    ///
    /// let optional_string = Schema::optional(
    ///     Box::new(Schema::string().min_len(1)) as Box<dyn ValueValidator>
    /// );
    ///
    /// // Null is valid
    /// let result = optional_string.validate(&json!(null), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// // Non-null values are validated
    /// let result = optional_string.validate(&json!("hello"), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = optional_string.validate(&json!(""), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn optional(inner: Box<dyn ValueValidator>) -> CombinatorSchema {
        use crate::schema::combinators::ValidatorFn;
        use std::sync::Arc;
        let validator = Arc::from(inner) as Arc<dyn ValueValidator>;
        let validator_fn: ValidatorFn = {
            let v = Arc::clone(&validator);
            Arc::new(
                move |value: &serde_json::Value, path: &crate::path::JsonPath| {
                    v.validate_value(value, path)
                },
            )
        };
        CombinatorSchema::Optional {
            inner: validator_fn,
            validator,
        }
    }

    /// Creates a reference to a named schema.
    ///
    /// Schema references enable reuse and recursive structures. The referenced
    /// schema must be registered in a `SchemaRegistry` before validation.
    ///
    /// References can only be validated through a registry. Attempting to validate
    /// without a registry produces an error with code `missing_registry`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, SchemaRegistry};
    /// use serde_json::json;
    ///
    /// let registry = SchemaRegistry::new();
    ///
    /// // Register base schema
    /// registry.register("UserId", Schema::integer().positive()).unwrap();
    ///
    /// // Use reference in another schema
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
    pub fn ref_(name: impl Into<String>) -> RefSchema {
        RefSchema::new(name)
    }
}

//! Object schema validation.
//!
//! This module provides [`ObjectSchema`] for validating JSON objects with
//! typed fields, optional fields, default values, additional property handling,
//! and cross-field validation.

use indexmap::IndexMap;
use serde_json::{Map, Value};
use std::collections::HashMap;
use stillwater::Validation;

use crate::error::{SchemaError, SchemaErrors};
use crate::path::JsonPath;

use super::traits::SchemaLike;

/// Type alias for cross-field validators.
///
/// A cross-field validator receives the validated object (after field validation)
/// and the current path, returning a validation result.
type CrossFieldValidator =
    Box<dyn Fn(&ValidatedObject, &JsonPath) -> Validation<(), SchemaErrors> + Send + Sync + 'static>;

/// Represents an object that has passed field-level validation.
///
/// All field values have been validated according to their schemas.
/// This type provides safe access to validated field values for cross-field validation.
pub struct ValidatedObject {
    fields: HashMap<String, Value>,
}

impl ValidatedObject {
    /// Get a field value by name. Returns None if field doesn't exist.
    pub fn get(&self, field: &str) -> Option<&Value> {
        self.fields.get(field)
    }

    /// Check if a field exists and is not null.
    ///
    /// Returns `false` for both missing fields and fields explicitly set to `null`.
    pub fn has(&self, field: &str) -> bool {
        self.get(field).is_some_and(|v| !v.is_null())
    }
}

/// Definition of a field within an object schema.
struct FieldDef {
    schema: Box<dyn SchemaLike<Output = Value>>,
    required: bool,
    default: Option<Value>,
}

/// How to handle properties not defined in the schema.
enum AdditionalProperties {
    /// Allow unknown properties (default behavior).
    Allow,
    /// Reject unknown properties.
    Deny,
    /// Validate unknown properties against a schema.
    Validate(Box<dyn SchemaLike<Output = Value>>),
}

/// A schema for validating JSON objects.
///
/// `ObjectSchema` validates that values are objects and optionally applies
/// constraints like required fields, optional fields with defaults, and
/// additional property handling. All field validation errors are accumulated
/// rather than short-circuiting on the first failure.
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
///     .additional_properties(false);
///
/// let result = schema.validate(&json!({
///     "name": "Alice",
///     "age": 30
/// }), &JsonPath::root());
/// assert!(result.is_success());
/// ```
pub struct ObjectSchema {
    fields: IndexMap<String, FieldDef>,
    additional_properties: AdditionalProperties,
    type_error_message: Option<String>,
    cross_field_validators: Vec<CrossFieldValidator>,
    skip_on_field_errors: bool,
}

impl ObjectSchema {
    /// Creates a new object schema with no fields.
    pub fn new() -> Self {
        Self {
            fields: IndexMap::new(),
            additional_properties: AdditionalProperties::Allow,
            type_error_message: None,
            cross_field_validators: Vec::new(),
            skip_on_field_errors: true,
        }
    }

    /// Adds a required field to the schema.
    ///
    /// The field must be present in the input object and its value must
    /// pass validation against the provided schema.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::object()
    ///     .field("name", Schema::string().min_len(1));
    ///
    /// // Missing required field produces error
    /// let result = schema.validate(&json!({}), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn field<S>(mut self, name: impl Into<String>, schema: S) -> Self
    where
        S: SchemaLike + 'static,
    {
        let name = name.into();
        self.fields.insert(
            name,
            FieldDef {
                schema: Box::new(SchemaWrapper(schema)),
                required: true,
                default: None,
            },
        );
        self
    }

    /// Adds an optional field to the schema.
    ///
    /// The field may be absent from the input object. If present, its value
    /// must pass validation against the provided schema.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::object()
    ///     .optional("nickname", Schema::string());
    ///
    /// // Missing optional field is OK
    /// let result = schema.validate(&json!({}), &JsonPath::root());
    /// assert!(result.is_success());
    /// ```
    pub fn optional<S>(mut self, name: impl Into<String>, schema: S) -> Self
    where
        S: SchemaLike + 'static,
    {
        let name = name.into();
        self.fields.insert(
            name,
            FieldDef {
                schema: Box::new(SchemaWrapper(schema)),
                required: false,
                default: None,
            },
        );
        self
    }

    /// Adds an optional field with a default value.
    ///
    /// If the field is absent from the input object, the default value is used.
    /// If present, its value must pass validation against the provided schema.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::object()
    ///     .default("role", Schema::string(), json!("user"));
    ///
    /// let result = schema.validate(&json!({}), &JsonPath::root());
    /// assert!(result.is_success());
    /// // The validated object will include "role": "user"
    /// ```
    pub fn default<S>(mut self, name: impl Into<String>, schema: S, default: Value) -> Self
    where
        S: SchemaLike + 'static,
    {
        let name = name.into();
        self.fields.insert(
            name,
            FieldDef {
                schema: Box::new(SchemaWrapper(schema)),
                required: false,
                default: Some(default),
            },
        );
        self
    }

    /// Configures how unknown properties are handled.
    ///
    /// By default, unknown properties are allowed. Use this method to reject
    /// unknown properties or validate them against a schema.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// // Reject unknown properties
    /// let strict = Schema::object()
    ///     .field("name", Schema::string())
    ///     .additional_properties(false);
    ///
    /// let result = strict.validate(&json!({
    ///     "name": "Alice",
    ///     "unknown": "field"
    /// }), &JsonPath::root());
    /// assert!(result.is_failure());
    ///
    /// // Validate unknown properties against a schema
    /// let validated = Schema::object()
    ///     .field("name", Schema::string())
    ///     .additional_properties(Schema::string());
    /// ```
    pub fn additional_properties<S>(mut self, setting: S) -> Self
    where
        S: Into<AdditionalPropertiesSetting>,
    {
        self.additional_properties = setting.into().0;
        self
    }

    /// Sets a custom error message for type errors.
    ///
    /// This message is used when the input value is not an object.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::object()
    ///     .error("must be a user object");
    ///
    /// let result = schema.validate(&json!("not an object"), &JsonPath::root());
    /// // Error message will be "must be a user object"
    /// ```
    pub fn error(mut self, message: impl Into<String>) -> Self {
        self.type_error_message = Some(message.into());
        self
    }

    /// Adds a custom cross-field validator.
    ///
    /// Cross-field validators run after all field-level validations pass (or fail,
    /// depending on `skip_on_field_errors` configuration). They receive a
    /// `ValidatedObject` containing all validated field values.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    /// use stillwater::Validation;
    ///
    /// let schema = Schema::object()
    ///     .field("quantity", Schema::integer().positive())
    ///     .field("unit_price", Schema::integer().non_negative())
    ///     .field("total", Schema::integer().non_negative())
    ///     .custom(|obj, path| {
    ///         let qty = obj.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);
    ///         let price = obj.get("unit_price").and_then(|v| v.as_i64()).unwrap_or(0);
    ///         let total = obj.get("total").and_then(|v| v.as_i64()).unwrap_or(0);
    ///
    ///         if qty * price != total {
    ///             Validation::Failure(postmortem::SchemaErrors::single(
    ///                 postmortem::SchemaError::new(
    ///                     path.push_field("total"),
    ///                     "total must equal quantity * unit_price"
    ///                 ).with_code("invalid_total")
    ///             ))
    ///         } else {
    ///             Validation::Success(())
    ///         }
    ///     });
    /// ```
    pub fn custom<F>(self, validator: F) -> Self
    where
        F: Fn(&ValidatedObject, &JsonPath) -> Validation<(), SchemaErrors> + Send + Sync + 'static,
    {
        let mut schema = self;
        schema.cross_field_validators.push(Box::new(validator));
        schema
    }

    /// Configure whether to skip cross-field validation if field validation fails.
    ///
    /// Default: `true` (skip cross-field when fields are invalid).
    ///
    /// When `true`, cross-field validators only run if all field-level validations
    /// pass. This is usually the desired behavior since cross-field validators
    /// often assume fields have valid values.
    ///
    /// Set to `false` to always run cross-field validators, even if some field
    /// validations failed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::Schema;
    ///
    /// let schema = Schema::object()
    ///     .field("name", Schema::string())
    ///     .skip_cross_field_on_errors(false);  // Always run cross-field validators
    /// ```
    pub fn skip_cross_field_on_errors(mut self, skip: bool) -> Self {
        self.skip_on_field_errors = skip;
        self
    }

    /// Requires a field when a condition is met.
    ///
    /// If the condition field matches the predicate, the required field must be present.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::Schema;
    /// use serde_json::json;
    ///
    /// let schema = Schema::object()
    ///     .field("method", Schema::string())
    ///     .optional("card_number", Schema::string())
    ///     .require_if("method", |v| v == &json!("card"), "card_number");
    /// ```
    pub fn require_if<P>(
        self,
        condition_field: impl Into<String>,
        predicate: P,
        required_field: impl Into<String>,
    ) -> Self
    where
        P: Fn(&Value) -> bool + Send + Sync + 'static,
    {
        let condition_field = condition_field.into();
        let required_field = required_field.into();

        self.custom(move |obj, path| {
            let condition_value = obj.get(&condition_field);
            let required_value = obj.get(&required_field);

            match (condition_value, required_value) {
                (Some(cv), None) if predicate(cv) => Validation::Failure(SchemaErrors::single(
                    SchemaError::new(
                        path.push_field(&required_field),
                        format!(
                            "'{}' is required when '{}' matches condition",
                            required_field, condition_field
                        ),
                    )
                    .with_code("conditional_required"),
                )),
                _ => Validation::Success(()),
            }
        })
    }

    /// Ensures two fields are mutually exclusive.
    ///
    /// At most one of the two fields can be present (non-null).
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::Schema;
    ///
    /// let schema = Schema::object()
    ///     .optional("email", Schema::string())
    ///     .optional("phone", Schema::string())
    ///     .mutually_exclusive("email", "phone");
    /// ```
    pub fn mutually_exclusive(
        self,
        field1: impl Into<String>,
        field2: impl Into<String>,
    ) -> Self {
        let field1 = field1.into();
        let field2 = field2.into();

        self.custom(move |obj, path| {
            let has_field1 = obj.has(&field1);
            let has_field2 = obj.has(&field2);

            if has_field1 && has_field2 {
                Validation::Failure(SchemaErrors::single(
                    SchemaError::new(
                        path.clone(),
                        format!("'{}' and '{}' are mutually exclusive", field1, field2),
                    )
                    .with_code("mutually_exclusive"),
                ))
            } else {
                Validation::Success(())
            }
        })
    }

    /// Requires at least one of the specified fields to be present.
    ///
    /// At least one field must exist and be non-null.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::Schema;
    ///
    /// let schema = Schema::object()
    ///     .optional("email", Schema::string())
    ///     .optional("phone", Schema::string())
    ///     .at_least_one_of(["email", "phone"]);
    /// ```
    pub fn at_least_one_of<I, S>(self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let fields: Vec<String> = fields.into_iter().map(Into::into).collect();

        self.custom(move |obj, path| {
            let has_any = fields.iter().any(|f| obj.has(f));

            if has_any {
                Validation::Success(())
            } else {
                Validation::Failure(SchemaErrors::single(
                    SchemaError::new(
                        path.clone(),
                        format!("at least one of {:?} is required", fields),
                    )
                    .with_code("at_least_one_required"),
                ))
            }
        })
    }

    /// Ensures two fields have equal values.
    ///
    /// If both fields are present, their values must be equal.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::Schema;
    ///
    /// let schema = Schema::object()
    ///     .field("password", Schema::string())
    ///     .field("confirm_password", Schema::string())
    ///     .equal_fields("password", "confirm_password");
    /// ```
    pub fn equal_fields(self, field1: impl Into<String>, field2: impl Into<String>) -> Self {
        let field1 = field1.into();
        let field2 = field2.into();

        self.custom(move |obj, path| {
            let value1 = obj.get(&field1);
            let value2 = obj.get(&field2);

            match (value1, value2) {
                (Some(v1), Some(v2)) if v1 != v2 => Validation::Failure(SchemaErrors::single(
                    SchemaError::new(
                        path.push_field(&field2),
                        format!("'{}' must match '{}'", field2, field1),
                    )
                    .with_code("fields_not_equal"),
                )),
                _ => Validation::Success(()),
            }
        })
    }

    /// Ensures field1 is less than field2.
    ///
    /// Works for numbers and strings (lexicographic comparison).
    /// Skips validation if fields are missing, null, or have incompatible types.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::Schema;
    ///
    /// let schema = Schema::object()
    ///     .field("start_date", Schema::string())
    ///     .field("end_date", Schema::string())
    ///     .field_less_than("start_date", "end_date");
    /// ```
    pub fn field_less_than(self, field1: impl Into<String>, field2: impl Into<String>) -> Self {
        let field1 = field1.into();
        let field2 = field2.into();

        self.custom(move |obj, path| {
            let value1 = obj.get(&field1);
            let value2 = obj.get(&field2);

            match (value1, value2) {
                (Some(Value::Number(n1)), Some(Value::Number(n2))) => {
                    let Some(f1) = n1.as_f64() else {
                        return Validation::Success(());
                    };
                    let Some(f2) = n2.as_f64() else {
                        return Validation::Success(());
                    };

                    if f1 >= f2 {
                        Validation::Failure(SchemaErrors::single(
                            SchemaError::new(
                                path.push_field(&field1),
                                format!("'{}' must be less than '{}'", field1, field2),
                            )
                            .with_code("field_not_less_than"),
                        ))
                    } else {
                        Validation::Success(())
                    }
                }
                (Some(Value::String(s1)), Some(Value::String(s2))) => {
                    if s1 >= s2 {
                        Validation::Failure(SchemaErrors::single(
                            SchemaError::new(
                                path.push_field(&field1),
                                format!("'{}' must be less than '{}'", field1, field2),
                            )
                            .with_code("field_not_less_than"),
                        ))
                    } else {
                        Validation::Success(())
                    }
                }
                _ => Validation::Success(()),
            }
        })
    }

    /// Ensures field1 is less than or equal to field2.
    ///
    /// Works for numbers and strings (lexicographic comparison).
    /// Skips validation if fields are missing, null, or have incompatible types.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::Schema;
    ///
    /// let schema = Schema::object()
    ///     .field("min", Schema::integer())
    ///     .field("max", Schema::integer())
    ///     .field_less_or_equal("min", "max");
    /// ```
    pub fn field_less_or_equal(
        self,
        field1: impl Into<String>,
        field2: impl Into<String>,
    ) -> Self {
        let field1 = field1.into();
        let field2 = field2.into();

        self.custom(move |obj, path| {
            let value1 = obj.get(&field1);
            let value2 = obj.get(&field2);

            match (value1, value2) {
                (Some(Value::Number(n1)), Some(Value::Number(n2))) => {
                    let Some(f1) = n1.as_f64() else {
                        return Validation::Success(());
                    };
                    let Some(f2) = n2.as_f64() else {
                        return Validation::Success(());
                    };

                    if f1 > f2 {
                        Validation::Failure(SchemaErrors::single(
                            SchemaError::new(
                                path.push_field(&field1),
                                format!("'{}' must be less than or equal to '{}'", field1, field2),
                            )
                            .with_code("field_not_less_or_equal"),
                        ))
                    } else {
                        Validation::Success(())
                    }
                }
                (Some(Value::String(s1)), Some(Value::String(s2))) => {
                    if s1 > s2 {
                        Validation::Failure(SchemaErrors::single(
                            SchemaError::new(
                                path.push_field(&field1),
                                format!("'{}' must be less than or equal to '{}'", field1, field2),
                            )
                            .with_code("field_not_less_or_equal"),
                        ))
                    } else {
                        Validation::Success(())
                    }
                }
                _ => Validation::Success(()),
            }
        })
    }

    /// Validates a value against this schema.
    ///
    /// Returns `Validation::Success` with a `Map<String, Value>` containing
    /// the validated fields if all validations pass, or `Validation::Failure`
    /// with accumulated errors if any validations fail.
    pub fn validate(
        &self,
        value: &Value,
        path: &JsonPath,
    ) -> Validation<Map<String, Value>, SchemaErrors> {
        // Check if it's an object
        let obj = match value.as_object() {
            Some(o) => o,
            None => {
                let message = self
                    .type_error_message
                    .clone()
                    .unwrap_or_else(|| "expected object".to_string());
                return Validation::Failure(SchemaErrors::single(
                    SchemaError::new(path.clone(), message)
                        .with_code("invalid_type")
                        .with_got(value_type_name(value))
                        .with_expected("object"),
                ));
            }
        };

        let mut errors = Vec::new();
        let mut validated = Map::new();

        // Validate defined fields
        for (name, field_def) in &self.fields {
            let field_path = path.push_field(name);

            match obj.get(name) {
                Some(field_value) => {
                    match field_def.schema.validate_to_value(field_value, &field_path) {
                        Validation::Success(v) => {
                            validated.insert(name.clone(), v);
                        }
                        Validation::Failure(e) => {
                            errors.extend(e.into_iter());
                        }
                    }
                }
                None if field_def.required => {
                    errors.push(
                        SchemaError::new(
                            field_path,
                            format!("required field '{}' is missing", name),
                        )
                        .with_code("required")
                        .with_expected("value"),
                    );
                }
                None => {
                    // Optional field - use default if provided
                    if let Some(default) = &field_def.default {
                        validated.insert(name.clone(), default.clone());
                    }
                }
            }
        }

        // Handle additional properties
        for (key, value) in obj {
            if !self.fields.contains_key(key) {
                let field_path = path.push_field(key);
                match &self.additional_properties {
                    AdditionalProperties::Allow => {
                        // Allow and include in output
                        validated.insert(key.clone(), value.clone());
                    }
                    AdditionalProperties::Deny => {
                        errors.push(
                            SchemaError::new(field_path, format!("unknown field '{}'", key))
                                .with_code("additional_property"),
                        );
                    }
                    AdditionalProperties::Validate(schema) => {
                        match schema.validate_to_value(value, &field_path) {
                            Validation::Success(v) => {
                                validated.insert(key.clone(), v);
                            }
                            Validation::Failure(e) => {
                                errors.extend(e.into_iter());
                            }
                        }
                    }
                }
            }
        }

        // Run cross-field validation if configured
        if !self.skip_on_field_errors || errors.is_empty() {
            let validated_obj = ValidatedObject {
                fields: validated
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            };

            for validator in &self.cross_field_validators {
                if let Validation::Failure(e) = validator(&validated_obj, path) {
                    errors.extend(e.into_iter());
                }
            }
        }

        if errors.is_empty() {
            Validation::Success(validated)
        } else {
            Validation::Failure(SchemaErrors::from_vec(errors))
        }
    }
}

impl Default for ObjectSchema {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaLike for ObjectSchema {
    type Output = Map<String, Value>;

    fn validate(&self, value: &Value, path: &JsonPath) -> Validation<Self::Output, SchemaErrors> {
        self.validate(value, path)
    }

    fn validate_to_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        self.validate(value, path).map(Value::Object)
    }
}

/// A wrapper to adapt any `SchemaLike` to produce `Value` output.
///
/// This is necessary because we store field schemas as `Box<dyn SchemaLike<Output = Value>>`
/// but the actual schemas have different output types.
struct SchemaWrapper<S>(S);

impl<S: SchemaLike> SchemaLike for SchemaWrapper<S> {
    type Output = Value;

    fn validate(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        self.0.validate_to_value(value, path)
    }

    fn validate_to_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        self.0.validate_to_value(value, path)
    }
}

/// A type that can be converted into an `AdditionalProperties` setting.
///
/// This allows `additional_properties()` to accept different types:
/// - `bool`: `true` for Allow, `false` for Deny
/// - Any schema type: Validate additional properties against the schema
pub struct AdditionalPropertiesSetting(AdditionalProperties);

impl From<bool> for AdditionalPropertiesSetting {
    fn from(allow: bool) -> Self {
        if allow {
            AdditionalPropertiesSetting(AdditionalProperties::Allow)
        } else {
            AdditionalPropertiesSetting(AdditionalProperties::Deny)
        }
    }
}

impl<S: SchemaLike + 'static> From<S> for AdditionalPropertiesSetting {
    fn from(schema: S) -> Self {
        AdditionalPropertiesSetting(AdditionalProperties::Validate(Box::new(SchemaWrapper(
            schema,
        ))))
    }
}

/// Returns the JSON type name for a value.
fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{IntegerSchema, StringSchema};
    use serde_json::json;

    fn unwrap_success<T, E: std::fmt::Debug>(v: Validation<T, E>) -> T {
        v.into_result().unwrap()
    }

    fn unwrap_failure<T: std::fmt::Debug, E>(v: Validation<T, E>) -> E {
        v.into_result().unwrap_err()
    }

    #[test]
    fn test_empty_object_schema() {
        let schema = ObjectSchema::new();
        let result = schema.validate(&json!({}), &JsonPath::root());
        assert!(result.is_success());
    }

    #[test]
    fn test_object_schema_rejects_non_object() {
        let schema = ObjectSchema::new();

        let result = schema.validate(&json!("not an object"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_type");
        assert_eq!(errors.first().got, Some("string".to_string()));

        let result = schema.validate(&json!(42), &JsonPath::root());
        assert!(result.is_failure());

        let result = schema.validate(&json!(null), &JsonPath::root());
        assert!(result.is_failure());

        let result = schema.validate(&json!([1, 2, 3]), &JsonPath::root());
        assert!(result.is_failure());
    }

    #[test]
    fn test_required_field() {
        let schema = ObjectSchema::new().field("name", StringSchema::new());

        // Present and valid
        let result = schema.validate(&json!({"name": "Alice"}), &JsonPath::root());
        assert!(result.is_success());
        let obj = unwrap_success(result);
        assert_eq!(obj.get("name"), Some(&json!("Alice")));

        // Missing required field
        let result = schema.validate(&json!({}), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "required");
        assert!(errors.first().message.contains("name"));
    }

    #[test]
    fn test_required_field_invalid_value() {
        let schema = ObjectSchema::new().field("age", IntegerSchema::new().positive());

        let result = schema.validate(&json!({"age": -5}), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "positive");
    }

    #[test]
    fn test_optional_field() {
        let schema = ObjectSchema::new().optional("nickname", StringSchema::new());

        // Without optional field
        let result = schema.validate(&json!({}), &JsonPath::root());
        assert!(result.is_success());
        let obj = unwrap_success(result);
        assert!(obj.get("nickname").is_none());

        // With optional field
        let result = schema.validate(&json!({"nickname": "Bob"}), &JsonPath::root());
        assert!(result.is_success());
        let obj = unwrap_success(result);
        assert_eq!(obj.get("nickname"), Some(&json!("Bob")));
    }

    #[test]
    fn test_optional_field_invalid_value() {
        let schema = ObjectSchema::new().optional("age", IntegerSchema::new());

        // Invalid optional field value
        let result = schema.validate(&json!({"age": "not a number"}), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_type");
    }

    #[test]
    fn test_default_field() {
        let schema = ObjectSchema::new().default("role", StringSchema::new(), json!("user"));

        // Without default field - uses default
        let result = schema.validate(&json!({}), &JsonPath::root());
        assert!(result.is_success());
        let obj = unwrap_success(result);
        assert_eq!(obj.get("role"), Some(&json!("user")));

        // With default field - uses provided value
        let result = schema.validate(&json!({"role": "admin"}), &JsonPath::root());
        assert!(result.is_success());
        let obj = unwrap_success(result);
        assert_eq!(obj.get("role"), Some(&json!("admin")));
    }

    #[test]
    fn test_additional_properties_allow() {
        let schema = ObjectSchema::new()
            .field("name", StringSchema::new())
            .additional_properties(true);

        let result = schema.validate(
            &json!({"name": "Alice", "extra": "field"}),
            &JsonPath::root(),
        );
        assert!(result.is_success());
        let obj = unwrap_success(result);
        assert_eq!(obj.get("extra"), Some(&json!("field")));
    }

    #[test]
    fn test_additional_properties_deny() {
        let schema = ObjectSchema::new()
            .field("name", StringSchema::new())
            .additional_properties(false);

        let result = schema.validate(
            &json!({"name": "Alice", "extra": "field"}),
            &JsonPath::root(),
        );
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "additional_property");
        assert!(errors.first().message.contains("extra"));
    }

    #[test]
    fn test_additional_properties_validate() {
        let schema = ObjectSchema::new()
            .field("name", StringSchema::new())
            .additional_properties(IntegerSchema::new());

        // Valid additional property
        let result = schema.validate(&json!({"name": "Alice", "count": 42}), &JsonPath::root());
        assert!(result.is_success());

        // Invalid additional property
        let result = schema.validate(
            &json!({"name": "Alice", "count": "not a number"}),
            &JsonPath::root(),
        );
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_type");
    }

    #[test]
    fn test_multiple_fields() {
        let schema = ObjectSchema::new()
            .field("name", StringSchema::new().min_len(1))
            .field("age", IntegerSchema::new().positive())
            .optional("email", StringSchema::new());

        let result = schema.validate(
            &json!({"name": "Alice", "age": 30, "email": "alice@example.com"}),
            &JsonPath::root(),
        );
        assert!(result.is_success());
    }

    #[test]
    fn test_error_accumulation() {
        let schema = ObjectSchema::new()
            .field("name", StringSchema::new().min_len(5))
            .field("age", IntegerSchema::new().positive());

        // Both fields invalid
        let result = schema.validate(&json!({"name": "AB", "age": -5}), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.len(), 2);
        assert!(errors.with_code("min_length").len() == 1);
        assert!(errors.with_code("positive").len() == 1);
    }

    #[test]
    fn test_error_accumulation_with_missing_fields() {
        let schema = ObjectSchema::new()
            .field("name", StringSchema::new())
            .field("age", IntegerSchema::new());

        // Both fields missing
        let result = schema.validate(&json!({}), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors.with_code("required").len(), 2);
    }

    #[test]
    fn test_path_tracking() {
        let schema = ObjectSchema::new().field("user", StringSchema::new().min_len(5));

        let result = schema.validate(&json!({"user": "AB"}), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().path.to_string(), "user");
    }

    #[test]
    fn test_nested_object() {
        let address_schema = ObjectSchema::new()
            .field("street", StringSchema::new().min_len(1))
            .field("city", StringSchema::new().min_len(1));

        let user_schema = ObjectSchema::new()
            .field("name", StringSchema::new())
            .field("address", address_schema);

        // Valid nested object
        let result = user_schema.validate(
            &json!({
                "name": "Alice",
                "address": {"street": "123 Main St", "city": "NYC"}
            }),
            &JsonPath::root(),
        );
        assert!(result.is_success());

        // Invalid nested object
        let result = user_schema.validate(
            &json!({
                "name": "Alice",
                "address": {"street": "", "city": ""}
            }),
            &JsonPath::root(),
        );
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_deeply_nested_path_tracking() {
        let inner = ObjectSchema::new().field("value", IntegerSchema::new().positive());
        let middle = ObjectSchema::new().field("inner", inner);
        let outer = ObjectSchema::new().field("middle", middle);

        let result = outer.validate(
            &json!({
                "middle": {
                    "inner": {
                        "value": -5
                    }
                }
            }),
            &JsonPath::root(),
        );
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().path.to_string(), "middle.inner.value");
    }

    #[test]
    fn test_custom_type_error_message() {
        let schema = ObjectSchema::new().error("must be a user object");

        let result = schema.validate(&json!("not an object"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().message, "must be a user object");
    }

    #[test]
    fn test_unicode_field_names() {
        let schema = ObjectSchema::new()
            .field("名前", StringSchema::new())
            .field("年齢", IntegerSchema::new());

        let result = schema.validate(&json!({"名前": "太郎", "年齢": 25}), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!({}), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_empty_input_with_required_fields() {
        let schema = ObjectSchema::new()
            .field("a", StringSchema::new())
            .field("b", IntegerSchema::new());

        let result = schema.validate(&json!({}), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_field_order_preserved() {
        let schema = ObjectSchema::new()
            .field("z", StringSchema::new())
            .field("a", StringSchema::new())
            .field("m", StringSchema::new());

        // Errors should be reported in field definition order
        let result = schema.validate(&json!({}), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        let paths: Vec<_> = errors.iter().map(|e| e.path.to_string()).collect();
        assert_eq!(paths, vec!["z", "a", "m"]);
    }

    #[test]
    fn test_schema_like_trait_validate_to_value() {
        let schema = ObjectSchema::new().field("name", StringSchema::new());

        let result = schema.validate_to_value(&json!({"name": "Alice"}), &JsonPath::root());
        assert!(result.is_success());
        match result.into_result().unwrap() {
            Value::Object(obj) => {
                assert_eq!(obj.get("name"), Some(&json!("Alice")));
            }
            _ => panic!("Expected object"),
        }
    }
}

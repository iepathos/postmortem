//! Array schema validation.
//!
//! This module provides [`ArraySchema`] for validating arrays with item schemas,
//! length constraints, and uniqueness requirements.

use serde_json::Value;
use std::collections::HashMap;
use stillwater::Validation;

use crate::error::{SchemaError, SchemaErrors};
use crate::path::JsonPath;

use super::traits::SchemaLike;

/// A constraint applied to array values.
enum ArrayConstraint {
    MinLength {
        min: usize,
        message: Option<String>,
    },
    MaxLength {
        max: usize,
        message: Option<String>,
    },
    Unique {
        message: Option<String>,
    },
    UniqueBy {
        key_fn: Box<dyn Fn(&Value) -> Value + Send + Sync>,
        message: Option<String>,
    },
}

/// A schema for validating array values.
///
/// `ArraySchema` validates that values are arrays, validates each item against
/// an item schema, and applies constraints like length and uniqueness. All
/// validation errors are accumulated rather than short-circuiting on the first failure.
///
/// # Example
///
/// ```rust
/// use postmortem::{Schema, JsonPath};
/// use serde_json::json;
///
/// // Create a schema for an array of strings
/// let schema = Schema::array(Schema::string().min_len(1))
///     .non_empty()
///     .max_len(10);
///
/// // Validate an array
/// let result = schema.validate(&json!(["hello", "world"]), &JsonPath::root());
/// assert!(result.is_success());
///
/// // Empty array fails non_empty constraint
/// let result = schema.validate(&json!([]), &JsonPath::root());
/// assert!(result.is_failure());
/// ```
pub struct ArraySchema<S> {
    item_schema: S,
    constraints: Vec<ArrayConstraint>,
    type_error_message: Option<String>,
}

impl<S: SchemaLike> ArraySchema<S> {
    /// Creates a new array schema with the given item schema.
    pub fn new(item_schema: S) -> Self {
        Self {
            item_schema,
            constraints: Vec::new(),
            type_error_message: None,
        }
    }

    /// Adds a minimum length constraint.
    ///
    /// The array must have at least `min` items.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::array(Schema::integer()).min_len(2);
    ///
    /// let result = schema.validate(&json!([1, 2, 3]), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!([1]), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn min_len(mut self, min: usize) -> Self {
        self.constraints
            .push(ArrayConstraint::MinLength { min, message: None });
        self
    }

    /// Adds a maximum length constraint.
    ///
    /// The array must have at most `max` items.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::array(Schema::integer()).max_len(3);
    ///
    /// let result = schema.validate(&json!([1, 2]), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!([1, 2, 3, 4]), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn max_len(mut self, max: usize) -> Self {
        self.constraints
            .push(ArrayConstraint::MaxLength { max, message: None });
        self
    }

    /// Adds a non-empty constraint.
    ///
    /// The array must have at least one item. This is a convenience method
    /// equivalent to `.min_len(1)`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::array(Schema::string()).non_empty();
    ///
    /// let result = schema.validate(&json!(["hello"]), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!([]), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn non_empty(self) -> Self {
        self.min_len(1)
    }

    /// Adds a uniqueness constraint.
    ///
    /// All items in the array must be distinct (by JSON equality).
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::array(Schema::string()).unique();
    ///
    /// let result = schema.validate(&json!(["a", "b", "c"]), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!(["a", "b", "a"]), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn unique(mut self) -> Self {
        self.constraints
            .push(ArrayConstraint::Unique { message: None });
        self
    }

    /// Adds a uniqueness-by-key constraint.
    ///
    /// All items in the array must have distinct values for the given key function.
    /// This is useful for arrays of objects where you want uniqueness by a specific field.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::{json, Value};
    ///
    /// let schema = Schema::array(
    ///     Schema::object()
    ///         .field("id", Schema::integer())
    ///         .field("name", Schema::string())
    /// ).unique_by(|item| item.get("id").cloned().unwrap_or(Value::Null));
    ///
    /// let result = schema.validate(&json!([
    ///     {"id": 1, "name": "Alice"},
    ///     {"id": 2, "name": "Bob"}
    /// ]), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!([
    ///     {"id": 1, "name": "Alice"},
    ///     {"id": 1, "name": "Bob"}
    /// ]), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn unique_by<F>(mut self, key_fn: F) -> Self
    where
        F: Fn(&Value) -> Value + Send + Sync + 'static,
    {
        self.constraints.push(ArrayConstraint::UniqueBy {
            key_fn: Box::new(key_fn),
            message: None,
        });
        self
    }

    /// Sets a custom error message for the most recent constraint.
    ///
    /// If no constraints have been added yet, this sets the type error message
    /// (used when the value is not an array).
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::array(Schema::string())
    ///     .min_len(1)
    ///     .error("at least one tag is required");
    ///
    /// let result = schema.validate(&json!([]), &JsonPath::root());
    /// // Error message will be "at least one tag is required"
    /// ```
    pub fn error(mut self, message: impl Into<String>) -> Self {
        if let Some(last) = self.constraints.last_mut() {
            match last {
                ArrayConstraint::MinLength { message: m, .. } => *m = Some(message.into()),
                ArrayConstraint::MaxLength { message: m, .. } => *m = Some(message.into()),
                ArrayConstraint::Unique { message: m } => *m = Some(message.into()),
                ArrayConstraint::UniqueBy { message: m, .. } => *m = Some(message.into()),
            }
        } else {
            self.type_error_message = Some(message.into());
        }
        self
    }

    /// Validates a value against this schema.
    ///
    /// Returns `Validation::Success` with a `Vec<Value>` containing the validated
    /// items if all validations pass, or `Validation::Failure` with all accumulated
    /// errors if any validations fail.
    ///
    /// # Validation Process
    ///
    /// 1. Check that the value is an array (type check)
    /// 2. Check length constraints (min/max)
    /// 3. Validate each item against the item schema
    /// 4. Check uniqueness constraints
    ///
    /// All errors from all steps are accumulated and returned together.
    pub fn validate(&self, value: &Value, path: &JsonPath) -> Validation<Vec<Value>, SchemaErrors> {
        // Check if it's an array
        let arr = match value.as_array() {
            Some(a) => a,
            None => {
                let message = self
                    .type_error_message
                    .clone()
                    .unwrap_or_else(|| "expected array".to_string());
                return Validation::Failure(SchemaErrors::single(
                    SchemaError::new(path.clone(), message)
                        .with_code("invalid_type")
                        .with_got(value_type_name(value))
                        .with_expected("array"),
                ));
            }
        };

        let mut errors = Vec::new();

        // Check length constraints
        for constraint in &self.constraints {
            match constraint {
                ArrayConstraint::MinLength { min, message } if arr.len() < *min => {
                    let msg = message.clone().unwrap_or_else(|| {
                        format!("array must have at least {} items, got {}", min, arr.len())
                    });
                    errors.push(
                        SchemaError::new(path.clone(), msg)
                            .with_code("min_length")
                            .with_expected(format!("at least {} items", min))
                            .with_got(format!("{} items", arr.len())),
                    );
                }
                ArrayConstraint::MaxLength { max, message } if arr.len() > *max => {
                    let msg = message.clone().unwrap_or_else(|| {
                        format!("array must have at most {} items, got {}", max, arr.len())
                    });
                    errors.push(
                        SchemaError::new(path.clone(), msg)
                            .with_code("max_length")
                            .with_expected(format!("at most {} items", max))
                            .with_got(format!("{} items", arr.len())),
                    );
                }
                _ => {}
            }
        }

        // Validate each item
        let mut validated_items = Vec::with_capacity(arr.len());
        for (index, item) in arr.iter().enumerate() {
            let item_path = path.push_index(index);
            match self.item_schema.validate_to_value(item, &item_path) {
                Validation::Success(v) => validated_items.push(v),
                Validation::Failure(e) => errors.extend(e.into_iter()),
            }
        }

        // Check uniqueness constraints
        for constraint in &self.constraints {
            match constraint {
                ArrayConstraint::Unique { message } => {
                    let duplicates = find_duplicates(arr, |v| v.clone());
                    for indices in duplicates.values() {
                        if indices.len() > 1 {
                            let msg = message.clone().unwrap_or_else(|| {
                                format!("duplicate value at indices {:?}", indices)
                            });
                            errors.push(
                                SchemaError::new(path.clone(), msg)
                                    .with_code("unique")
                                    .with_got(format!("duplicates at indices {:?}", indices)),
                            );
                        }
                    }
                }
                ArrayConstraint::UniqueBy { key_fn, message } => {
                    let duplicates = find_duplicates(arr, key_fn);
                    for indices in duplicates.values() {
                        if indices.len() > 1 {
                            let msg = message.clone().unwrap_or_else(|| {
                                format!("duplicate key at indices {:?}", indices)
                            });
                            errors.push(
                                SchemaError::new(path.clone(), msg)
                                    .with_code("unique")
                                    .with_got(format!("duplicates at indices {:?}", indices)),
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        if errors.is_empty() {
            Validation::Success(validated_items)
        } else {
            Validation::Failure(SchemaErrors::from_vec(errors))
        }
    }
}

impl<S: SchemaLike> SchemaLike for ArraySchema<S> {
    type Output = Vec<Value>;

    fn validate(&self, value: &Value, path: &JsonPath) -> Validation<Self::Output, SchemaErrors> {
        self.validate(value, path)
    }

    fn validate_to_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        self.validate(value, path).map(Value::Array)
    }

    fn validate_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &crate::validation::ValidationContext,
    ) -> Validation<Self::Output, SchemaErrors> {
        // Check if it's an array
        let arr = match value.as_array() {
            Some(a) => a,
            None => {
                let message = self
                    .type_error_message
                    .clone()
                    .unwrap_or_else(|| "expected array".to_string());
                return Validation::Failure(SchemaErrors::single(
                    SchemaError::new(path.clone(), message)
                        .with_code("invalid_type")
                        .with_got(value_type_name(value))
                        .with_expected("array"),
                ));
            }
        };

        let mut errors = Vec::new();

        // Check length constraints
        for constraint in &self.constraints {
            match constraint {
                ArrayConstraint::MinLength { min, message } if arr.len() < *min => {
                    let msg = message.clone().unwrap_or_else(|| {
                        format!("array must have at least {} items, got {}", min, arr.len())
                    });
                    errors.push(
                        SchemaError::new(path.clone(), msg)
                            .with_code("min_length")
                            .with_expected(format!("at least {} items", min))
                            .with_got(format!("{} items", arr.len())),
                    );
                }
                ArrayConstraint::MaxLength { max, message } if arr.len() > *max => {
                    let msg = message.clone().unwrap_or_else(|| {
                        format!("array must have at most {} items, got {}", max, arr.len())
                    });
                    errors.push(
                        SchemaError::new(path.clone(), msg)
                            .with_code("max_length")
                            .with_expected(format!("at most {} items", max))
                            .with_got(format!("{} items", arr.len())),
                    );
                }
                _ => {}
            }
        }

        // Validate each item with context (depth does not increment for array items)
        let mut validated_items = Vec::with_capacity(arr.len());
        for (index, item) in arr.iter().enumerate() {
            let item_path = path.push_index(index);
            match self
                .item_schema
                .validate_to_value_with_context(item, &item_path, context)
            {
                Validation::Success(v) => validated_items.push(v),
                Validation::Failure(e) => errors.extend(e.into_iter()),
            }
        }

        // Check uniqueness constraints
        for constraint in &self.constraints {
            match constraint {
                ArrayConstraint::Unique { message } => {
                    let duplicates = find_duplicates(arr, |v| v.clone());
                    for indices in duplicates.values() {
                        if indices.len() > 1 {
                            let msg = message.clone().unwrap_or_else(|| {
                                format!("duplicate value at indices {:?}", indices)
                            });
                            errors.push(
                                SchemaError::new(path.clone(), msg)
                                    .with_code("unique")
                                    .with_got(format!("duplicates at indices {:?}", indices)),
                            );
                        }
                    }
                }
                ArrayConstraint::UniqueBy { key_fn, message } => {
                    let duplicates = find_duplicates(arr, key_fn);
                    for indices in duplicates.values() {
                        if indices.len() > 1 {
                            let msg = message.clone().unwrap_or_else(|| {
                                format!("duplicate key at indices {:?}", indices)
                            });
                            errors.push(
                                SchemaError::new(path.clone(), msg)
                                    .with_code("unique")
                                    .with_got(format!("duplicates at indices {:?}", indices)),
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        if errors.is_empty() {
            Validation::Success(validated_items)
        } else {
            Validation::Failure(SchemaErrors::from_vec(errors))
        }
    }

    fn validate_to_value_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &crate::validation::ValidationContext,
    ) -> Validation<Value, SchemaErrors> {
        self.validate_with_context(value, path, context)
            .map(Value::Array)
    }

    fn collect_refs(&self, refs: &mut Vec<String>) {
        self.item_schema.collect_refs(refs);
    }
}

/// Finds duplicate values in an array based on a key function.
///
/// Returns a HashMap where keys are the JSON-serialized key values and values
/// are vectors of indices where that key appears.
fn find_duplicates<F>(arr: &[Value], key_fn: F) -> HashMap<String, Vec<usize>>
where
    F: Fn(&Value) -> Value,
{
    let mut seen: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, item) in arr.iter().enumerate() {
        let key = key_fn(item);
        // Use JSON serialization as the key for HashMap
        // This handles all JSON value types correctly
        let key_str = serde_json::to_string(&key).unwrap_or_else(|_| format!("{:?}", key));
        seen.entry(key_str).or_default().push(i);
    }
    seen
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
    use crate::schema::{IntegerSchema, ObjectSchema, StringSchema};
    use serde_json::json;

    fn unwrap_success<T, E: std::fmt::Debug>(v: Validation<T, E>) -> T {
        v.into_result().unwrap()
    }

    fn unwrap_failure<T: std::fmt::Debug, E>(v: Validation<T, E>) -> E {
        v.into_result().unwrap_err()
    }

    // Basic array validation tests

    #[test]
    fn test_array_schema_accepts_array() {
        let schema = ArraySchema::new(StringSchema::new());
        let result = schema.validate(&json!(["hello", "world"]), &JsonPath::root());
        assert!(result.is_success());
        let items = unwrap_success(result);
        assert_eq!(items, vec![json!("hello"), json!("world")]);
    }

    #[test]
    fn test_array_schema_accepts_empty_array() {
        let schema = ArraySchema::new(StringSchema::new());
        let result = schema.validate(&json!([]), &JsonPath::root());
        assert!(result.is_success());
        let items = unwrap_success(result);
        assert!(items.is_empty());
    }

    #[test]
    fn test_array_schema_rejects_non_array() {
        let schema = ArraySchema::new(StringSchema::new());

        let result = schema.validate(&json!("not an array"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_type");
        assert_eq!(errors.first().got, Some("string".to_string()));

        let result = schema.validate(&json!(42), &JsonPath::root());
        assert!(result.is_failure());

        let result = schema.validate(&json!(null), &JsonPath::root());
        assert!(result.is_failure());

        let result = schema.validate(&json!({"key": "value"}), &JsonPath::root());
        assert!(result.is_failure());
    }

    // Item validation tests

    #[test]
    fn test_array_validates_items() {
        let schema = ArraySchema::new(IntegerSchema::new().positive());
        let result = schema.validate(&json!([1, 2, 3]), &JsonPath::root());
        assert!(result.is_success());
    }

    #[test]
    fn test_array_reports_invalid_items() {
        let schema = ArraySchema::new(IntegerSchema::new().positive());
        let result = schema.validate(&json!([1, -2, 3]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors.first().code, "positive");
        assert_eq!(errors.first().path.to_string(), "[1]");
    }

    #[test]
    fn test_array_accumulates_multiple_item_errors() {
        let schema = ArraySchema::new(IntegerSchema::new().positive());
        let result = schema.validate(&json!([-1, -2, 3, -4]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.len(), 3);
    }

    #[test]
    fn test_array_validates_nested_objects() {
        let user_schema = ObjectSchema::new()
            .field("name", StringSchema::new().min_len(1))
            .field("age", IntegerSchema::new().positive());

        let schema = ArraySchema::new(user_schema);

        // Valid nested objects
        let result = schema.validate(
            &json!([
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 25}
            ]),
            &JsonPath::root(),
        );
        assert!(result.is_success());

        // Invalid nested objects
        let result = schema.validate(
            &json!([
                {"name": "", "age": 30},
                {"name": "Bob", "age": -5}
            ]),
            &JsonPath::root(),
        );
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.len(), 2);

        // Check paths include array index
        let paths: Vec<_> = errors.iter().map(|e| e.path.to_string()).collect();
        assert!(paths.contains(&"[0].name".to_string()));
        assert!(paths.contains(&"[1].age".to_string()));
    }

    // Length constraint tests

    #[test]
    fn test_min_len_constraint() {
        let schema = ArraySchema::new(StringSchema::new()).min_len(2);

        let result = schema.validate(&json!(["a", "b"]), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(["a", "b", "c"]), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(["a"]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "min_length");
    }

    #[test]
    fn test_max_len_constraint() {
        let schema = ArraySchema::new(StringSchema::new()).max_len(3);

        let result = schema.validate(&json!(["a", "b"]), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(["a", "b", "c"]), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(["a", "b", "c", "d"]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "max_length");
    }

    #[test]
    fn test_non_empty_constraint() {
        let schema = ArraySchema::new(StringSchema::new()).non_empty();

        let result = schema.validate(&json!(["a"]), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!([]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "min_length");
    }

    #[test]
    fn test_combined_length_constraints() {
        let schema = ArraySchema::new(StringSchema::new()).min_len(2).max_len(4);

        let result = schema.validate(&json!(["a"]), &JsonPath::root());
        assert!(result.is_failure());

        let result = schema.validate(&json!(["a", "b"]), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(["a", "b", "c", "d"]), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(["a", "b", "c", "d", "e"]), &JsonPath::root());
        assert!(result.is_failure());
    }

    // Uniqueness constraint tests

    #[test]
    fn test_unique_constraint_with_distinct_values() {
        let schema = ArraySchema::new(StringSchema::new()).unique();
        let result = schema.validate(&json!(["a", "b", "c"]), &JsonPath::root());
        assert!(result.is_success());
    }

    #[test]
    fn test_unique_constraint_with_duplicates() {
        let schema = ArraySchema::new(StringSchema::new()).unique();
        let result = schema.validate(&json!(["a", "b", "a"]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "unique");
    }

    #[test]
    fn test_unique_constraint_with_integers() {
        let schema = ArraySchema::new(IntegerSchema::new()).unique();

        let result = schema.validate(&json!([1, 2, 3]), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!([1, 2, 1]), &JsonPath::root());
        assert!(result.is_failure());
    }

    #[test]
    fn test_unique_constraint_empty_array() {
        let schema = ArraySchema::new(StringSchema::new()).unique();
        let result = schema.validate(&json!([]), &JsonPath::root());
        assert!(result.is_success());
    }

    #[test]
    fn test_unique_constraint_single_item() {
        let schema = ArraySchema::new(StringSchema::new()).unique();
        let result = schema.validate(&json!(["only"]), &JsonPath::root());
        assert!(result.is_success());
    }

    #[test]
    fn test_unique_by_constraint() {
        let user_schema = ObjectSchema::new()
            .field("id", IntegerSchema::new())
            .field("name", StringSchema::new());

        let schema = ArraySchema::new(user_schema)
            .unique_by(|v| v.get("id").cloned().unwrap_or(Value::Null));

        // Unique IDs
        let result = schema.validate(
            &json!([
                {"id": 1, "name": "Alice"},
                {"id": 2, "name": "Bob"}
            ]),
            &JsonPath::root(),
        );
        assert!(result.is_success());

        // Duplicate IDs
        let result = schema.validate(
            &json!([
                {"id": 1, "name": "Alice"},
                {"id": 1, "name": "Bob"}
            ]),
            &JsonPath::root(),
        );
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "unique");
    }

    // Error accumulation tests

    #[test]
    fn test_error_accumulation_length_and_items() {
        let schema = ArraySchema::new(IntegerSchema::new().positive()).min_len(3);

        // Too short AND has invalid items
        let result = schema.validate(&json!([-1, -2]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        // Should have: 1 min_length error + 2 positive errors
        assert_eq!(errors.len(), 3);
        assert_eq!(errors.with_code("min_length").len(), 1);
        assert_eq!(errors.with_code("positive").len(), 2);
    }

    #[test]
    fn test_error_accumulation_all_constraint_types() {
        let schema = ArraySchema::new(IntegerSchema::new().positive())
            .min_len(5)
            .unique();

        // Too short, has invalid items, and has duplicates
        let result = schema.validate(&json!([1, -2, 1]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        // Should have: 1 min_length + 1 positive + 1 unique
        assert_eq!(errors.len(), 3);
    }

    // Path tracking tests

    #[test]
    fn test_path_tracking_simple() {
        let schema = ArraySchema::new(StringSchema::new().min_len(5));
        let result = schema.validate(&json!(["hi"]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().path.to_string(), "[0]");
    }

    #[test]
    fn test_path_tracking_nested() {
        let inner_schema = ObjectSchema::new().field("value", IntegerSchema::new().positive());
        let schema = ArraySchema::new(inner_schema);

        let path = JsonPath::root().push_field("items");
        let result = schema.validate(&json!([{"value": -5}]), &path);
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().path.to_string(), "items[0].value");
    }

    #[test]
    fn test_path_tracking_deeply_nested() {
        let inner_array = ArraySchema::new(IntegerSchema::new().positive());
        let outer_schema = ObjectSchema::new().field("numbers", inner_array);
        let outer_array = ArraySchema::new(outer_schema);

        let result = outer_array.validate(
            &json!([
                {"numbers": [1, -2, 3]}
            ]),
            &JsonPath::root(),
        );
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().path.to_string(), "[0].numbers[1]");
    }

    // Custom error message tests

    #[test]
    fn test_custom_type_error_message() {
        let schema = ArraySchema::new(StringSchema::new()).error("must be a list of tags");

        let result = schema.validate(&json!("not an array"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().message, "must be a list of tags");
    }

    #[test]
    fn test_custom_min_length_error_message() {
        let schema = ArraySchema::new(StringSchema::new())
            .min_len(1)
            .error("at least one tag is required");

        let result = schema.validate(&json!([]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().message, "at least one tag is required");
    }

    #[test]
    fn test_custom_unique_error_message() {
        let schema = ArraySchema::new(StringSchema::new())
            .unique()
            .error("all tags must be unique");

        let result = schema.validate(&json!(["a", "a"]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().message, "all tags must be unique");
    }

    // Edge case tests

    #[test]
    fn test_array_of_nulls() {
        // String schema should reject nulls
        let schema = ArraySchema::new(StringSchema::new());
        let result = schema.validate(&json!([null, null]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_mixed_type_array() {
        // Integer schema should reject strings
        let schema = ArraySchema::new(IntegerSchema::new());
        let result = schema.validate(&json!([1, "two", 3]), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors.first().path.to_string(), "[1]");
    }

    #[test]
    fn test_large_array() {
        let schema = ArraySchema::new(IntegerSchema::new());
        let large_array: Vec<i32> = (0..1000).collect();
        let result = schema.validate(&json!(large_array), &JsonPath::root());
        assert!(result.is_success());
    }

    #[test]
    fn test_unique_with_objects() {
        let schema = ArraySchema::new(ObjectSchema::new()).unique();

        // Different objects
        let result = schema.validate(&json!([{"a": 1}, {"a": 2}]), &JsonPath::root());
        assert!(result.is_success());

        // Same objects
        let result = schema.validate(&json!([{"a": 1}, {"a": 1}]), &JsonPath::root());
        assert!(result.is_failure());
    }

    // SchemaLike trait tests

    #[test]
    fn test_schema_like_validate_to_value() {
        let schema = ArraySchema::new(StringSchema::new());
        let result = schema.validate_to_value(&json!(["hello"]), &JsonPath::root());
        assert!(result.is_success());
        match result.into_result().unwrap() {
            Value::Array(arr) => {
                assert_eq!(arr, vec![json!("hello")]);
            }
            _ => panic!("Expected array"),
        }
    }
}

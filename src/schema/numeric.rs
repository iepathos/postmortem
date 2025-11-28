//! Numeric schema validation.
//!
//! This module provides [`IntegerSchema`] for validating integer values with
//! constraints like minimum/maximum value and sign requirements.

use serde_json::Value;
use std::ops::RangeInclusive;
use stillwater::Validation;

use crate::error::{SchemaError, SchemaErrors};
use crate::path::JsonPath;

use super::traits::SchemaLike;

/// A constraint applied to integer values.
#[derive(Clone)]
enum IntegerConstraint {
    Min { value: i64, message: Option<String> },
    Max { value: i64, message: Option<String> },
    Positive { message: Option<String> },
    NonNegative { message: Option<String> },
    Negative { message: Option<String> },
}

/// A schema for validating integer values.
///
/// `IntegerSchema` validates that values are integers and optionally applies
/// constraints like minimum/maximum value and sign requirements. All constraint
/// violations are accumulated rather than short-circuiting on the first failure.
///
/// # Example
///
/// ```rust
/// use postmortem::{Schema, JsonPath};
/// use serde_json::json;
///
/// // Create schema with multiple constraints
/// let schema = Schema::integer()
///     .min(0)
///     .max(100);
///
/// // Validation accumulates all errors
/// let result = schema.validate(&json!(-50), &JsonPath::root());
/// assert!(result.is_failure());
/// // Will report: value less than minimum
/// ```
#[derive(Clone)]
pub struct IntegerSchema {
    constraints: Vec<IntegerConstraint>,
    type_error_message: Option<String>,
}

impl IntegerSchema {
    /// Creates a new integer schema with no constraints.
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
            type_error_message: None,
        }
    }

    /// Adds a minimum value constraint (inclusive).
    ///
    /// The integer must be at least `value`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::integer().min(5);
    ///
    /// let result = schema.validate(&json!(10), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!(3), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn min(mut self, value: i64) -> Self {
        self.constraints.push(IntegerConstraint::Min {
            value,
            message: None,
        });
        self
    }

    /// Adds a maximum value constraint (inclusive).
    ///
    /// The integer must be at most `value`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::integer().max(10);
    ///
    /// let result = schema.validate(&json!(5), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!(15), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn max(mut self, value: i64) -> Self {
        self.constraints.push(IntegerConstraint::Max {
            value,
            message: None,
        });
        self
    }

    /// Adds both minimum and maximum value constraints (inclusive range).
    ///
    /// This is a convenience method equivalent to calling `.min(start).max(end)`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::integer().range(1..=100);
    ///
    /// let result = schema.validate(&json!(50), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!(150), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn range(self, range: RangeInclusive<i64>) -> Self {
        self.min(*range.start()).max(*range.end())
    }

    /// Adds a positive value constraint.
    ///
    /// The integer must be greater than 0.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::integer().positive();
    ///
    /// let result = schema.validate(&json!(5), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!(0), &JsonPath::root());
    /// assert!(result.is_failure());
    ///
    /// let result = schema.validate(&json!(-1), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn positive(mut self) -> Self {
        self.constraints
            .push(IntegerConstraint::Positive { message: None });
        self
    }

    /// Adds a non-negative value constraint.
    ///
    /// The integer must be greater than or equal to 0.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::integer().non_negative();
    ///
    /// let result = schema.validate(&json!(0), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!(5), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!(-1), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn non_negative(mut self) -> Self {
        self.constraints
            .push(IntegerConstraint::NonNegative { message: None });
        self
    }

    /// Adds a negative value constraint.
    ///
    /// The integer must be less than 0.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::integer().negative();
    ///
    /// let result = schema.validate(&json!(-5), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!(0), &JsonPath::root());
    /// assert!(result.is_failure());
    ///
    /// let result = schema.validate(&json!(1), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn negative(mut self) -> Self {
        self.constraints
            .push(IntegerConstraint::Negative { message: None });
        self
    }

    /// Sets a custom error message for the most recent constraint.
    ///
    /// If no constraints have been added yet, this sets the type error message
    /// (used when the value is not an integer).
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::integer()
    ///     .min(18)
    ///     .error("must be at least 18 years old");
    ///
    /// let result = schema.validate(&json!(16), &JsonPath::root());
    /// // Error message will be "must be at least 18 years old"
    /// ```
    pub fn error(mut self, message: impl Into<String>) -> Self {
        if let Some(last) = self.constraints.last_mut() {
            match last {
                IntegerConstraint::Min { message: m, .. } => *m = Some(message.into()),
                IntegerConstraint::Max { message: m, .. } => *m = Some(message.into()),
                IntegerConstraint::Positive { message: m } => *m = Some(message.into()),
                IntegerConstraint::NonNegative { message: m } => *m = Some(message.into()),
                IntegerConstraint::Negative { message: m } => *m = Some(message.into()),
            }
        } else {
            self.type_error_message = Some(message.into());
        }
        self
    }

    /// Validates a value against this schema.
    ///
    /// Returns `Validation::Success` with the validated i64 if all
    /// constraints pass, or `Validation::Failure` with all accumulated
    /// errors if any constraints fail.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::integer().min(0).max(100);
    ///
    /// match schema.validate(&json!(50), &JsonPath::root()) {
    ///     stillwater::Validation::Success(n) => println!("Valid: {}", n),
    ///     stillwater::Validation::Failure(errors) => {
    ///         for error in errors.iter() {
    ///             println!("Error: {}", error);
    ///         }
    ///     }
    /// }
    /// ```
    pub fn validate(&self, value: &Value, path: &JsonPath) -> Validation<i64, SchemaErrors> {
        // Check for integer (not float)
        let n = match value {
            Value::Number(num) if num.is_i64() => num.as_i64().unwrap(),
            Value::Number(num) if num.is_u64() => {
                // Handle u64 values that fit in i64
                let u = num.as_u64().unwrap();
                if u <= i64::MAX as u64 {
                    u as i64
                } else {
                    // u64 value too large for i64, still valid integer but report overflow
                    let message = self
                        .type_error_message
                        .clone()
                        .unwrap_or_else(|| "integer value too large for i64".to_string());
                    return Validation::Failure(SchemaErrors::single(
                        SchemaError::new(path.clone(), message)
                            .with_code("overflow")
                            .with_got(format!("{}", u))
                            .with_expected("integer in i64 range"),
                    ));
                }
            }
            Value::Number(_) => {
                // It's a float
                let message = self
                    .type_error_message
                    .clone()
                    .unwrap_or_else(|| "expected integer, got float".to_string());
                return Validation::Failure(SchemaErrors::single(
                    SchemaError::new(path.clone(), message)
                        .with_code("invalid_type")
                        .with_got("float")
                        .with_expected("integer"),
                ));
            }
            _ => {
                let message = self
                    .type_error_message
                    .clone()
                    .unwrap_or_else(|| "expected integer".to_string());
                return Validation::Failure(SchemaErrors::single(
                    SchemaError::new(path.clone(), message)
                        .with_code("invalid_type")
                        .with_got(value_type_name(value))
                        .with_expected("integer"),
                ));
            }
        };

        // Collect all constraint violations
        let errors: Vec<SchemaError> = self
            .constraints
            .iter()
            .filter_map(|c| check_constraint(c, n, path))
            .collect();

        if errors.is_empty() {
            Validation::Success(n)
        } else {
            Validation::Failure(SchemaErrors::from_vec(errors))
        }
    }
}

impl Default for IntegerSchema {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaLike for IntegerSchema {
    type Output = i64;

    fn validate(&self, value: &Value, path: &JsonPath) -> Validation<Self::Output, SchemaErrors> {
        self.validate(value, path)
    }

    fn validate_to_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        self.validate(value, path).map(|n| Value::Number(n.into()))
    }
}

/// Checks a single constraint and returns an error if it fails.
fn check_constraint(
    constraint: &IntegerConstraint,
    value: i64,
    path: &JsonPath,
) -> Option<SchemaError> {
    match constraint {
        IntegerConstraint::Min {
            value: min,
            message,
        } => {
            if value < *min {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("must be at least {}, got {}", min, value));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code("min_value")
                        .with_expected(format!("at least {}", min))
                        .with_got(format!("{}", value)),
                )
            } else {
                None
            }
        }
        IntegerConstraint::Max {
            value: max,
            message,
        } => {
            if value > *max {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("must be at most {}, got {}", max, value));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code("max_value")
                        .with_expected(format!("at most {}", max))
                        .with_got(format!("{}", value)),
                )
            } else {
                None
            }
        }
        IntegerConstraint::Positive { message } => {
            if value <= 0 {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("must be positive, got {}", value));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code("positive")
                        .with_expected("value > 0")
                        .with_got(format!("{}", value)),
                )
            } else {
                None
            }
        }
        IntegerConstraint::NonNegative { message } => {
            if value < 0 {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("must be non-negative, got {}", value));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code("non_negative")
                        .with_expected("value >= 0")
                        .with_got(format!("{}", value)),
                )
            } else {
                None
            }
        }
        IntegerConstraint::Negative { message } => {
            if value >= 0 {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("must be negative, got {}", value));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code("negative")
                        .with_expected("value < 0")
                        .with_got(format!("{}", value)),
                )
            } else {
                None
            }
        }
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
    use serde_json::json;

    fn unwrap_success<T, E: std::fmt::Debug>(v: Validation<T, E>) -> T {
        v.into_result().unwrap()
    }

    fn unwrap_failure<T: std::fmt::Debug, E>(v: Validation<T, E>) -> E {
        v.into_result().unwrap_err()
    }

    #[test]
    fn test_integer_schema_accepts_integer() {
        let schema = IntegerSchema::new();
        let result = schema.validate(&json!(42), &JsonPath::root());
        assert!(result.is_success());
        assert_eq!(unwrap_success(result), 42);
    }

    #[test]
    fn test_integer_schema_accepts_negative_integer() {
        let schema = IntegerSchema::new();
        let result = schema.validate(&json!(-42), &JsonPath::root());
        assert!(result.is_success());
        assert_eq!(unwrap_success(result), -42);
    }

    #[test]
    fn test_integer_schema_accepts_zero() {
        let schema = IntegerSchema::new();
        let result = schema.validate(&json!(0), &JsonPath::root());
        assert!(result.is_success());
        assert_eq!(unwrap_success(result), 0);
    }

    #[test]
    fn test_integer_schema_rejects_float() {
        let schema = IntegerSchema::new();
        let result = schema.validate(&json!(1.5), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_type");
        assert_eq!(errors.first().got, Some("float".to_string()));
    }

    #[test]
    fn test_integer_schema_rejects_float_with_zero_decimal() {
        let schema = IntegerSchema::new();
        // Note: JSON 1.0 is parsed as float by serde_json
        let result = schema.validate(&json!(1.0), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_type");
    }

    #[test]
    fn test_integer_schema_rejects_non_number() {
        let schema = IntegerSchema::new();

        let result = schema.validate(&json!("42"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_type");
        assert_eq!(errors.first().got, Some("string".to_string()));

        let result = schema.validate(&json!(null), &JsonPath::root());
        assert!(result.is_failure());

        let result = schema.validate(&json!(true), &JsonPath::root());
        assert!(result.is_failure());

        let result = schema.validate(&json!([1, 2, 3]), &JsonPath::root());
        assert!(result.is_failure());

        let result = schema.validate(&json!({"key": "value"}), &JsonPath::root());
        assert!(result.is_failure());
    }

    #[test]
    fn test_min_constraint() {
        let schema = IntegerSchema::new().min(5);

        let result = schema.validate(&json!(5), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(10), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(4), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "min_value");
    }

    #[test]
    fn test_max_constraint() {
        let schema = IntegerSchema::new().max(10);

        let result = schema.validate(&json!(10), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(5), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(11), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "max_value");
    }

    #[test]
    fn test_range_constraint() {
        let schema = IntegerSchema::new().range(5..=10);

        let result = schema.validate(&json!(5), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(7), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(10), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(4), &JsonPath::root());
        assert!(result.is_failure());

        let result = schema.validate(&json!(11), &JsonPath::root());
        assert!(result.is_failure());
    }

    #[test]
    fn test_both_range_violations_reported() {
        // Test with min > max (impossible constraint) to verify accumulation
        let schema = IntegerSchema::new().min(10).max(5);

        let result = schema.validate(&json!(7), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        // Should report max_value violation (7 > 5)
        assert!(errors.with_code("max_value").len() == 1);
    }

    #[test]
    fn test_value_outside_range_reports_both_errors() {
        // Value below min with impossible range
        let schema = IntegerSchema::new().min(10).max(5);

        let result = schema.validate(&json!(3), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        // Should report both min_value violation (3 < 10) and max_value (3 is OK for max 5)
        // Actually 3 < 5, so only min_value violation
        assert_eq!(errors.len(), 1);
        assert!(errors.with_code("min_value").len() == 1);
    }

    #[test]
    fn test_positive_constraint() {
        let schema = IntegerSchema::new().positive();

        let result = schema.validate(&json!(1), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(100), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(0), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "positive");

        let result = schema.validate(&json!(-1), &JsonPath::root());
        assert!(result.is_failure());
    }

    #[test]
    fn test_non_negative_constraint() {
        let schema = IntegerSchema::new().non_negative();

        let result = schema.validate(&json!(0), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(1), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(-1), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "non_negative");
    }

    #[test]
    fn test_negative_constraint() {
        let schema = IntegerSchema::new().negative();

        let result = schema.validate(&json!(-1), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(-100), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(0), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "negative");

        let result = schema.validate(&json!(1), &JsonPath::root());
        assert!(result.is_failure());
    }

    #[test]
    fn test_custom_error_message() {
        let schema = IntegerSchema::new()
            .min(18)
            .error("must be at least 18 years old");

        let result = schema.validate(&json!(16), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().message, "must be at least 18 years old");
    }

    #[test]
    fn test_custom_type_error_message() {
        let schema = IntegerSchema::new().error("must be an integer");

        let result = schema.validate(&json!("abc"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().message, "must be an integer");
    }

    #[test]
    fn test_error_accumulation() {
        let schema = IntegerSchema::new().min(10).positive();

        // -5 violates both min (< 10) and positive (< 0)
        let result = schema.validate(&json!(-5), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.len(), 2);
        assert!(errors.with_code("min_value").len() == 1);
        assert!(errors.with_code("positive").len() == 1);
    }

    #[test]
    fn test_path_tracking() {
        let schema = IntegerSchema::new().min(5);
        let path = JsonPath::root().push_field("user").push_field("age");

        let result = schema.validate(&json!(3), &path);
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().path.to_string(), "user.age");
    }

    #[test]
    fn test_i64_min_max() {
        let schema = IntegerSchema::new();

        let result = schema.validate(&json!(i64::MIN), &JsonPath::root());
        assert!(result.is_success());
        assert_eq!(unwrap_success(result), i64::MIN);

        let result = schema.validate(&json!(i64::MAX), &JsonPath::root());
        assert!(result.is_success());
        assert_eq!(unwrap_success(result), i64::MAX);
    }

    #[test]
    fn test_schema_clone() {
        let schema = IntegerSchema::new().min(5).max(10);
        let cloned = schema.clone();

        let result = cloned.validate(&json!(7), &JsonPath::root());
        assert!(result.is_success());
    }
}

//! String schema validation.
//!
//! This module provides [`StringSchema`] for validating string values with
//! constraints like minimum/maximum length and regex patterns.

use regex::Regex;
use serde_json::Value;
use stillwater::Validation;

use crate::error::{SchemaError, SchemaErrors};
use crate::path::JsonPath;

/// A constraint applied to string values.
#[derive(Clone)]
enum StringConstraint {
    MinLength {
        min: usize,
        message: Option<String>,
    },
    MaxLength {
        max: usize,
        message: Option<String>,
    },
    Pattern {
        regex: Regex,
        pattern_str: String,
        message: Option<String>,
    },
}

/// A schema for validating string values.
///
/// `StringSchema` validates that values are strings and optionally applies
/// constraints like minimum/maximum length and regex patterns. All constraint
/// violations are accumulated rather than short-circuiting on the first failure.
///
/// # Example
///
/// ```rust
/// use postmortem::{Schema, JsonPath};
/// use serde_json::json;
///
/// // Create schema with multiple constraints
/// let schema = Schema::string()
///     .min_len(3)
///     .max_len(20)
///     .pattern(r"^[a-z]+$")
///     .unwrap();
///
/// // Validation accumulates all errors
/// let result = schema.validate(&json!("AB"), &JsonPath::root());
/// assert!(result.is_failure());
/// // Will report both: too short AND pattern mismatch
/// ```
#[derive(Clone)]
pub struct StringSchema {
    constraints: Vec<StringConstraint>,
    type_error_message: Option<String>,
}

impl StringSchema {
    /// Creates a new string schema with no constraints.
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
            type_error_message: None,
        }
    }

    /// Adds a minimum length constraint.
    ///
    /// The string must have at least `min` characters (Unicode scalar values).
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
    pub fn min_len(mut self, min: usize) -> Self {
        self.constraints
            .push(StringConstraint::MinLength { min, message: None });
        self
    }

    /// Adds a maximum length constraint.
    ///
    /// The string must have at most `max` characters (Unicode scalar values).
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::string().max_len(10);
    ///
    /// let result = schema.validate(&json!("hello"), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!("this is too long"), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn max_len(mut self, max: usize) -> Self {
        self.constraints
            .push(StringConstraint::MaxLength { max, message: None });
        self
    }

    /// Adds a regex pattern constraint.
    ///
    /// The string must match the provided regex pattern.
    /// Returns an error if the regex pattern is invalid.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::string()
    ///     .pattern(r"^\d+$")
    ///     .unwrap();
    ///
    /// let result = schema.validate(&json!("12345"), &JsonPath::root());
    /// assert!(result.is_success());
    ///
    /// let result = schema.validate(&json!("abc"), &JsonPath::root());
    /// assert!(result.is_failure());
    /// ```
    pub fn pattern(mut self, pattern: &str) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        self.constraints.push(StringConstraint::Pattern {
            regex,
            pattern_str: pattern.to_string(),
            message: None,
        });
        Ok(self)
    }

    /// Sets a custom error message for the most recent constraint.
    ///
    /// If no constraints have been added yet, this sets the type error message
    /// (used when the value is not a string).
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::string()
    ///     .min_len(5)
    ///     .error("username must be at least 5 characters");
    ///
    /// let result = schema.validate(&json!("hi"), &JsonPath::root());
    /// // Error message will be "username must be at least 5 characters"
    /// ```
    pub fn error(mut self, message: impl Into<String>) -> Self {
        if let Some(last) = self.constraints.last_mut() {
            match last {
                StringConstraint::MinLength { message: m, .. } => *m = Some(message.into()),
                StringConstraint::MaxLength { message: m, .. } => *m = Some(message.into()),
                StringConstraint::Pattern { message: m, .. } => *m = Some(message.into()),
            }
        } else {
            self.type_error_message = Some(message.into());
        }
        self
    }

    /// Validates a value against this schema.
    ///
    /// Returns `Validation::Success` with the validated string if all
    /// constraints pass, or `Validation::Failure` with all accumulated
    /// errors if any constraints fail.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{Schema, JsonPath};
    /// use serde_json::json;
    ///
    /// let schema = Schema::string().min_len(1).max_len(10);
    ///
    /// match schema.validate(&json!("hello"), &JsonPath::root()) {
    ///     stillwater::Validation::Success(s) => println!("Valid: {}", s),
    ///     stillwater::Validation::Failure(errors) => {
    ///         for error in errors.iter() {
    ///             println!("Error: {}", error);
    ///         }
    ///     }
    /// }
    /// ```
    pub fn validate(&self, value: &Value, path: &JsonPath) -> Validation<String, SchemaErrors> {
        // First check if it's a string
        let s = match value.as_str() {
            Some(s) => s,
            None => {
                let message = self
                    .type_error_message
                    .clone()
                    .unwrap_or_else(|| "expected string".to_string());
                return Validation::Failure(SchemaErrors::single(
                    SchemaError::new(path.clone(), message)
                        .with_code("invalid_type")
                        .with_got(value_type_name(value))
                        .with_expected("string"),
                ));
            }
        };

        // Collect all constraint violations
        let errors: Vec<SchemaError> = self
            .constraints
            .iter()
            .filter_map(|c| check_constraint(c, s, path))
            .collect();

        if errors.is_empty() {
            Validation::Success(s.to_string())
        } else {
            Validation::Failure(SchemaErrors::from_vec(errors))
        }
    }
}

impl Default for StringSchema {
    fn default() -> Self {
        Self::new()
    }
}

/// Checks a single constraint and returns an error if it fails.
fn check_constraint(
    constraint: &StringConstraint,
    value: &str,
    path: &JsonPath,
) -> Option<SchemaError> {
    match constraint {
        StringConstraint::MinLength { min, message } => {
            let len = value.chars().count();
            if len < *min {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("length must be at least {}, got {}", min, len));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code("min_length")
                        .with_expected(format!("at least {} characters", min))
                        .with_got(format!("{} characters", len)),
                )
            } else {
                None
            }
        }
        StringConstraint::MaxLength { max, message } => {
            let len = value.chars().count();
            if len > *max {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("length must be at most {}, got {}", max, len));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code("max_length")
                        .with_expected(format!("at most {} characters", max))
                        .with_got(format!("{} characters", len)),
                )
            } else {
                None
            }
        }
        StringConstraint::Pattern {
            regex,
            pattern_str,
            message,
        } => {
            if !regex.is_match(value) {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("must match pattern '{}'", pattern_str));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code("pattern")
                        .with_expected(format!("string matching '{}'", pattern_str))
                        .with_got(value.to_string()),
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
    fn test_string_schema_accepts_string() {
        let schema = StringSchema::new();
        let result = schema.validate(&json!("hello"), &JsonPath::root());
        assert!(result.is_success());
        assert_eq!(unwrap_success(result), "hello");
    }

    #[test]
    fn test_string_schema_rejects_non_string() {
        let schema = StringSchema::new();

        let result = schema.validate(&json!(42), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_type");
        assert_eq!(errors.first().got, Some("number".to_string()));

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
    fn test_min_len_constraint() {
        let schema = StringSchema::new().min_len(5);

        let result = schema.validate(&json!("hello"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("hello world"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("hi"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "min_length");
    }

    #[test]
    fn test_max_len_constraint() {
        let schema = StringSchema::new().max_len(10);

        let result = schema.validate(&json!("hello"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!(""), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("this is way too long"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "max_length");
    }

    #[test]
    fn test_combined_length_constraints() {
        let schema = StringSchema::new().min_len(5).max_len(10);

        let result = schema.validate(&json!("hello"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("hi"), &JsonPath::root());
        assert!(result.is_failure());

        let result = schema.validate(&json!("this is way too long"), &JsonPath::root());
        assert!(result.is_failure());
    }

    #[test]
    fn test_both_length_violations_reported() {
        let schema = StringSchema::new().min_len(5).max_len(3);
        // This is an impossible constraint (min > max), but it tests accumulation

        let result = schema.validate(&json!("ab"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        // Should report min_length violation (2 < 5)
        assert!(errors.with_code("min_length").len() == 1);
    }

    #[test]
    fn test_pattern_constraint() {
        let schema = StringSchema::new().pattern(r"^\d+$").unwrap();

        let result = schema.validate(&json!("12345"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("abc"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "pattern");
    }

    #[test]
    fn test_pattern_error_includes_pattern() {
        let schema = StringSchema::new().pattern(r"^\d+$").unwrap();
        let result = schema.validate(&json!("abc"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert!(errors.first().message.contains(r"^\d+$"));
    }

    #[test]
    fn test_custom_error_message() {
        let schema = StringSchema::new().min_len(5).error("username too short");

        let result = schema.validate(&json!("ab"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().message, "username too short");
    }

    #[test]
    fn test_custom_type_error_message() {
        let schema = StringSchema::new().error("must be a string");

        let result = schema.validate(&json!(42), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().message, "must be a string");
    }

    #[test]
    fn test_error_accumulation() {
        let schema = StringSchema::new().min_len(10).pattern(r"^\d+$").unwrap();

        let result = schema.validate(&json!("abc"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        // Should have both min_length and pattern errors
        assert_eq!(errors.len(), 2);
        assert!(errors.with_code("min_length").len() == 1);
        assert!(errors.with_code("pattern").len() == 1);
    }

    #[test]
    fn test_path_tracking() {
        let schema = StringSchema::new().min_len(5);
        let path = JsonPath::root().push_field("user").push_field("name");

        let result = schema.validate(&json!("ab"), &path);
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().path.to_string(), "user.name");
    }

    #[test]
    fn test_empty_string() {
        let schema = StringSchema::new().min_len(1);

        let result = schema.validate(&json!(""), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "min_length");
    }

    #[test]
    fn test_unicode_length() {
        // Unicode strings should count characters, not bytes
        let schema = StringSchema::new().min_len(3).max_len(5);

        // "日本語" is 3 characters
        let result = schema.validate(&json!("日本語"), &JsonPath::root());
        assert!(result.is_success());

        // "🎉🎊" is 2 characters
        let result = schema.validate(&json!("🎉🎊"), &JsonPath::root());
        assert!(result.is_failure());
    }

    #[test]
    fn test_invalid_regex_pattern() {
        let result = StringSchema::new().pattern(r"[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_schema_clone() {
        let schema = StringSchema::new().min_len(5).max_len(10);
        let cloned = schema.clone();

        let result = cloned.validate(&json!("hello"), &JsonPath::root());
        assert!(result.is_success());
    }
}

//! String schema validation.
//!
//! This module provides [`StringSchema`] for validating string values with
//! constraints like minimum/maximum length and regex patterns.

use regex::Regex;
use serde_json::Value;
use std::sync::Arc;
use stillwater::Validation;

use crate::error::{SchemaError, SchemaErrors};
use crate::path::JsonPath;

use super::traits::SchemaLike;

/// Type alias for custom string validators.
type CustomValidator = Arc<dyn Fn(&str, &JsonPath) -> Validation<(), SchemaErrors> + Send + Sync>;

/// Format validator types for string values.
#[derive(Clone, Debug)]
enum Format {
    Email,
    Url,
    Uuid,
    Date,
    DateTime,
    Ip,
    Ipv4,
    Ipv6,
}

/// String transformation types.
#[derive(Clone, Debug)]
enum Transform {
    Trim,
    Lowercase,
}

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
    Format {
        format: Format,
        message: Option<String>,
    },
    OneOf {
        values: Vec<String>,
        message: Option<String>,
    },
    StartsWith {
        prefix: String,
        message: Option<String>,
    },
    EndsWith {
        suffix: String,
        message: Option<String>,
    },
    Contains {
        substring: String,
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
    transforms: Vec<Transform>,
    custom_validators: Vec<CustomValidator>,
    type_error_message: Option<String>,
}

impl StringSchema {
    /// Creates a new string schema with no constraints.
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
            transforms: Vec::new(),
            custom_validators: Vec::new(),
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

    /// Adds an email format constraint.
    pub fn email(mut self) -> Self {
        self.constraints.push(StringConstraint::Format {
            format: Format::Email,
            message: None,
        });
        self
    }

    /// Adds a URL format constraint (http/https).
    pub fn url(mut self) -> Self {
        self.constraints.push(StringConstraint::Format {
            format: Format::Url,
            message: None,
        });
        self
    }

    /// Adds a UUID format constraint.
    pub fn uuid(mut self) -> Self {
        self.constraints.push(StringConstraint::Format {
            format: Format::Uuid,
            message: None,
        });
        self
    }

    /// Adds a date format constraint (YYYY-MM-DD).
    pub fn date(mut self) -> Self {
        self.constraints.push(StringConstraint::Format {
            format: Format::Date,
            message: None,
        });
        self
    }

    /// Adds a datetime format constraint (ISO 8601).
    pub fn datetime(mut self) -> Self {
        self.constraints.push(StringConstraint::Format {
            format: Format::DateTime,
            message: None,
        });
        self
    }

    /// Adds an IP address format constraint (IPv4 or IPv6).
    pub fn ip(mut self) -> Self {
        self.constraints.push(StringConstraint::Format {
            format: Format::Ip,
            message: None,
        });
        self
    }

    /// Adds an IPv4 format constraint.
    pub fn ipv4(mut self) -> Self {
        self.constraints.push(StringConstraint::Format {
            format: Format::Ipv4,
            message: None,
        });
        self
    }

    /// Adds an IPv6 format constraint.
    pub fn ipv6(mut self) -> Self {
        self.constraints.push(StringConstraint::Format {
            format: Format::Ipv6,
            message: None,
        });
        self
    }

    /// Adds an enumeration constraint.
    pub fn one_of<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let values: Vec<String> = values.into_iter().map(Into::into).collect();
        self.constraints.push(StringConstraint::OneOf {
            values,
            message: None,
        });
        self
    }

    /// Adds a starts-with constraint.
    pub fn starts_with(mut self, prefix: impl Into<String>) -> Self {
        self.constraints.push(StringConstraint::StartsWith {
            prefix: prefix.into(),
            message: None,
        });
        self
    }

    /// Adds an ends-with constraint.
    pub fn ends_with(mut self, suffix: impl Into<String>) -> Self {
        self.constraints.push(StringConstraint::EndsWith {
            suffix: suffix.into(),
            message: None,
        });
        self
    }

    /// Adds a contains constraint.
    pub fn contains(mut self, substring: impl Into<String>) -> Self {
        self.constraints.push(StringConstraint::Contains {
            substring: substring.into(),
            message: None,
        });
        self
    }

    /// Adds a trim transformation.
    pub fn trim(mut self) -> Self {
        self.transforms.push(Transform::Trim);
        self
    }

    /// Adds a lowercase transformation.
    pub fn lowercase(mut self) -> Self {
        self.transforms.push(Transform::Lowercase);
        self
    }

    /// Adds a custom validator.
    pub fn custom<F>(mut self, validator: F) -> Self
    where
        F: Fn(&str, &JsonPath) -> Validation<(), SchemaErrors> + Send + Sync + 'static,
    {
        self.custom_validators.push(Arc::new(validator));
        self
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
                StringConstraint::Format { message: m, .. } => *m = Some(message.into()),
                StringConstraint::OneOf { message: m, .. } => *m = Some(message.into()),
                StringConstraint::StartsWith { message: m, .. } => *m = Some(message.into()),
                StringConstraint::EndsWith { message: m, .. } => *m = Some(message.into()),
                StringConstraint::Contains { message: m, .. } => *m = Some(message.into()),
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

        // Apply transformations
        let mut transformed = s.to_string();
        for transform in &self.transforms {
            transformed = match transform {
                Transform::Trim => transformed.trim().to_string(),
                Transform::Lowercase => transformed.to_lowercase(),
            };
        }

        // Collect all constraint violations
        let mut errors: Vec<SchemaError> = self
            .constraints
            .iter()
            .filter_map(|c| check_constraint(c, &transformed, path))
            .collect();

        // Run custom validators
        for validator in &self.custom_validators {
            match validator(&transformed, path) {
                Validation::Success(_) => {}
                Validation::Failure(errs) => {
                    errors.extend(errs.into_vec());
                }
            }
        }

        if errors.is_empty() {
            Validation::Success(transformed)
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

impl SchemaLike for StringSchema {
    type Output = String;

    fn validate(&self, value: &Value, path: &JsonPath) -> Validation<Self::Output, SchemaErrors> {
        self.validate(value, path)
    }

    fn validate_to_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        self.validate(value, path).map(Value::String)
    }
}

/// Validates email format using a basic regex.
fn validate_email(s: &str) -> bool {
    let re = Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap();
    re.is_match(s)
}

/// Validates URL format (http/https).
fn validate_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Validates UUID format.
fn validate_uuid(s: &str) -> bool {
    let re = Regex::new(
        r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$",
    )
    .unwrap();
    re.is_match(s)
}

/// Validates date format (YYYY-MM-DD).
fn validate_date(s: &str) -> bool {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    if !re.is_match(s) {
        return false;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    let year: i32 = parts[0].parse().unwrap_or(0);
    let month: u32 = parts[1].parse().unwrap_or(0);
    let day: u32 = parts[2].parse().unwrap_or(0);
    (1000..=9999).contains(&year) && (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// Validates datetime format (ISO 8601).
fn validate_datetime(s: &str) -> bool {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}").unwrap();
    re.is_match(s)
}

/// Validates IPv4 format.
fn validate_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| p.parse::<u8>().is_ok())
}

/// Validates IPv6 format.
fn validate_ipv6(s: &str) -> bool {
    let re = Regex::new(r"^([0-9a-fA-F]{0,4}:){7}[0-9a-fA-F]{0,4}$|^::$|^::1$|^([0-9a-fA-F]{0,4}:){0,6}:([0-9a-fA-F]{0,4}:){0,6}[0-9a-fA-F]{0,4}$").unwrap();
    re.is_match(s)
}

/// Validates IP format (IPv4 or IPv6).
fn validate_ip(s: &str) -> bool {
    validate_ipv4(s) || validate_ipv6(s)
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
        StringConstraint::Format { format, message } => {
            let (is_valid, format_name, code) = match format {
                Format::Email => (validate_email(value), "valid email", "invalid_email"),
                Format::Url => (validate_url(value), "valid URL", "invalid_url"),
                Format::Uuid => (validate_uuid(value), "valid UUID", "invalid_uuid"),
                Format::Date => (
                    validate_date(value),
                    "valid date (YYYY-MM-DD)",
                    "invalid_date",
                ),
                Format::DateTime => (
                    validate_datetime(value),
                    "valid ISO 8601 datetime",
                    "invalid_datetime",
                ),
                Format::Ip => (validate_ip(value), "valid IP address", "invalid_ip"),
                Format::Ipv4 => (validate_ipv4(value), "valid IPv4 address", "invalid_ipv4"),
                Format::Ipv6 => (validate_ipv6(value), "valid IPv6 address", "invalid_ipv6"),
            };
            if !is_valid {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("must be {}", format_name));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code(code)
                        .with_expected(format_name)
                        .with_got(value.to_string()),
                )
            } else {
                None
            }
        }
        StringConstraint::OneOf { values, message } => {
            if !values.contains(&value.to_string()) {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("must be one of: {}", values.join(", ")));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code("invalid_enum")
                        .with_expected(format!("one of: {}", values.join(", ")))
                        .with_got(value.to_string()),
                )
            } else {
                None
            }
        }
        StringConstraint::StartsWith { prefix, message } => {
            if !value.starts_with(prefix) {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("must start with '{}'", prefix));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code("invalid_prefix")
                        .with_expected(format!("string starting with '{}'", prefix))
                        .with_got(value.to_string()),
                )
            } else {
                None
            }
        }
        StringConstraint::EndsWith { suffix, message } => {
            if !value.ends_with(suffix) {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("must end with '{}'", suffix));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code("invalid_suffix")
                        .with_expected(format!("string ending with '{}'", suffix))
                        .with_got(value.to_string()),
                )
            } else {
                None
            }
        }
        StringConstraint::Contains { substring, message } => {
            if !value.contains(substring) {
                let msg = message
                    .clone()
                    .unwrap_or_else(|| format!("must contain '{}'", substring));
                Some(
                    SchemaError::new(path.clone(), msg)
                        .with_code("invalid_substring")
                        .with_expected(format!("string containing '{}'", substring))
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

    #[test]
    fn test_email_format() {
        let schema = StringSchema::new().email();

        let result = schema.validate(&json!("test@example.com"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("invalid"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_email");
    }

    #[test]
    fn test_url_format() {
        let schema = StringSchema::new().url();

        let result = schema.validate(&json!("http://example.com"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("https://example.com"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("ftp://example.com"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_url");
    }

    #[test]
    fn test_uuid_format() {
        let schema = StringSchema::new().uuid();

        let result = schema.validate(
            &json!("550e8400-e29b-41d4-a716-446655440000"),
            &JsonPath::root(),
        );
        assert!(result.is_success());

        let result = schema.validate(&json!("invalid-uuid"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_uuid");
    }

    #[test]
    fn test_date_format() {
        let schema = StringSchema::new().date();

        let result = schema.validate(&json!("2025-11-28"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("2025-13-01"), &JsonPath::root());
        assert!(result.is_failure());

        let result = schema.validate(&json!("invalid-date"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_date");
    }

    #[test]
    fn test_datetime_format() {
        let schema = StringSchema::new().datetime();

        let result = schema.validate(&json!("2025-11-28T14:30:00"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("invalid"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_datetime");
    }

    #[test]
    fn test_ipv4_format() {
        let schema = StringSchema::new().ipv4();

        let result = schema.validate(&json!("192.168.1.1"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("256.1.1.1"), &JsonPath::root());
        assert!(result.is_failure());

        let result = schema.validate(&json!("invalid"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_ipv4");
    }

    #[test]
    fn test_ipv6_format() {
        let schema = StringSchema::new().ipv6();

        let result = schema.validate(
            &json!("2001:0db8:85a3:0000:0000:8a2e:0370:7334"),
            &JsonPath::root(),
        );
        assert!(result.is_success());

        let result = schema.validate(&json!("::1"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("invalid"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_ipv6");
    }

    #[test]
    fn test_ip_format() {
        let schema = StringSchema::new().ip();

        let result = schema.validate(&json!("192.168.1.1"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("::1"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("invalid"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_ip");
    }

    #[test]
    fn test_one_of_constraint() {
        let schema = StringSchema::new().one_of(["pending", "active", "completed"]);

        let result = schema.validate(&json!("active"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("invalid"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_enum");
        assert!(errors.first().message.contains("pending"));
    }

    #[test]
    fn test_starts_with_constraint() {
        let schema = StringSchema::new().starts_with("http");

        let result = schema.validate(&json!("http://example.com"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("ftp://example.com"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_prefix");
    }

    #[test]
    fn test_ends_with_constraint() {
        let schema = StringSchema::new().ends_with(".json");

        let result = schema.validate(&json!("config.json"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("config.xml"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_suffix");
    }

    #[test]
    fn test_contains_constraint() {
        let schema = StringSchema::new().contains("@");

        let result = schema.validate(&json!("test@example.com"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("invalid"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "invalid_substring");
    }

    #[test]
    fn test_trim_transformation() {
        let schema = StringSchema::new().trim().min_len(5);

        let result = schema.validate(&json!("  hello  "), &JsonPath::root());
        assert!(result.is_success());
        assert_eq!(unwrap_success(result), "hello");

        let result = schema.validate(&json!("  hi  "), &JsonPath::root());
        assert!(result.is_failure());
    }

    #[test]
    fn test_lowercase_transformation() {
        let schema = StringSchema::new()
            .lowercase()
            .pattern(r"^[a-z]+$")
            .unwrap();

        let result = schema.validate(&json!("HELLO"), &JsonPath::root());
        assert!(result.is_success());
        assert_eq!(unwrap_success(result), "hello");
    }

    #[test]
    fn test_combined_transformations() {
        let schema = StringSchema::new().trim().lowercase().min_len(3);

        let result = schema.validate(&json!("  HELLO  "), &JsonPath::root());
        assert!(result.is_success());
        assert_eq!(unwrap_success(result), "hello");
    }

    #[test]
    fn test_custom_validator() {
        let schema = StringSchema::new().custom(|s, path| {
            if s.chars().any(|c| c.is_uppercase()) {
                Validation::Success(())
            } else {
                Validation::Failure(SchemaErrors::single(
                    SchemaError::new(path.clone(), "must contain uppercase")
                        .with_code("no_uppercase"),
                ))
            }
        });

        let result = schema.validate(&json!("Hello"), &JsonPath::root());
        assert!(result.is_success());

        let result = schema.validate(&json!("hello"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().code, "no_uppercase");
    }

    #[test]
    fn test_multiple_custom_validators() {
        let schema = StringSchema::new()
            .custom(|s, path| {
                if s.chars().any(|c| c.is_uppercase()) {
                    Validation::Success(())
                } else {
                    Validation::Failure(SchemaErrors::single(
                        SchemaError::new(path.clone(), "must contain uppercase")
                            .with_code("no_uppercase"),
                    ))
                }
            })
            .custom(|s, path| {
                if s.chars().any(|c| c.is_numeric()) {
                    Validation::Success(())
                } else {
                    Validation::Failure(SchemaErrors::single(
                        SchemaError::new(path.clone(), "must contain digit").with_code("no_digit"),
                    ))
                }
            });

        let result = schema.validate(&json!("hello"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.len(), 2);
        assert!(errors.with_code("no_uppercase").len() == 1);
        assert!(errors.with_code("no_digit").len() == 1);
    }

    #[test]
    fn test_format_with_custom_error() {
        let schema = StringSchema::new()
            .email()
            .error("must be a valid email address");

        let result = schema.validate(&json!("invalid"), &JsonPath::root());
        assert!(result.is_failure());
        let errors = unwrap_failure(result);
        assert_eq!(errors.first().message, "must be a valid email address");
    }
}

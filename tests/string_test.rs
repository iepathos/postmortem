//! Integration tests for string schema validation.

use postmortem::{JsonPath, Schema, SchemaErrors};
use serde_json::json;

/// Helper to extract the success value from a Validation
fn unwrap_success<T, E: std::fmt::Debug>(v: stillwater::Validation<T, E>) -> T {
    v.into_result().unwrap()
}

/// Helper to extract the error value from a Validation
fn unwrap_failure<T, E>(v: stillwater::Validation<T, E>) -> E
where
    T: std::fmt::Debug,
{
    v.into_result().unwrap_err()
}

#[test]
fn test_schema_string_factory() {
    let schema = Schema::string();
    let result = schema.validate(&json!("test"), &JsonPath::root());
    assert!(result.is_success());
}

#[test]
fn test_min_len_rejects_short_strings() {
    let schema = Schema::string().min_len(5);

    // Exactly 5 characters - should pass
    let result = schema.validate(&json!("hello"), &JsonPath::root());
    assert!(result.is_success());
    assert_eq!(unwrap_success(result), "hello");

    // 4 characters - should fail
    let result = schema.validate(&json!("test"), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "min_length");
}

#[test]
fn test_max_len_rejects_long_strings() {
    let schema = Schema::string().max_len(10);

    // Exactly 10 characters - should pass
    let result = schema.validate(&json!("1234567890"), &JsonPath::root());
    assert!(result.is_success());

    // 11 characters - should fail
    let result = schema.validate(&json!("12345678901"), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "max_length");
}

#[test]
fn test_combined_min_max_len() {
    let schema = Schema::string().min_len(5).max_len(10);

    // Within range
    let result = schema.validate(&json!("hello"), &JsonPath::root());
    assert!(result.is_success());

    let result = schema.validate(&json!("1234567890"), &JsonPath::root());
    assert!(result.is_success());

    // Below minimum
    let result = schema.validate(&json!("hi"), &JsonPath::root());
    assert!(result.is_failure());

    // Above maximum
    let result = schema.validate(&json!("this is too long"), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_both_length_violations_reported() {
    // 2 chars with min 5 and max 10
    let schema = Schema::string().min_len(5).max_len(10);
    let result = schema.validate(&json!("ab"), &JsonPath::root());

    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    // Should report min_length violation
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.first().code, "min_length");
}

#[test]
fn test_pattern_validates_regex() {
    let schema = Schema::string().pattern(r"^\d+$").unwrap();

    // Digits only - should pass
    let result = schema.validate(&json!("12345"), &JsonPath::root());
    assert!(result.is_success());

    // Contains letters - should fail
    let result = schema.validate(&json!("abc123"), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "pattern");
}

#[test]
fn test_pattern_error_includes_pattern() {
    let schema = Schema::string().pattern(r"^\d+$").unwrap();
    let result = schema.validate(&json!("abc"), &JsonPath::root());

    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    // Error message should include the pattern
    assert!(errors.first().message.contains(r"^\d+$"));
}

#[test]
fn test_custom_error_message() {
    let schema = Schema::string()
        .min_len(5)
        .error("username must be at least 5 characters");

    let result = schema.validate(&json!("ab"), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(
        errors.first().message,
        "username must be at least 5 characters"
    );
}

#[test]
fn test_non_string_produces_invalid_type() {
    let schema = Schema::string();

    // Number
    let result = schema.validate(&json!(42), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "invalid_type");
    assert_eq!(errors.first().got, Some("number".to_string()));
    assert_eq!(errors.first().expected, Some("string".to_string()));

    // Boolean
    let result = schema.validate(&json!(true), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "invalid_type");

    // Null
    let result = schema.validate(&json!(null), &JsonPath::root());
    assert!(result.is_failure());

    // Array
    let result = schema.validate(&json!([1, 2, 3]), &JsonPath::root());
    assert!(result.is_failure());

    // Object
    let result = schema.validate(&json!({"key": "value"}), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_constraint_error_accumulation() {
    let schema = Schema::string().min_len(10).pattern(r"^\d+$").unwrap();

    // "abc" is both too short AND doesn't match the pattern
    let result = schema.validate(&json!("abc"), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);

    // Should have both errors
    assert_eq!(errors.len(), 2);
    assert!(errors.with_code("min_length").len() == 1);
    assert!(errors.with_code("pattern").len() == 1);
}

#[test]
fn test_validated_string_returned_on_success() {
    let schema = Schema::string().min_len(1).max_len(100);
    let result = schema.validate(&json!("hello"), &JsonPath::root());

    assert!(result.is_success());
    assert_eq!(unwrap_success(result), "hello");
}

#[test]
fn test_path_included_in_errors() {
    let schema = Schema::string().min_len(5);
    let path = JsonPath::root()
        .push_field("users")
        .push_index(0)
        .push_field("name");

    let result = schema.validate(&json!("ab"), &path);
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().path.to_string(), "users[0].name");
}

#[test]
fn test_empty_string_validation() {
    let schema = Schema::string().min_len(1);

    let result = schema.validate(&json!(""), &JsonPath::root());
    assert!(result.is_failure());

    // Empty string with no constraints should pass
    let schema = Schema::string();
    let result = schema.validate(&json!(""), &JsonPath::root());
    assert!(result.is_success());
}

#[test]
fn test_unicode_character_counting() {
    // Unicode strings should count characters (Unicode scalar values), not bytes
    let schema = Schema::string().min_len(3).max_len(5);

    // "日本語" is 3 characters (9 bytes)
    let result = schema.validate(&json!("日本語"), &JsonPath::root());
    assert!(result.is_success());

    // "🎉🎊" is 2 characters (8 bytes) - should fail min_len(3)
    let result = schema.validate(&json!("🎉🎊"), &JsonPath::root());
    assert!(result.is_failure());

    // "日本語です" is 5 characters - should pass max_len(5)
    let result = schema.validate(&json!("日本語です"), &JsonPath::root());
    assert!(result.is_success());

    // "日本語ですね" is 6 characters - should fail max_len(5)
    let result = schema.validate(&json!("日本語ですね"), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_email_like_pattern() {
    let schema = Schema::string()
        .pattern(r"@")
        .unwrap()
        .error("must contain @");

    let result = schema.validate(&json!("user@example.com"), &JsonPath::root());
    assert!(result.is_success());

    let result = schema.validate(&json!("not-an-email"), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().message, "must contain @");
}

#[test]
fn test_multiple_custom_errors() {
    let schema = Schema::string()
        .min_len(5)
        .error("too short")
        .max_len(10)
        .error("too long");

    // Test too short
    let result = schema.validate(&json!("ab"), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().message, "too short");

    // Test too long
    let result = schema.validate(&json!("this is way too long"), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().message, "too long");
}

#[test]
fn test_complex_validation_scenario() {
    // Username: 3-20 characters, alphanumeric only
    let schema = Schema::string()
        .min_len(3)
        .error("username must be at least 3 characters")
        .max_len(20)
        .error("username must be at most 20 characters")
        .pattern(r"^[a-zA-Z0-9]+$")
        .unwrap()
        .error("username can only contain letters and numbers");

    // Valid username
    let result = schema.validate(&json!("john123"), &JsonPath::root());
    assert!(result.is_success());

    // Invalid: too short and contains special char
    let result = schema.validate(&json!("a@"), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    // Should have both errors
    assert_eq!(errors.len(), 2);
}

#[allow(dead_code)]
fn assert_errors_contain(errors: &SchemaErrors, messages: &[&str]) {
    for msg in messages {
        assert!(
            errors.iter().any(|e| e.message.contains(msg)),
            "Expected error containing '{}' but not found in {:?}",
            msg,
            errors.iter().map(|e| &e.message).collect::<Vec<_>>()
        );
    }
}

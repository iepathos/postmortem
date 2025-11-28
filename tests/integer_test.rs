//! Integration tests for integer schema validation.

use postmortem::{JsonPath, Schema};
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
fn test_schema_integer_factory() {
    let schema = Schema::integer();
    let result = schema.validate(&json!(42), &JsonPath::root());
    assert!(result.is_success());
}

#[test]
fn test_min_rejects_integers_less_than_min() {
    let schema = Schema::integer().min(5);

    // Exactly 5 - should pass
    let result = schema.validate(&json!(5), &JsonPath::root());
    assert!(result.is_success());
    assert_eq!(unwrap_success(result), 5);

    // Greater than 5 - should pass
    let result = schema.validate(&json!(10), &JsonPath::root());
    assert!(result.is_success());

    // Less than 5 - should fail
    let result = schema.validate(&json!(4), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "min_value");
}

#[test]
fn test_max_rejects_integers_greater_than_max() {
    let schema = Schema::integer().max(10);

    // Exactly 10 - should pass
    let result = schema.validate(&json!(10), &JsonPath::root());
    assert!(result.is_success());

    // Less than 10 - should pass
    let result = schema.validate(&json!(5), &JsonPath::root());
    assert!(result.is_success());

    // Greater than 10 - should fail
    let result = schema.validate(&json!(11), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "max_value");
}

#[test]
fn test_range_validates_both_min_and_max() {
    let schema = Schema::integer().range(5..=10);

    // At lower bound
    let result = schema.validate(&json!(5), &JsonPath::root());
    assert!(result.is_success());

    // In range
    let result = schema.validate(&json!(7), &JsonPath::root());
    assert!(result.is_success());

    // At upper bound
    let result = schema.validate(&json!(10), &JsonPath::root());
    assert!(result.is_success());

    // Below range
    let result = schema.validate(&json!(4), &JsonPath::root());
    assert!(result.is_failure());

    // Above range
    let result = schema.validate(&json!(11), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_both_range_violations_reported_for_value_outside_range() {
    // Create an impossible constraint where min > max
    // A value of 7 would be: < 10 (violates min) but > 5 (violates max)
    let schema = Schema::integer().min(10).max(5);

    // Value of 7: 7 < 10 (violates min), 7 > 5 (violates max)
    let result = schema.validate(&json!(7), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    // Should have both violations
    assert_eq!(errors.len(), 2);
    assert!(errors.with_code("min_value").len() == 1);
    assert!(errors.with_code("max_value").len() == 1);
}

#[test]
fn test_positive_rejects_zero_and_negatives() {
    let schema = Schema::integer().positive();

    // Positive - should pass
    let result = schema.validate(&json!(1), &JsonPath::root());
    assert!(result.is_success());

    let result = schema.validate(&json!(100), &JsonPath::root());
    assert!(result.is_success());

    // Zero - should fail
    let result = schema.validate(&json!(0), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "positive");

    // Negative - should fail
    let result = schema.validate(&json!(-1), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_non_negative_accepts_zero_rejects_negatives() {
    let schema = Schema::integer().non_negative();

    // Zero - should pass
    let result = schema.validate(&json!(0), &JsonPath::root());
    assert!(result.is_success());

    // Positive - should pass
    let result = schema.validate(&json!(5), &JsonPath::root());
    assert!(result.is_success());

    // Negative - should fail
    let result = schema.validate(&json!(-1), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "non_negative");
}

#[test]
fn test_negative_rejects_zero_and_positives() {
    let schema = Schema::integer().negative();

    // Negative - should pass
    let result = schema.validate(&json!(-1), &JsonPath::root());
    assert!(result.is_success());

    let result = schema.validate(&json!(-100), &JsonPath::root());
    assert!(result.is_success());

    // Zero - should fail
    let result = schema.validate(&json!(0), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "negative");

    // Positive - should fail
    let result = schema.validate(&json!(1), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_custom_error_message() {
    let schema = Schema::integer()
        .min(18)
        .error("must be at least 18 years old");

    let result = schema.validate(&json!(16), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().message, "must be at least 18 years old");
}

#[test]
fn test_non_integer_produces_invalid_type() {
    let schema = Schema::integer();

    // String
    let result = schema.validate(&json!("42"), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "invalid_type");
    assert_eq!(errors.first().got, Some("string".to_string()));
    assert_eq!(errors.first().expected, Some("integer".to_string()));

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
fn test_float_produces_invalid_type() {
    let schema = Schema::integer();

    // Float with decimal
    let result = schema.validate(&json!(1.5), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "invalid_type");
    assert_eq!(errors.first().got, Some("float".to_string()));

    // Float with zero decimal (1.0 is still parsed as float by serde_json)
    let result = schema.validate(&json!(1.0), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "invalid_type");
}

#[test]
fn test_constraint_error_accumulation() {
    let schema = Schema::integer().min(10).positive();

    // -5 violates both min (< 10) and positive (< 0)
    let result = schema.validate(&json!(-5), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);

    // Should have both errors
    assert_eq!(errors.len(), 2);
    assert!(errors.with_code("min_value").len() == 1);
    assert!(errors.with_code("positive").len() == 1);
}

#[test]
fn test_validated_integer_returned_on_success() {
    let schema = Schema::integer().min(0).max(100);
    let result = schema.validate(&json!(50), &JsonPath::root());

    assert!(result.is_success());
    assert_eq!(unwrap_success(result), 50);
}

#[test]
fn test_path_included_in_errors() {
    let schema = Schema::integer().min(5);
    let path = JsonPath::root()
        .push_field("users")
        .push_index(0)
        .push_field("age");

    let result = schema.validate(&json!(3), &path);
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().path.to_string(), "users[0].age");
}

#[test]
fn test_i64_min_max_values() {
    let schema = Schema::integer();

    // i64::MIN
    let result = schema.validate(&json!(i64::MIN), &JsonPath::root());
    assert!(result.is_success());
    assert_eq!(unwrap_success(result), i64::MIN);

    // i64::MAX
    let result = schema.validate(&json!(i64::MAX), &JsonPath::root());
    assert!(result.is_success());
    assert_eq!(unwrap_success(result), i64::MAX);
}

#[test]
fn test_zero_handling_for_sign_constraints() {
    // positive() rejects 0
    let schema = Schema::integer().positive();
    let result = schema.validate(&json!(0), &JsonPath::root());
    assert!(result.is_failure());

    // non_negative() accepts 0
    let schema = Schema::integer().non_negative();
    let result = schema.validate(&json!(0), &JsonPath::root());
    assert!(result.is_success());

    // negative() rejects 0
    let schema = Schema::integer().negative();
    let result = schema.validate(&json!(0), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_multiple_custom_errors() {
    let schema = Schema::integer()
        .min(0)
        .error("must be non-negative")
        .max(100)
        .error("must be at most 100");

    // Test below minimum
    let result = schema.validate(&json!(-5), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().message, "must be non-negative");

    // Test above maximum
    let result = schema.validate(&json!(150), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().message, "must be at most 100");
}

#[test]
fn test_age_validation_scenario() {
    // Age validation (0-150)
    let schema = Schema::integer()
        .non_negative()
        .max(150)
        .error("age must be between 0 and 150");

    // Valid age
    let result = schema.validate(&json!(25), &JsonPath::root());
    assert!(result.is_success());

    // Negative age
    let result = schema.validate(&json!(-5), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_pagination_scenario() {
    // Page number (must be positive)
    let page_schema = Schema::integer().positive().error("page must be positive");

    let result = page_schema.validate(&json!(1), &JsonPath::root());
    assert!(result.is_success());

    let result = page_schema.validate(&json!(0), &JsonPath::root());
    assert!(result.is_failure());

    // Limit (1-100)
    let limit_schema = Schema::integer()
        .range(1..=100)
        .error("limit must be between 1 and 100");

    let result = limit_schema.validate(&json!(50), &JsonPath::root());
    assert!(result.is_success());

    let result = limit_schema.validate(&json!(0), &JsonPath::root());
    assert!(result.is_failure());

    let result = limit_schema.validate(&json!(150), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_equal_min_max_for_exact_value() {
    // Range with equal min/max for exact value validation
    let schema = Schema::integer().range(42..=42);

    let result = schema.validate(&json!(42), &JsonPath::root());
    assert!(result.is_success());

    let result = schema.validate(&json!(41), &JsonPath::root());
    assert!(result.is_failure());

    let result = schema.validate(&json!(43), &JsonPath::root());
    assert!(result.is_failure());
}

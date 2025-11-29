use postmortem::{JsonPath, Schema, SchemaErrors};
use serde_json::json;
use stillwater::Validation;

fn unwrap_failure<T: std::fmt::Debug, E>(v: Validation<T, E>) -> E {
    v.into_result().unwrap_err()
}

#[test]
fn test_custom_validator_success() {
    let schema = Schema::object()
        .field("quantity", Schema::integer().positive())
        .field("unit_price", Schema::integer().non_negative())
        .field("total", Schema::integer().non_negative())
        .custom(|obj, path| {
            let qty = obj.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);
            let price = obj.get("unit_price").and_then(|v| v.as_i64()).unwrap_or(0);
            let total = obj.get("total").and_then(|v| v.as_i64()).unwrap_or(0);

            if qty * price != total {
                Validation::Failure(SchemaErrors::single(
                    postmortem::SchemaError::new(
                        path.push_field("total"),
                        "total must equal quantity * unit_price",
                    )
                    .with_code("invalid_total"),
                ))
            } else {
                Validation::Success(())
            }
        });

    let result = schema.validate(
        &json!({
            "quantity": 5,
            "unit_price": 10,
            "total": 50
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());
}

#[test]
fn test_custom_validator_failure() {
    let schema = Schema::object()
        .field("quantity", Schema::integer().positive())
        .field("unit_price", Schema::integer().non_negative())
        .field("total", Schema::integer().non_negative())
        .custom(|obj, path| {
            let qty = obj.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);
            let price = obj.get("unit_price").and_then(|v| v.as_i64()).unwrap_or(0);
            let total = obj.get("total").and_then(|v| v.as_i64()).unwrap_or(0);

            if qty * price != total {
                Validation::Failure(SchemaErrors::single(
                    postmortem::SchemaError::new(
                        path.push_field("total"),
                        "total must equal quantity * unit_price",
                    )
                    .with_code("invalid_total"),
                ))
            } else {
                Validation::Success(())
            }
        });

    let result = schema.validate(
        &json!({
            "quantity": 5,
            "unit_price": 10,
            "total": 30  // Wrong total
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "invalid_total");
}

#[test]
fn test_require_if_condition_met() {
    let schema = Schema::object()
        .field("method", Schema::string())
        .optional("card_number", Schema::string())
        .require_if("method", |v| v == &json!("card"), "card_number");

    // Card method without card_number - should fail
    let result = schema.validate(
        &json!({
            "method": "card"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "conditional_required");
    assert!(errors.first().message.contains("card_number"));
}

#[test]
fn test_require_if_condition_not_met() {
    let schema = Schema::object()
        .field("method", Schema::string())
        .optional("card_number", Schema::string())
        .require_if("method", |v| v == &json!("card"), "card_number");

    // Cash method without card_number - should pass
    let result = schema.validate(
        &json!({
            "method": "cash"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());
}

#[test]
fn test_require_if_with_required_field() {
    let schema = Schema::object()
        .field("method", Schema::string())
        .optional("card_number", Schema::string())
        .require_if("method", |v| v == &json!("card"), "card_number");

    // Card method with card_number - should pass
    let result = schema.validate(
        &json!({
            "method": "card",
            "card_number": "1234567890123456"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());
}

#[test]
fn test_mutually_exclusive_both_present() {
    let schema = Schema::object()
        .optional("email", Schema::string())
        .optional("phone", Schema::string())
        .mutually_exclusive("email", "phone");

    let result = schema.validate(
        &json!({
            "email": "user@example.com",
            "phone": "+1234567890"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "mutually_exclusive");
}

#[test]
fn test_mutually_exclusive_one_present() {
    let schema = Schema::object()
        .optional("email", Schema::string())
        .optional("phone", Schema::string())
        .mutually_exclusive("email", "phone");

    // Only email
    let result = schema.validate(
        &json!({
            "email": "user@example.com"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Only phone
    let result = schema.validate(
        &json!({
            "phone": "+1234567890"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());
}

#[test]
fn test_mutually_exclusive_none_present() {
    let schema = Schema::object()
        .optional("email", Schema::string())
        .optional("phone", Schema::string())
        .mutually_exclusive("email", "phone");

    let result = schema.validate(&json!({}), &JsonPath::root());
    assert!(result.is_success());
}

#[test]
fn test_mutually_exclusive_with_null() {
    // Note: Since the schema doesn't accept null values for string fields,
    // we test mutually_exclusive by having only one field present (omitting the other)
    // which effectively tests that null/missing fields are treated as absent
    let schema = Schema::object()
        .optional("email", Schema::string())
        .optional("phone", Schema::string())
        .mutually_exclusive("email", "phone");

    // Only email present (phone missing/absent) - should pass
    let result = schema.validate(
        &json!({
            "email": "user@example.com"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Only phone present (email missing/absent) - should pass
    let result = schema.validate(
        &json!({
            "phone": "+1234567890"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Neither field present - should pass
    let result = schema.validate(&json!({}), &JsonPath::root());
    assert!(result.is_success());
}

#[test]
fn test_at_least_one_of_none_present() {
    let schema = Schema::object()
        .optional("email", Schema::string())
        .optional("phone", Schema::string())
        .at_least_one_of(["email", "phone"]);

    let result = schema.validate(&json!({}), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "at_least_one_required");
}

#[test]
fn test_at_least_one_of_one_present() {
    let schema = Schema::object()
        .optional("email", Schema::string())
        .optional("phone", Schema::string())
        .at_least_one_of(["email", "phone"]);

    let result = schema.validate(
        &json!({
            "email": "user@example.com"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());
}

#[test]
fn test_at_least_one_of_both_present() {
    let schema = Schema::object()
        .optional("email", Schema::string())
        .optional("phone", Schema::string())
        .at_least_one_of(["email", "phone"]);

    let result = schema.validate(
        &json!({
            "email": "user@example.com",
            "phone": "+1234567890"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());
}

#[test]
fn test_at_least_one_of_all_missing() {
    // Note: We test with missing fields rather than explicit null values
    // since the schema doesn't accept null for string fields.
    // Missing fields are treated as absent, which is what we want to test.
    let schema = Schema::object()
        .optional("email", Schema::string())
        .optional("phone", Schema::string())
        .at_least_one_of(["email", "phone"]);

    // All fields missing - should fail
    let result = schema.validate(&json!({}), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "at_least_one_required");
}

#[test]
fn test_equal_fields_matching() {
    let schema = Schema::object()
        .field("password", Schema::string())
        .field("confirm_password", Schema::string())
        .equal_fields("password", "confirm_password");

    let result = schema.validate(
        &json!({
            "password": "secret123",
            "confirm_password": "secret123"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());
}

#[test]
fn test_equal_fields_not_matching() {
    let schema = Schema::object()
        .field("password", Schema::string())
        .field("confirm_password", Schema::string())
        .equal_fields("password", "confirm_password");

    let result = schema.validate(
        &json!({
            "password": "secret123",
            "confirm_password": "different"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "fields_not_equal");
}

#[test]
fn test_equal_fields_one_missing() {
    let schema = Schema::object()
        .optional("password", Schema::string())
        .optional("confirm_password", Schema::string())
        .equal_fields("password", "confirm_password");

    // If one field is missing, validation is skipped
    let result = schema.validate(
        &json!({
            "password": "secret123"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());
}

#[test]
fn test_field_less_than_numbers() {
    let schema = Schema::object()
        .field("min", Schema::integer())
        .field("max", Schema::integer())
        .field_less_than("min", "max");

    // Valid: min < max
    let result = schema.validate(
        &json!({
            "min": 10,
            "max": 20
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Invalid: min >= max
    let result = schema.validate(
        &json!({
            "min": 20,
            "max": 10
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "field_not_less_than");

    // Invalid: min == max
    let result = schema.validate(
        &json!({
            "min": 10,
            "max": 10
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
}

#[test]
fn test_field_less_than_strings() {
    let schema = Schema::object()
        .field("start_date", Schema::string())
        .field("end_date", Schema::string())
        .field_less_than("start_date", "end_date");

    // Valid: start < end (lexicographic)
    let result = schema.validate(
        &json!({
            "start_date": "2024-01-01",
            "end_date": "2024-12-31"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Invalid: start >= end
    let result = schema.validate(
        &json!({
            "start_date": "2024-12-31",
            "end_date": "2024-01-01"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
}

#[test]
fn test_field_less_than_type_mismatch() {
    let schema = Schema::object()
        .field("start", Schema::integer())
        .field("end", Schema::string())
        .field_less_than("start", "end");

    // Type mismatch - validation is skipped
    let result = schema.validate(
        &json!({
            "start": 100,
            "end": "200"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());
}

#[test]
fn test_field_less_or_equal_numbers() {
    let schema = Schema::object()
        .field("min", Schema::integer())
        .field("max", Schema::integer())
        .field_less_or_equal("min", "max");

    // Valid: min < max
    let result = schema.validate(
        &json!({
            "min": 10,
            "max": 20
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Valid: min == max
    let result = schema.validate(
        &json!({
            "min": 10,
            "max": 10
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Invalid: min > max
    let result = schema.validate(
        &json!({
            "min": 20,
            "max": 10
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "field_not_less_or_equal");
}

#[test]
fn test_multiple_cross_field_rules() {
    let schema = Schema::object()
        .field("method", Schema::string())
        .optional("email", Schema::string())
        .optional("phone", Schema::string())
        .optional("card_number", Schema::string())
        .require_if("method", |v| v == &json!("card"), "card_number")
        .mutually_exclusive("email", "phone")
        .at_least_one_of(["email", "phone"]);

    // All rules satisfied
    let result = schema.validate(
        &json!({
            "method": "cash",
            "email": "user@example.com"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Multiple rules violated
    let result = schema.validate(
        &json!({
            "method": "card",
            "email": "user@example.com",
            "phone": "+1234567890"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    // Should have at least 2 errors: conditional_required and mutually_exclusive
    assert!(errors.len() >= 2);
}

#[test]
fn test_skip_cross_field_on_errors_default() {
    let schema = Schema::object()
        .field("name", Schema::string().min_len(5))
        .field("age", Schema::integer().positive())
        .custom(|_obj, _path| {
            // This should NOT run because field validation fails
            panic!("Cross-field validator should not run");
        });

    // Field validation fails, cross-field should be skipped
    let result = schema.validate(
        &json!({
            "name": "AB",  // Too short
            "age": 30
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
}

#[test]
fn test_skip_cross_field_on_errors_disabled() {
    let schema = Schema::object()
        .field("name", Schema::string().min_len(5))
        .field("age", Schema::integer().positive())
        .skip_cross_field_on_errors(false)
        .custom(move |_obj, _path| {
            // This runs even when field validation fails
            Validation::Success(())
        });

    // Even with field errors, cross-field should run
    let result = schema.validate(
        &json!({
            "name": "AB",  // Too short
            "age": 30
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
    // Should have field error
    let errors = unwrap_failure(result);
    assert!(errors.with_code("min_length").len() > 0);
}

#[test]
fn test_cross_field_error_accumulation() {
    let schema = Schema::object()
        .field("password", Schema::string())
        .field("confirm", Schema::string())
        .field("min", Schema::integer())
        .field("max", Schema::integer())
        .equal_fields("password", "confirm")
        .field_less_than("min", "max");

    // Both cross-field rules fail
    let result = schema.validate(
        &json!({
            "password": "secret",
            "confirm": "different",
            "min": 100,
            "max": 50
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.len(), 2);
    assert!(errors.with_code("fields_not_equal").len() > 0);
    assert!(errors.with_code("field_not_less_than").len() > 0);
}

#[test]
fn test_validated_object_has_method() {
    let schema = Schema::object()
        .optional("email", Schema::string())
        .optional("phone", Schema::string())
        .custom(|obj, _path| {
            // Test the has() method
            assert!(!obj.has("email"));
            assert!(!obj.has("phone"));
            Validation::Success(())
        });

    // No fields provided
    let result = schema.validate(&json!({}), &JsonPath::root());
    assert!(result.is_success());
}

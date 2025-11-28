//! Integration tests for array schema validation.

use postmortem::{JsonPath, Schema};
use serde_json::{json, Value};

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
fn test_schema_array_factory() {
    let schema = Schema::array(Schema::string());
    let result = schema.validate(&json!(["test"]), &JsonPath::root());
    assert!(result.is_success());
}

#[test]
fn test_array_of_strings() {
    let schema = Schema::array(Schema::string().min_len(1));

    let result = schema.validate(&json!(["hello", "world"]), &JsonPath::root());
    assert!(result.is_success());
    let items = unwrap_success(result);
    assert_eq!(items.len(), 2);
}

#[test]
fn test_array_of_integers() {
    let schema = Schema::array(Schema::integer().positive());

    let result = schema.validate(&json!([1, 2, 3]), &JsonPath::root());
    assert!(result.is_success());

    // Negative number should fail
    let result = schema.validate(&json!([1, -2, 3]), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "positive");
    assert_eq!(errors.first().path.to_string(), "[1]");
}

#[test]
fn test_array_of_objects() {
    let user_schema = Schema::object()
        .field("name", Schema::string().min_len(1))
        .field("age", Schema::integer().positive());

    let schema = Schema::array(user_schema);

    let result = schema.validate(
        &json!([
            {"name": "Alice", "age": 30},
            {"name": "Bob", "age": 25}
        ]),
        &JsonPath::root(),
    );
    assert!(result.is_success());
}

#[test]
fn test_min_len_rejects_short_arrays() {
    let schema = Schema::array(Schema::string()).min_len(3);

    // Exactly 3 items - should pass
    let result = schema.validate(&json!(["a", "b", "c"]), &JsonPath::root());
    assert!(result.is_success());

    // 2 items - should fail
    let result = schema.validate(&json!(["a", "b"]), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "min_length");
}

#[test]
fn test_max_len_rejects_long_arrays() {
    let schema = Schema::array(Schema::string()).max_len(3);

    // Exactly 3 items - should pass
    let result = schema.validate(&json!(["a", "b", "c"]), &JsonPath::root());
    assert!(result.is_success());

    // 4 items - should fail
    let result = schema.validate(&json!(["a", "b", "c", "d"]), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "max_length");
}

#[test]
fn test_non_empty_rejects_empty_arrays() {
    let schema = Schema::array(Schema::string()).non_empty();

    // One item - should pass
    let result = schema.validate(&json!(["a"]), &JsonPath::root());
    assert!(result.is_success());

    // Empty - should fail
    let result = schema.validate(&json!([]), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "min_length");
}

#[test]
fn test_unique_rejects_duplicates() {
    let schema = Schema::array(Schema::string()).unique();

    // All unique - should pass
    let result = schema.validate(&json!(["a", "b", "c"]), &JsonPath::root());
    assert!(result.is_success());

    // Has duplicates - should fail
    let result = schema.validate(&json!(["a", "b", "a"]), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "unique");
}

#[test]
fn test_unique_by_rejects_duplicates_by_key() {
    let user_schema = Schema::object()
        .field("id", Schema::integer())
        .field("name", Schema::string());

    let schema =
        Schema::array(user_schema).unique_by(|v| v.get("id").cloned().unwrap_or(Value::Null));

    // Unique IDs - should pass
    let result = schema.validate(
        &json!([
            {"id": 1, "name": "Alice"},
            {"id": 2, "name": "Bob"}
        ]),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Duplicate IDs - should fail
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

#[test]
fn test_non_array_produces_invalid_type() {
    let schema = Schema::array(Schema::string());

    // String
    let result = schema.validate(&json!("not an array"), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "invalid_type");
    assert_eq!(errors.first().got, Some("string".to_string()));
    assert_eq!(errors.first().expected, Some("array".to_string()));

    // Object
    let result = schema.validate(&json!({"key": "value"}), &JsonPath::root());
    assert!(result.is_failure());

    // Number
    let result = schema.validate(&json!(42), &JsonPath::root());
    assert!(result.is_failure());

    // Null
    let result = schema.validate(&json!(null), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_error_accumulation_multiple_invalid_items() {
    let schema = Schema::array(Schema::integer().positive());

    let result = schema.validate(&json!([-1, 2, -3, 4, -5]), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.len(), 3);

    // Check all paths are correct
    let paths: Vec<_> = errors.iter().map(|e| e.path.to_string()).collect();
    assert!(paths.contains(&"[0]".to_string()));
    assert!(paths.contains(&"[2]".to_string()));
    assert!(paths.contains(&"[4]".to_string()));
}

#[test]
fn test_error_accumulation_length_and_items() {
    let schema = Schema::array(Schema::integer().positive()).min_len(5);

    // Too short AND has invalid items
    let result = schema.validate(&json!([-1, -2]), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    // Should have: 1 min_length + 2 positive errors
    assert_eq!(errors.len(), 3);
    assert_eq!(errors.with_code("min_length").len(), 1);
    assert_eq!(errors.with_code("positive").len(), 2);
}

#[test]
fn test_nested_array_path_tracking() {
    let user_schema = Schema::object()
        .field("email", Schema::string().min_len(5))
        .field("age", Schema::integer().positive());

    let schema = Schema::array(user_schema);

    let result = schema.validate(
        &json!([
            {"email": "a@b", "age": 30},
            {"email": "valid@email.com", "age": -5}
        ]),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.len(), 2);

    let paths: Vec<_> = errors.iter().map(|e| e.path.to_string()).collect();
    assert!(paths.contains(&"[0].email".to_string()));
    assert!(paths.contains(&"[1].age".to_string()));
}

#[test]
fn test_deeply_nested_path_tracking() {
    let inner_schema = Schema::object().field("value", Schema::integer().positive());
    let array_schema = Schema::array(inner_schema);
    let outer_schema = Schema::object().field("items", array_schema);

    let result = outer_schema.validate(
        &json!({
            "items": [
                {"value": 1},
                {"value": -2},
                {"value": 3}
            ]
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().path.to_string(), "items[1].value");
}

#[test]
fn test_custom_error_messages() {
    let schema = Schema::array(Schema::string())
        .non_empty()
        .error("at least one tag required")
        .max_len(5)
        .error("maximum 5 tags allowed")
        .unique()
        .error("all tags must be unique");

    // Empty array
    let result = schema.validate(&json!([]), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().message, "at least one tag required");

    // Too many items
    let result = schema.validate(&json!(["a", "b", "c", "d", "e", "f"]), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().message, "maximum 5 tags allowed");

    // Duplicates
    let result = schema.validate(&json!(["a", "b", "a"]), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().message, "all tags must be unique");
}

#[test]
fn test_tags_validation_scenario() {
    // Real-world scenario: validating blog post tags
    let schema = Schema::array(Schema::string().min_len(1).max_len(30))
        .non_empty()
        .max_len(10)
        .unique();

    // Valid tags
    let result = schema.validate(&json!(["rust", "programming", "webdev"]), &JsonPath::root());
    assert!(result.is_success());

    // Empty tag should fail
    let result = schema.validate(&json!(["rust", "", "webdev"]), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().path.to_string(), "[1]");

    // Duplicate tags should fail
    let result = schema.validate(&json!(["rust", "programming", "rust"]), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_users_validation_scenario() {
    // Real-world scenario: validating a list of users
    let user_schema = Schema::object()
        .field("id", Schema::integer().positive())
        .field("name", Schema::string().min_len(1))
        .field("email", Schema::string().pattern(r"@").unwrap())
        .optional("age", Schema::integer().non_negative());

    let schema = Schema::array(user_schema)
        .non_empty()
        .unique_by(|v| v.get("id").cloned().unwrap_or(Value::Null));

    // Valid users
    let result = schema.validate(
        &json!([
            {"id": 1, "name": "Alice", "email": "alice@example.com"},
            {"id": 2, "name": "Bob", "email": "bob@example.com", "age": 30}
        ]),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Duplicate IDs should fail
    let result = schema.validate(
        &json!([
            {"id": 1, "name": "Alice", "email": "alice@example.com"},
            {"id": 1, "name": "Bob", "email": "bob@example.com"}
        ]),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().code, "unique");
}

#[test]
fn test_nested_arrays() {
    // Array of arrays
    let inner_schema = Schema::array(Schema::integer()).non_empty();
    let outer_schema = Schema::array(inner_schema);

    let result = outer_schema.validate(&json!([[1, 2], [3, 4, 5], [6]]), &JsonPath::root());
    assert!(result.is_success());

    // Inner array is empty
    let result = outer_schema.validate(&json!([[1, 2], [], [6]]), &JsonPath::root());
    assert!(result.is_failure());
    let errors = unwrap_failure(result);
    assert_eq!(errors.first().path.to_string(), "[1]");
}

#[test]
fn test_empty_array_with_no_constraints() {
    let schema = Schema::array(Schema::string());
    let result = schema.validate(&json!([]), &JsonPath::root());
    assert!(result.is_success());
    let items = unwrap_success(result);
    assert!(items.is_empty());
}

#[test]
fn test_validated_array_returned_on_success() {
    let schema = Schema::array(Schema::integer());
    let result = schema.validate(&json!([1, 2, 3]), &JsonPath::root());

    assert!(result.is_success());
    let items = unwrap_success(result);
    assert_eq!(items, vec![json!(1), json!(2), json!(3)]);
}

#[test]
fn test_unique_with_different_types() {
    // Unique constraint with integers
    let int_schema = Schema::array(Schema::integer()).unique();
    let result = int_schema.validate(&json!([1, 2, 2, 3]), &JsonPath::root());
    assert!(result.is_failure());

    // Unique constraint with objects (by value equality)
    let obj_schema = Schema::array(Schema::object()).unique();
    let result = obj_schema.validate(&json!([{"a": 1}, {"a": 1}]), &JsonPath::root());
    assert!(result.is_failure());
}

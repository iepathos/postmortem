use postmortem::{Schema, ToJsonSchema};
use serde_json::json;

#[test]
fn test_string_schema_to_json_schema() {
    let schema = Schema::string().min_len(5).max_len(100);
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "string");
    assert_eq!(json_schema["minLength"], 5);
    assert_eq!(json_schema["maxLength"], 100);
}

#[test]
fn test_string_schema_with_pattern() {
    let schema = Schema::string().pattern(r"^\d+$").unwrap();
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "string");
    assert_eq!(json_schema["pattern"], r"^\d+$");
}

#[test]
fn test_string_schema_with_format() {
    let schema = Schema::string().email();
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "string");
    assert_eq!(json_schema["format"], "email");
}

#[test]
fn test_string_schema_with_url_format() {
    let schema = Schema::string().url();
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "string");
    assert_eq!(json_schema["format"], "uri");
}

#[test]
fn test_string_schema_with_uuid_format() {
    let schema = Schema::string().uuid();
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "string");
    assert_eq!(json_schema["format"], "uuid");
}

#[test]
fn test_string_schema_with_date_format() {
    let schema = Schema::string().date();
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "string");
    assert_eq!(json_schema["format"], "date");
}

#[test]
fn test_string_schema_with_datetime_format() {
    let schema = Schema::string().datetime();
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "string");
    assert_eq!(json_schema["format"], "date-time");
}

#[test]
fn test_string_schema_with_enum() {
    let schema = Schema::string().one_of(["pending", "active", "completed"]);
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "string");
    assert_eq!(
        json_schema["enum"],
        json!(["pending", "active", "completed"])
    );
}

#[test]
fn test_integer_schema_to_json_schema() {
    let schema = Schema::integer().min(0).max(100);
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "integer");
    assert_eq!(json_schema["minimum"], 0);
    assert_eq!(json_schema["maximum"], 100);
}

#[test]
fn test_integer_schema_positive() {
    let schema = Schema::integer().positive();
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "integer");
    assert_eq!(json_schema["exclusiveMinimum"], 0);
}

#[test]
fn test_integer_schema_non_negative() {
    let schema = Schema::integer().non_negative();
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "integer");
    assert_eq!(json_schema["minimum"], 0);
}

#[test]
fn test_integer_schema_negative() {
    let schema = Schema::integer().negative();
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "integer");
    assert_eq!(json_schema["exclusiveMaximum"], 0);
}

#[test]
fn test_array_schema_to_json_schema() {
    let schema = Schema::array(Schema::string().min_len(1))
        .min_len(1)
        .max_len(10);
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "array");
    assert_eq!(json_schema["minItems"], 1);
    assert_eq!(json_schema["maxItems"], 10);
    assert_eq!(json_schema["items"]["type"], "string");
    assert_eq!(json_schema["items"]["minLength"], 1);
}

#[test]
fn test_array_schema_unique() {
    let schema = Schema::array(Schema::integer()).unique();
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "array");
    assert_eq!(json_schema["uniqueItems"], true);
    assert_eq!(json_schema["items"]["type"], "integer");
}

#[test]
fn test_ref_schema_to_json_schema() {
    let schema = Schema::ref_("UserId");
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["$ref"], "#/$defs/UserId");
}

#[test]
fn test_nested_array_schema() {
    let schema = Schema::array(Schema::array(Schema::integer().positive()));
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "array");
    assert_eq!(json_schema["items"]["type"], "array");
    assert_eq!(json_schema["items"]["items"]["type"], "integer");
    assert_eq!(json_schema["items"]["items"]["exclusiveMinimum"], 0);
}

#[test]
fn test_array_of_refs() {
    let schema = Schema::array(Schema::ref_("User"));
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "array");
    assert_eq!(json_schema["items"]["$ref"], "#/$defs/User");
}

#[test]
fn test_combined_constraints() {
    let schema = Schema::string()
        .min_len(5)
        .max_len(20)
        .pattern(r"^[a-z]+$")
        .unwrap();
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "string");
    assert_eq!(json_schema["minLength"], 5);
    assert_eq!(json_schema["maxLength"], 20);
    assert_eq!(json_schema["pattern"], r"^[a-z]+$");
}

#[test]
fn test_integer_with_multiple_constraints() {
    let schema = Schema::integer().min(10).max(100).positive();
    let json_schema = schema.to_json_schema();

    assert_eq!(json_schema["type"], "integer");
    // Should have min, max, and exclusiveMinimum
    assert_eq!(json_schema["minimum"], 10);
    assert_eq!(json_schema["maximum"], 100);
    assert_eq!(json_schema["exclusiveMinimum"], 0);
}

#[test]
fn test_generated_schema_is_valid_json() {
    let schema = Schema::string().email();
    let json_schema = schema.to_json_schema();

    // Should be valid JSON
    let serialized = serde_json::to_string(&json_schema).unwrap();
    assert!(!serialized.is_empty());

    // Should roundtrip
    let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(json_schema, deserialized);
}

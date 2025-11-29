use postmortem::{JsonPath, Schema, SchemaLike, ValueValidator};
use serde_json::json;
use stillwater::Validation;

// Helper to convert Box<dyn SchemaLike> to Box<dyn ValueValidator>
fn boxed<T: ValueValidator + 'static>(schema: T) -> Box<dyn ValueValidator> {
    Box::new(schema)
}

// ====== one_of Tests ======

#[test]
fn test_one_of_exactly_one_match() {
    let schema = Schema::one_of(vec![
        boxed(Schema::string().min_len(1)),
        boxed(Schema::integer().positive()),
    ]);

    // String matches first schema only
    let result = schema.validate(&json!("hello"), &JsonPath::root());
    assert!(result.is_success());

    // Integer matches second schema only
    let result = schema.validate(&json!(42), &JsonPath::root());
    assert!(result.is_success());
}

#[test]
fn test_one_of_no_matches() {
    let schema = Schema::one_of(vec![
        boxed(Schema::string().min_len(5)),
        boxed(Schema::integer().positive()),
    ]);

    // Empty string doesn't match either
    let result = schema.validate(&json!(""), &JsonPath::root());
    assert!(result.is_failure());

    if let Validation::Failure(errors) = result {
        let error = errors.iter().next().unwrap();
        assert_eq!(error.code, "one_of_none_matched");
        assert!(error.message.contains("did not match any of 2 schemas"));
    }
}

#[test]
fn test_one_of_multiple_matches() {
    // Both schemas accept strings
    let schema = Schema::one_of(vec![
        boxed(Schema::string()),
        boxed(Schema::string().min_len(1)),
    ]);

    // "hello" matches both schemas - ambiguous
    let result = schema.validate(&json!("hello"), &JsonPath::root());
    assert!(result.is_failure());

    if let Validation::Failure(errors) = result {
        let error = errors.iter().next().unwrap();
        assert_eq!(error.code, "one_of_multiple_matched");
        assert!(error.message.contains("matched 2 schemas"));
        assert!(error.message.contains("expected exactly one"));
    }
}

#[test]
fn test_one_of_discriminated_union() {
    // Discriminated union - circle vs rectangle
    let circle = Schema::object()
        .field("type", Schema::string())
        .field("radius", Schema::integer().positive());

    let rectangle = Schema::object()
        .field("type", Schema::string())
        .field("width", Schema::integer().positive())
        .field("height", Schema::integer().positive());

    let shape = Schema::one_of(vec![boxed(circle), boxed(rectangle)]);

    // Valid circle
    let result = shape.validate(
        &json!({
            "type": "circle",
            "radius": 5
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Valid rectangle
    let result = shape.validate(
        &json!({
            "type": "rectangle",
            "width": 10,
            "height": 20
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Invalid - missing required field
    let result = shape.validate(
        &json!({
            "type": "circle"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
}

// ====== any_of Tests ======

#[test]
fn test_any_of_first_match() {
    let schema = Schema::any_of(vec![
        boxed(Schema::string().min_len(1)),
        boxed(Schema::integer().positive()),
    ]);

    // String matches first schema
    let result = schema.validate(&json!("hello"), &JsonPath::root());
    assert!(result.is_success());
}

#[test]
fn test_any_of_later_match() {
    let schema = Schema::any_of(vec![
        boxed(Schema::string().min_len(1)),
        boxed(Schema::integer().positive()),
    ]);

    // Integer matches second schema
    let result = schema.validate(&json!(42), &JsonPath::root());
    assert!(result.is_success());
}

#[test]
fn test_any_of_no_matches() {
    let schema = Schema::any_of(vec![
        boxed(Schema::string().min_len(5)),
        boxed(Schema::integer().positive()),
    ]);

    // Empty string doesn't match either
    let result = schema.validate(&json!(""), &JsonPath::root());
    assert!(result.is_failure());

    if let Validation::Failure(errors) = result {
        let error = errors.iter().next().unwrap();
        assert_eq!(error.code, "any_of_none_matched");
        assert!(error.message.contains("did not match any of 2 schemas"));
    }
}

#[test]
fn test_any_of_flexible_id() {
    // ID can be string or integer
    let id = Schema::any_of(vec![
        boxed(Schema::string().min_len(1)),
        boxed(Schema::integer().positive()),
    ]);

    let result = id.validate(&json!("abc-123"), &JsonPath::root());
    assert!(result.is_success());

    let result = id.validate(&json!(42), &JsonPath::root());
    assert!(result.is_success());

    // Neither string nor positive integer
    let result = id.validate(&json!(-5), &JsonPath::root());
    assert!(result.is_failure());
}

// ====== all_of Tests ======

#[test]
fn test_all_of_all_passing() {
    let schema = Schema::all_of(vec![
        boxed(Schema::string()),
        boxed(Schema::string().min_len(1)),
        boxed(Schema::string().max_len(10)),
    ]);

    let result = schema.validate(&json!("hello"), &JsonPath::root());
    assert!(result.is_success());
}

#[test]
fn test_all_of_some_failing() {
    let schema = Schema::all_of(vec![
        boxed(Schema::string()),
        boxed(Schema::string().min_len(10)), // Fails - "hello" is 5 chars
        boxed(Schema::string().max_len(3)),  // Fails - "hello" is 5 chars
    ]);

    let result = schema.validate(&json!("hello"), &JsonPath::root());
    assert!(result.is_failure());

    // Should accumulate errors from both failing schemas
    if let Validation::Failure(errors) = result {
        assert_eq!(errors.len(), 2);
    }
}

#[test]
fn test_all_of_schema_composition() {
    let named = Schema::object().field("name", Schema::string().min_len(1));

    let timestamped = Schema::object().field("created_at", Schema::string());

    let entity = Schema::all_of(vec![boxed(named), boxed(timestamped)]);

    // Valid - has both name and created_at
    let result = entity.validate(
        &json!({
            "name": "Alice",
            "created_at": "2025-01-01"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Invalid - missing created_at
    let result = entity.validate(
        &json!({
            "name": "Alice"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());

    // Invalid - missing name
    let result = entity.validate(
        &json!({
            "created_at": "2025-01-01"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
}

// ====== optional Tests ======

#[test]
fn test_optional_with_null() {
    let schema = Schema::optional(boxed(Schema::string().min_len(1)));

    let result = schema.validate(&json!(null), &JsonPath::root());
    assert!(result.is_success());
}

#[test]
fn test_optional_with_valid_value() {
    let schema = Schema::optional(boxed(Schema::string().min_len(1)));

    let result = schema.validate(&json!("hello"), &JsonPath::root());
    assert!(result.is_success());
}

#[test]
fn test_optional_with_invalid_value() {
    let schema = Schema::optional(boxed(Schema::string().min_len(5)));

    // Empty string fails inner validation
    let result = schema.validate(&json!("hi"), &JsonPath::root());
    assert!(result.is_failure());
}

// ====== Nested Combinators ======

#[test]
fn test_nested_any_of_in_one_of() {
    // Shape can be circle (with flexible radius) or rectangle
    let flexible_number = Schema::any_of(vec![
        boxed(Schema::integer().positive()),
        boxed(Schema::string()), // Allow string numbers
    ]);

    let circle = Schema::object()
        .field("type", Schema::string())
        .field("radius", flexible_number);

    let rectangle = Schema::object()
        .field("type", Schema::string())
        .field("width", Schema::integer().positive())
        .field("height", Schema::integer().positive());

    let shape = Schema::one_of(vec![boxed(circle), boxed(rectangle)]);

    // Circle with integer radius
    let result = shape.validate(
        &json!({
            "type": "circle",
            "radius": 5
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Circle with string radius
    let result = shape.validate(
        &json!({
            "type": "circle",
            "radius": "5"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());
}

#[test]
fn test_optional_in_object() {
    // Object with optional field - can be omitted but if present must pass validation
    let user = Schema::object()
        .field("email", Schema::string())
        .optional("nickname", Schema::string().min_len(1));

    // Valid - nickname provided
    let result = user.validate(
        &json!({
            "email": "alice@example.com",
            "nickname": "alice"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Valid - nickname omitted
    let result = user.validate(
        &json!({
            "email": "alice@example.com"
        }),
        &JsonPath::root(),
    );
    assert!(result.is_success());

    // Invalid - nickname is null (not a string)
    let result = user.validate(
        &json!({
            "email": "alice@example.com",
            "nickname": null
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());

    // Invalid - nickname is empty string
    let result = user.validate(
        &json!({
            "email": "alice@example.com",
            "nickname": ""
        }),
        &JsonPath::root(),
    );
    assert!(result.is_failure());
}

// ====== Edge Cases ======

#[test]
fn test_one_of_single_schema() {
    let schema = Schema::one_of(vec![boxed(Schema::string())]);

    let result = schema.validate(&json!("hello"), &JsonPath::root());
    assert!(result.is_success());

    let result = schema.validate(&json!(42), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_any_of_single_schema() {
    let schema = Schema::any_of(vec![boxed(Schema::string())]);

    let result = schema.validate(&json!("hello"), &JsonPath::root());
    assert!(result.is_success());

    let result = schema.validate(&json!(42), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_all_of_single_schema() {
    let schema = Schema::all_of(vec![boxed(Schema::string())]);

    let result = schema.validate(&json!("hello"), &JsonPath::root());
    assert!(result.is_success());

    let result = schema.validate(&json!(42), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_deeply_nested_combinators() {
    // any_of containing all_of containing schemas
    let strict_string = Schema::all_of(vec![
        boxed(Schema::string()),
        boxed(Schema::string().min_len(3)),
        boxed(Schema::string().max_len(10)),
    ]);

    let value = Schema::any_of(vec![
        boxed(strict_string),
        boxed(Schema::integer().positive()),
    ]);

    // Valid string
    let result = value.validate(&json!("hello"), &JsonPath::root());
    assert!(result.is_success());

    // Valid integer
    let result = value.validate(&json!(42), &JsonPath::root());
    assert!(result.is_success());

    // Invalid - string too short
    let result = value.validate(&json!("hi"), &JsonPath::root());
    assert!(result.is_failure());

    // Invalid - neither valid string nor positive integer
    let result = value.validate(&json!(-5), &JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_combinator_error_paths() {
    let schema = Schema::object().field(
        "id",
        Schema::any_of(vec![
            boxed(Schema::string().min_len(1)),
            boxed(Schema::integer().positive()),
        ]),
    );

    let result = schema.validate(
        &json!({
            "id": -5
        }),
        &JsonPath::root(),
    );

    assert!(result.is_failure());
    if let Validation::Failure(errors) = result {
        let error = errors.iter().next().unwrap();
        assert_eq!(error.path.to_string(), "id");
    }
}

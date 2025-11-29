//! Tests for schema reference resolution in various contexts.

use postmortem::{Schema, SchemaLike, SchemaRegistry, ValueValidator};
use serde_json::json;

#[test]
fn test_ref_without_registry_fails() {
    let schema = Schema::ref_("UserId");
    let result = schema.validate(&json!(42), &postmortem::JsonPath::root());
    assert!(result.is_failure());
}

#[test]
fn test_ref_with_registry_succeeds() {
    let registry = SchemaRegistry::new();

    registry
        .register("UserId", Schema::integer().positive())
        .unwrap();

    let result = registry.validate("UserId", &json!(42)).unwrap();
    assert!(result.is_success());
}

#[test]
fn test_ref_in_object_field() {
    let registry = SchemaRegistry::new();

    registry
        .register("UserId", Schema::integer().positive())
        .unwrap();

    registry
        .register(
            "User",
            Schema::object()
                .field("id", Schema::ref_("UserId"))
                .field("name", Schema::string()),
        )
        .unwrap();

    let result = registry
        .validate(
            "User",
            &json!({
                "id": 42,
                "name": "Alice"
            }),
        )
        .unwrap();

    assert!(result.is_success());
}

#[test]
fn test_ref_in_one_of_combinator() {
    let registry = SchemaRegistry::new();

    registry
        .register("StringId", Schema::string().min_len(1))
        .unwrap();

    registry
        .register("IntegerId", Schema::integer().positive())
        .unwrap();

    registry
        .register(
            "Id",
            Schema::one_of(vec![
                Box::new(Schema::ref_("StringId")) as Box<dyn ValueValidator>,
                Box::new(Schema::ref_("IntegerId")) as Box<dyn ValueValidator>,
            ]),
        )
        .unwrap();

    let result = registry.validate("Id", &json!("abc-123")).unwrap();
    assert!(result.is_success());

    let result = registry.validate("Id", &json!(42)).unwrap();
    assert!(result.is_success());

    let result = registry.validate("Id", &json!(-5)).unwrap();
    assert!(result.is_failure());
}

#[test]
fn test_ref_in_any_of_combinator() {
    let registry = SchemaRegistry::new();

    registry.register("Email", Schema::string()).unwrap();

    registry.register("PhoneNumber", Schema::string()).unwrap();

    registry
        .register(
            "Contact",
            Schema::any_of(vec![
                Box::new(Schema::ref_("Email")) as Box<dyn ValueValidator>,
                Box::new(Schema::ref_("PhoneNumber")) as Box<dyn ValueValidator>,
            ]),
        )
        .unwrap();

    let result = registry
        .validate("Contact", &json!("test@example.com"))
        .unwrap();
    assert!(result.is_success());
}

#[test]
fn test_ref_in_all_of_combinator() {
    let registry = SchemaRegistry::new();

    registry
        .register("Named", Schema::object().field("name", Schema::string()))
        .unwrap();

    registry
        .register(
            "Timestamped",
            Schema::object().field("created_at", Schema::string()),
        )
        .unwrap();

    registry
        .register(
            "Entity",
            Schema::all_of(vec![
                Box::new(Schema::ref_("Named")) as Box<dyn ValueValidator>,
                Box::new(Schema::ref_("Timestamped")) as Box<dyn ValueValidator>,
            ]),
        )
        .unwrap();

    let result = registry
        .validate(
            "Entity",
            &json!({
                "name": "Test",
                "created_at": "2025-01-01"
            }),
        )
        .unwrap();

    assert!(result.is_success());
}

#[test]
fn test_ref_in_optional_combinator() {
    let registry = SchemaRegistry::new();

    registry.register("Email", Schema::string()).unwrap();

    registry
        .register(
            "OptionalEmail",
            Schema::optional(Box::new(Schema::ref_("Email")) as Box<dyn ValueValidator>),
        )
        .unwrap();

    let result = registry.validate("OptionalEmail", &json!(null)).unwrap();
    assert!(result.is_success());

    let result = registry
        .validate("OptionalEmail", &json!("test@example.com"))
        .unwrap();
    assert!(result.is_success());
}

#[test]
fn test_ref_in_array_items() {
    let registry = SchemaRegistry::new();

    registry
        .register("UserId", Schema::integer().positive())
        .unwrap();

    registry
        .register("UserIds", Schema::array(Schema::ref_("UserId")))
        .unwrap();

    let result = registry.validate("UserIds", &json!([1, 2, 3])).unwrap();
    assert!(result.is_success());

    let result = registry.validate("UserIds", &json!([1, -2, 3])).unwrap();
    assert!(result.is_failure());
}

#[test]
fn test_nested_combinator_refs() {
    let registry = SchemaRegistry::new();

    registry
        .register("StringId", Schema::string().min_len(1))
        .unwrap();

    registry
        .register("IntegerId", Schema::integer().positive())
        .unwrap();

    registry
        .register(
            "Id",
            Schema::any_of(vec![
                Box::new(Schema::ref_("StringId")) as Box<dyn ValueValidator>,
                Box::new(Schema::ref_("IntegerId")) as Box<dyn ValueValidator>,
            ]),
        )
        .unwrap();

    registry
        .register(
            "Entity",
            Schema::object()
                .field("id", Schema::ref_("Id"))
                .field("name", Schema::string()),
        )
        .unwrap();

    let result = registry
        .validate(
            "Entity",
            &json!({
                "id": "abc-123",
                "name": "Test"
            }),
        )
        .unwrap();
    assert!(result.is_success());

    let result = registry
        .validate(
            "Entity",
            &json!({
                "id": 42,
                "name": "Test"
            }),
        )
        .unwrap();
    assert!(result.is_success());
}

#[test]
fn test_collect_refs_from_combinators() {
    let schema = Schema::one_of(vec![
        Box::new(Schema::ref_("A")) as Box<dyn ValueValidator>,
        Box::new(Schema::ref_("B")) as Box<dyn ValueValidator>,
    ]);

    let mut refs = Vec::new();
    ValueValidator::collect_refs(&schema, &mut refs);

    refs.sort();
    assert_eq!(refs, vec!["A", "B"]);
}

#[test]
fn test_collect_refs_from_nested_combinators() {
    let inner = Schema::all_of(vec![
        Box::new(Schema::ref_("A")) as Box<dyn ValueValidator>,
        Box::new(Schema::ref_("B")) as Box<dyn ValueValidator>,
    ]);

    let outer = Schema::any_of(vec![
        Box::new(inner) as Box<dyn ValueValidator>,
        Box::new(Schema::ref_("C")) as Box<dyn ValueValidator>,
    ]);

    let mut refs = Vec::new();
    ValueValidator::collect_refs(&outer, &mut refs);

    refs.sort();
    assert_eq!(refs, vec!["A", "B", "C"]);
}

#[test]
fn test_ref_resolution_error() {
    let registry = SchemaRegistry::new();

    registry
        .register(
            "User",
            Schema::object().field("id", Schema::ref_("MissingId")),
        )
        .unwrap();

    let result = registry
        .validate(
            "User",
            &json!({
                "id": 42
            }),
        )
        .unwrap();

    assert!(result.is_failure());
}

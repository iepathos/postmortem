//! Tests for schema registry operations.

use postmortem::{Schema, SchemaRegistry};
use serde_json::json;

#[test]
fn test_register_and_get() {
    let registry = SchemaRegistry::new();

    registry
        .register("Email", Schema::string().min_len(1))
        .unwrap();

    let schema = registry.get("Email");
    assert!(schema.is_some());

    let missing = registry.get("Missing");
    assert!(missing.is_none());
}

#[test]
fn test_duplicate_registration_fails() {
    let registry = SchemaRegistry::new();

    registry.register("Email", Schema::string()).unwrap();

    let result = registry.register("Email", Schema::integer());
    assert!(result.is_err());
}

#[test]
fn test_validate_with_registry() {
    let registry = SchemaRegistry::new();

    registry
        .register(
            "User",
            Schema::object()
                .field("name", Schema::string().min_len(1))
                .field("age", Schema::integer().positive()),
        )
        .unwrap();

    let result = registry
        .validate(
            "User",
            &json!({
                "name": "Alice",
                "age": 30
            }),
        )
        .unwrap();

    assert!(result.is_success());
}

#[test]
fn test_validate_missing_schema() {
    let registry = SchemaRegistry::new();

    let result = registry.validate("Missing", &json!({}));
    assert!(result.is_err());
}

#[test]
fn test_max_depth_configuration() {
    let registry = SchemaRegistry::new().with_max_depth(50);

    registry.register("Simple", Schema::string()).unwrap();

    // Should still work normally
    let result = registry.validate("Simple", &json!("test")).unwrap();
    assert!(result.is_success());
}

#[test]
fn test_validate_refs_with_valid_references() {
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

    let unresolved = registry.validate_refs();
    assert!(unresolved.is_empty());
}

#[test]
fn test_validate_refs_with_missing_references() {
    let registry = SchemaRegistry::new();

    registry
        .register("User", Schema::object().field("id", Schema::ref_("UserId")))
        .unwrap();

    let unresolved = registry.validate_refs();
    assert_eq!(unresolved, vec!["UserId"]);
}

#[test]
fn test_validate_refs_with_multiple_missing() {
    let registry = SchemaRegistry::new();

    registry
        .register(
            "User",
            Schema::object()
                .field("id", Schema::ref_("UserId"))
                .field("role", Schema::ref_("Role")),
        )
        .unwrap();

    let mut unresolved = registry.validate_refs();
    unresolved.sort();
    assert_eq!(unresolved, vec!["Role", "UserId"]);
}

#[test]
fn test_registry_clone() {
    let registry = SchemaRegistry::new();

    registry.register("Email", Schema::string()).unwrap();

    let cloned = registry.clone();

    // Both should have access to the same schemas
    assert!(registry.get("Email").is_some());
    assert!(cloned.get("Email").is_some());
}

#[test]
fn test_validation_with_nested_refs() {
    let registry = SchemaRegistry::new();

    registry
        .register("UserId", Schema::integer().positive())
        .unwrap();

    registry.register("Email", Schema::string()).unwrap();

    registry
        .register(
            "User",
            Schema::object()
                .field("id", Schema::ref_("UserId"))
                .field("email", Schema::ref_("Email")),
        )
        .unwrap();

    let result = registry
        .validate(
            "User",
            &json!({
                "id": 42,
                "email": "test@example.com"
            }),
        )
        .unwrap();

    assert!(result.is_success());
}

#[test]
fn test_validation_with_invalid_nested_ref() {
    let registry = SchemaRegistry::new();

    registry
        .register("UserId", Schema::integer().positive())
        .unwrap();

    registry
        .register("User", Schema::object().field("id", Schema::ref_("UserId")))
        .unwrap();

    let result = registry
        .validate(
            "User",
            &json!({
                "id": -5
            }),
        )
        .unwrap();

    assert!(result.is_failure());
}

#[test]
fn test_default_registry() {
    let registry = SchemaRegistry::default();

    registry.register("Test", Schema::string()).unwrap();

    assert!(registry.get("Test").is_some());
}

#[test]
fn test_registry_with_array_of_refs() {
    let registry = SchemaRegistry::new();

    registry
        .register("UserId", Schema::integer().positive())
        .unwrap();

    registry
        .register("UserList", Schema::array(Schema::ref_("UserId")))
        .unwrap();

    let result = registry.validate("UserList", &json!([1, 2, 3])).unwrap();
    assert!(result.is_success());

    let result = registry.validate("UserList", &json!([1, -2, 3])).unwrap();
    assert!(result.is_failure());
}

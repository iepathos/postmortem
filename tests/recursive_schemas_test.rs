//! Tests for recursive schema structures and depth tracking.

use postmortem::{Schema, SchemaRegistry, ValueValidator};
use serde_json::json;

#[test]
fn test_self_referencing_schema() {
    let registry = SchemaRegistry::new();

    // Comment schema with optional replies array that references itself
    registry
        .register(
            "Comment",
            Schema::object()
                .field("text", Schema::string())
                .optional("replies", Schema::array(Schema::ref_("Comment"))),
        )
        .unwrap();

    let result = registry
        .validate(
            "Comment",
            &json!({
                "text": "Top comment",
                "replies": [
                    {
                        "text": "Reply 1"
                    },
                    {
                        "text": "Reply 2",
                        "replies": [
                            {
                                "text": "Nested reply"
                            }
                        ]
                    }
                ]
            }),
        )
        .unwrap();

    assert!(result.is_success());
}

#[test]
fn test_mutually_recursive_schemas() {
    let registry = SchemaRegistry::new();

    // A references B, B references A
    registry
        .register(
            "A",
            Schema::object()
                .field("name", Schema::string())
                .optional("b", Schema::ref_("B")),
        )
        .unwrap();

    registry
        .register(
            "B",
            Schema::object()
                .field("value", Schema::integer())
                .optional("a", Schema::ref_("A")),
        )
        .unwrap();

    let result = registry
        .validate(
            "A",
            &json!({
                "name": "First A",
                "b": {
                    "value": 42,
                    "a": {
                        "name": "Nested A"
                    }
                }
            }),
        )
        .unwrap();

    assert!(result.is_success());
}

#[test]
fn test_depth_limit_enforcement() {
    // Test that depth limiting works to prevent stack overflow
    let registry = SchemaRegistry::new().with_max_depth(5);

    registry
        .register(
            "Node",
            Schema::object()
                .field("value", Schema::integer())
                .optional("next", Schema::ref_("Node")),
        )
        .unwrap();

    // Build a nested structure
    fn build_nested(depth: usize) -> serde_json::Value {
        if depth == 0 {
            json!({ "value": depth })
        } else {
            json!({
                "value": depth,
                "next": build_nested(depth - 1)
            })
        }
    }

    // Shallow nesting should succeed
    let result = registry.validate("Node", &build_nested(2)).unwrap();
    assert!(result.is_success());

    // Very deep nesting should fail
    let result = registry.validate("Node", &build_nested(20)).unwrap();
    assert!(result.is_failure());
}

#[test]
fn test_reasonable_recursion_depth() {
    let registry = SchemaRegistry::new(); // default max_depth = 100

    registry
        .register(
            "Node",
            Schema::object()
                .field("value", Schema::integer())
                .optional("next", Schema::ref_("Node")),
        )
        .unwrap();

    fn build_nested(depth: usize) -> serde_json::Value {
        if depth == 0 {
            json!({ "value": depth })
        } else {
            json!({
                "value": depth,
                "next": build_nested(depth - 1)
            })
        }
    }

    // Moderate depth should succeed
    let result = registry.validate("Node", &build_nested(50)).unwrap();
    assert!(result.is_success());
}

#[test]
fn test_circular_ref_through_array() {
    let registry = SchemaRegistry::new();

    registry
        .register(
            "Tree",
            Schema::object()
                .field("value", Schema::integer())
                .optional("children", Schema::array(Schema::ref_("Tree"))),
        )
        .unwrap();

    let result = registry
        .validate(
            "Tree",
            &json!({
                "value": 1,
                "children": [
                    {
                        "value": 2,
                        "children": [
                            { "value": 3 }
                        ]
                    },
                    {
                        "value": 4
                    }
                ]
            }),
        )
        .unwrap();

    assert!(result.is_success());
}

#[test]
fn test_circular_ref_through_combinator() {
    let registry = SchemaRegistry::new();

    registry
        .register(
            "Value",
            Schema::one_of(vec![
                Box::new(Schema::integer()) as Box<dyn ValueValidator>,
                Box::new(Schema::object().field("nested", Schema::ref_("Value")))
                    as Box<dyn ValueValidator>,
            ]),
        )
        .unwrap();

    let result = registry
        .validate(
            "Value",
            &json!({
                "nested": {
                    "nested": 42
                }
            }),
        )
        .unwrap();

    assert!(result.is_success());
}

#[test]
fn test_three_way_mutual_recursion() {
    let registry = SchemaRegistry::new();

    registry
        .register(
            "A",
            Schema::object()
                .field("type", Schema::string())
                .optional("b", Schema::ref_("B")),
        )
        .unwrap();

    registry
        .register(
            "B",
            Schema::object()
                .field("type", Schema::string())
                .optional("c", Schema::ref_("C")),
        )
        .unwrap();

    registry
        .register(
            "C",
            Schema::object()
                .field("type", Schema::string())
                .optional("a", Schema::ref_("A")),
        )
        .unwrap();

    let result = registry
        .validate(
            "A",
            &json!({
                "type": "A",
                "b": {
                    "type": "B",
                    "c": {
                        "type": "C",
                        "a": {
                            "type": "A"
                        }
                    }
                }
            }),
        )
        .unwrap();

    assert!(result.is_success());
}

#[test]
fn test_validate_refs_catches_circular_references() {
    let registry = SchemaRegistry::new();

    registry
        .register(
            "Comment",
            Schema::object()
                .field("text", Schema::string())
                .optional("replies", Schema::array(Schema::ref_("Comment"))),
        )
        .unwrap();

    // validate_refs should not report Comment as unresolved
    let unresolved = registry.validate_refs();
    assert!(unresolved.is_empty());
}

#[test]
fn test_complex_nested_structure() {
    let registry = SchemaRegistry::new();

    // Filesystem-like structure with discriminator fields
    registry
        .register(
            "File",
            Schema::object()
                .field("type", Schema::string())
                .field("name", Schema::string())
                .field("size", Schema::integer())
                .additional_properties(false),
        )
        .unwrap();

    registry
        .register(
            "Directory",
            Schema::object()
                .field("type", Schema::string())
                .field("name", Schema::string())
                .optional("children", Schema::array(Schema::ref_("Entry")))
                .additional_properties(false),
        )
        .unwrap();

    registry
        .register(
            "Entry",
            Schema::one_of(vec![
                Box::new(Schema::ref_("File")) as Box<dyn ValueValidator>,
                Box::new(Schema::ref_("Directory")) as Box<dyn ValueValidator>,
            ]),
        )
        .unwrap();

    let result = registry
        .validate(
            "Directory",
            &json!({
                "type": "directory",
                "name": "root",
                "children": [
                    {
                        "type": "file",
                        "name": "file1.txt",
                        "size": 100
                    },
                    {
                        "type": "directory",
                        "name": "subdir",
                        "children": [
                            {
                                "type": "file",
                                "name": "file2.txt",
                                "size": 200
                            }
                        ]
                    }
                ]
            }),
        )
        .unwrap();

    if result.is_failure() {
        if let stillwater::Validation::Failure(ref errs) = result {
            for err in errs.iter() {
                eprintln!("Validation error: {}", err);
            }
        }
    }
    assert!(result.is_success());
}

#[test]
fn test_invalid_data_in_recursive_structure() {
    let registry = SchemaRegistry::new();

    registry
        .register(
            "Node",
            Schema::object()
                .field("value", Schema::integer().positive())
                .optional("next", Schema::ref_("Node")),
        )
        .unwrap();

    let result = registry
        .validate(
            "Node",
            &json!({
                "value": 1,
                "next": {
                    "value": -5
                }
            }),
        )
        .unwrap();

    assert!(result.is_failure());
}

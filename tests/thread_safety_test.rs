//! Tests for thread-safe concurrent access to schema registry.

use postmortem::{Schema, SchemaRegistry};
use serde_json::json;
use std::sync::Arc;
use std::thread;

#[test]
fn test_concurrent_validation() {
    let registry = Arc::new(SchemaRegistry::new());

    registry
        .register("User", Schema::object()
            .field("name", Schema::string())
            .field("age", Schema::integer().positive()))
        .unwrap();

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let registry = Arc::clone(&registry);
            thread::spawn(move || {
                let result = registry.validate("User", &json!({
                    "name": format!("User{}", i),
                    "age": 20 + i
                })).unwrap();
                assert!(result.is_success());
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_schema_access() {
    let registry = Arc::new(SchemaRegistry::new());

    registry
        .register("Email", Schema::string())
        .unwrap();

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let registry = Arc::clone(&registry);
            thread::spawn(move || {
                let schema = registry.get("Email");
                assert!(schema.is_some());
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_validation_with_refs() {
    let registry = Arc::new(SchemaRegistry::new());

    registry
        .register("UserId", Schema::integer().positive())
        .unwrap();

    registry
        .register("User", Schema::object()
            .field("id", Schema::ref_("UserId"))
            .field("name", Schema::string()))
        .unwrap();

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let registry = Arc::clone(&registry);
            thread::spawn(move || {
                let result = registry.validate("User", &json!({
                    "id": i + 1,
                    "name": format!("User{}", i)
                })).unwrap();
                assert!(result.is_success());
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_recursive_validation() {
    let registry = Arc::new(SchemaRegistry::new());

    registry
        .register("Node", Schema::object()
            .field("value", Schema::integer())
            .optional("next", Schema::ref_("Node")))
        .unwrap();

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let registry = Arc::clone(&registry);
            thread::spawn(move || {
                let result = registry.validate("Node", &json!({
                    "value": i,
                    "next": {
                        "value": i + 1,
                        "next": {
                            "value": i + 2
                        }
                    }
                })).unwrap();
                assert!(result.is_success());
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_registry_clone_thread_safety() {
    let registry = SchemaRegistry::new();

    registry
        .register("Test", Schema::string())
        .unwrap();

    let cloned = registry.clone();
    let registry1 = Arc::new(registry);
    let registry2 = Arc::new(cloned);

    let handle1 = {
        let registry = Arc::clone(&registry1);
        thread::spawn(move || {
            let result = registry.validate("Test", &json!("hello")).unwrap();
            assert!(result.is_success());
        })
    };

    let handle2 = {
        let registry = Arc::clone(&registry2);
        thread::spawn(move || {
            let result = registry.validate("Test", &json!("world")).unwrap();
            assert!(result.is_success());
        })
    };

    handle1.join().unwrap();
    handle2.join().unwrap();
}

#[test]
fn test_concurrent_mixed_operations() {
    let registry = Arc::new(SchemaRegistry::new());

    registry
        .register("UserId", Schema::integer().positive())
        .unwrap();

    registry
        .register("User", Schema::object()
            .field("id", Schema::ref_("UserId")))
        .unwrap();

    let handles: Vec<_> = (0..20)
        .map(|i| {
            let registry = Arc::clone(&registry);
            thread::spawn(move || {
                if i % 2 == 0 {
                    // Even threads validate
                    let result = registry.validate("User", &json!({
                        "id": i + 1
                    })).unwrap();
                    assert!(result.is_success());
                } else {
                    // Odd threads just get schema
                    let schema = registry.get("User");
                    assert!(schema.is_some());
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_validate_refs() {
    let registry = Arc::new(SchemaRegistry::new());

    registry
        .register("A", Schema::ref_("B"))
        .unwrap();

    registry
        .register("B", Schema::string())
        .unwrap();

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let registry = Arc::clone(&registry);
            thread::spawn(move || {
                let unresolved = registry.validate_refs();
                assert!(unresolved.is_empty());
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_stress_concurrent_validation() {
    let registry = Arc::new(SchemaRegistry::new());

    registry
        .register("Email", Schema::string())
        .unwrap();

    registry
        .register("UserId", Schema::integer().positive())
        .unwrap();

    registry
        .register("User", Schema::object()
            .field("id", Schema::ref_("UserId"))
            .field("email", Schema::ref_("Email"))
            .field("name", Schema::string()))
        .unwrap();

    // Create 100 threads all validating concurrently
    let handles: Vec<_> = (0..100)
        .map(|i| {
            let registry = Arc::clone(&registry);
            thread::spawn(move || {
                for j in 0..10 {
                    let result = registry.validate("User", &json!({
                        "id": i * 10 + j + 1,
                        "email": format!("user{}@example.com", i),
                        "name": format!("User {}", i)
                    })).unwrap();
                    assert!(result.is_success());
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_access_different_schemas() {
    let registry = Arc::new(SchemaRegistry::new());

    registry
        .register("String", Schema::string())
        .unwrap();

    registry
        .register("Integer", Schema::integer())
        .unwrap();

    registry
        .register("Object", Schema::object()
            .field("value", Schema::string()))
        .unwrap();

    let schemas = ["String", "Integer", "Object"];
    let values = [json!("test"), json!(42), json!({"value": "hello"})];

    let handles: Vec<_> = (0..30)
        .map(|i| {
            let registry = Arc::clone(&registry);
            let schema_name = schemas[i % 3];
            let value = values[i % 3].clone();
            thread::spawn(move || {
                let result = registry.validate(schema_name, &value).unwrap();
                assert!(result.is_success());
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

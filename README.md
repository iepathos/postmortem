# postmortem

> Learn what went wrong—all at once.

[![Crates.io](https://img.shields.io/crates/v/postmortem.svg)](https://crates.io/crates/postmortem)
[![Documentation](https://docs.rs/postmortem/badge.svg)](https://docs.rs/postmortem)
[![License](https://img.shields.io/badge/license-MIT)](LICENSE)

A validation library that accumulates **all** validation errors instead of short-circuiting on the first failure.

## Why "postmortem"?

In software, a **postmortem** is what you do after something breaks—gathering all the evidence to understand what went wrong. Traditional validation libraries give you the frustrating experience of fixing one error only to discover another:

```
Validation failed: email is required

# fix email...

Validation failed: age must be >= 18

# fix age...

Validation failed: password too short
```

**postmortem** gives you the complete picture upfront:

```
Validation errors (3):
  $.email: missing required field
  $.age: value 15 must be >= 18
  $.password: length 5 is less than minimum 8
```

One validation run. All errors. Complete feedback for fixing what went wrong.

## Features

- **Accumulate all errors** — Never stop at the first problem
- **Composable schemas** — Build complex validators from simple primitives
- **Type-safe** — Leverage Rust's type system
- **JSON path tracking** — Know exactly which field failed (e.g., `users[0].email`)
- **Schema registry** — Define reusable schemas with references
- **Cross-field validation** — Validate relationships between fields
- **Recursive schemas** — Support for self-referential data structures

## Quick Start

```rust
use postmortem::{Schema, JsonPath};
use serde_json::json;

// Build a validation schema
let user_schema = Schema::object()
    .required("email", Schema::string().min_len(1).max_len(255))
    .required("age", Schema::integer().min(18).max(120))
    .required("password", Schema::string().min_len(8));

// Validate data - accumulates ALL errors
let data = json!({
    "email": "",
    "age": 15,
    "password": "short"
});

let result = user_schema.validate(&data, &JsonPath::root());

// Handle accumulated errors
match result {
    Ok(value) => println!("Valid: {:?}", value),
    Err(errors) => {
        eprintln!("Validation errors ({}):", errors.len());
        for error in errors.iter() {
            eprintln!("  {}: {}", error.path, error.message);
        }
    }
}
```

## Installation

```toml
[dependencies]
postmortem = "0.1"
```

## Core Concepts

### Schema Types

Build validation schemas using a fluent API:

```rust
// String validation
let name = Schema::string()
    .min_len(1)
    .max_len(100)
    .pattern(r"^[a-zA-Z\s]+$");

// Integer validation
let age = Schema::integer()
    .min(0)
    .max(150);

// Array validation
let tags = Schema::array()
    .min_items(1)
    .max_items(10)
    .items(Schema::string());

// Object validation
let user = Schema::object()
    .required("name", Schema::string())
    .optional("bio", Schema::string().max_len(500));
```

### Schema Combinators

Combine schemas for complex validation logic:

```rust
// One of multiple schemas
let id = Schema::one_of(vec![
    Schema::string().pattern(r"^usr_[0-9a-f]{16}$"),
    Schema::integer().min(1),
]);

// All schemas must pass
let strict_string = Schema::all_of(vec![
    Schema::string().min_len(8),
    Schema::string().pattern(r"[A-Z]"),  // has uppercase
    Schema::string().pattern(r"[0-9]"),  // has digit
]);

// Exactly one schema must pass (XOR)
let payment = Schema::exactly_one_of(vec![
    Schema::object().required("card_number", Schema::string()),
    Schema::object().required("bank_account", Schema::string()),
]);
```

### Schema Registry and References

Define reusable schemas with references:

```rust
use postmortem::SchemaRegistry;

let mut registry = SchemaRegistry::new();

// Define a reusable schema
registry.define("address", Schema::object()
    .required("street", Schema::string())
    .required("city", Schema::string())
    .required("zip", Schema::string().pattern(r"^\d{5}$")));

// Reference it in other schemas
let person = Schema::object()
    .required("name", Schema::string())
    .required("home", Schema::ref_schema("#/address"))
    .required("work", Schema::ref_schema("#/address"));

// Validate with the registry
let result = registry.validate("person", &data);
```

### JSON Path Tracking

Every error includes the exact path to the failing field:

```rust
let data = json!({
    "users": [
        {"email": "valid@example.com"},
        {"email": ""},  // Invalid
        {"email": "also-valid@example.com"},
        {"email": ""}   // Also invalid
    ]
});

let schema = Schema::array().items(
    Schema::object().required("email", Schema::string().min_len(1))
);

let result = schema.validate(&data, &JsonPath::root());
// Errors at: users[1].email and users[3].email
```

### Cross-Field Validation

Validate relationships between fields:

```rust
let schema = Schema::object()
    .required("password", Schema::string().min_len(8))
    .required("confirm_password", Schema::string())
    .custom(|value| {
        if value["password"] != value["confirm_password"] {
            Err(SchemaError::custom(
                JsonPath::new("confirm_password"),
                "passwords must match"
            ))
        } else {
            Ok(value.clone())
        }
    });
```

### Effect Integration

For advanced use cases requiring dependency injection, async validation, or schema loading from external sources, **postmortem** provides Effect integration:

```rust
use postmortem::effect::{SchemaEnv, FileSystem, load_schemas_from_dir, AsyncValidator};
use std::path::{Path, PathBuf};
use std::fs;

// 1. Implement FileSystem for your storage backend
struct RealFileSystem;

impl FileSystem for RealFileSystem {
    type Error = std::io::Error;

    fn read_file(&self, path: &Path) -> Result<String, Self::Error> {
        fs::read_to_string(path)
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, Self::Error> {
        fs::read_dir(path)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| std::io::Error::new(
                std::io::ErrorKind::Other,
                "failed to read dir entry"
            ))
    }
}

// 2. Implement SchemaEnv to provide environment dependencies
struct AppEnv {
    fs: RealFileSystem,
}

impl SchemaEnv for AppEnv {
    type Fs = RealFileSystem;

    fn filesystem(&self) -> &Self::Fs {
        &self.fs
    }
}

// 3. Load schemas from a directory
let env = AppEnv { fs: RealFileSystem };
let schemas_dir = Path::new("./schemas");

let registry = load_schemas_from_dir(&env, schemas_dir)?;

// 4. Use the loaded schemas for validation
let user_data = json!({
    "email": "user@example.com",
    "age": 25
});

let result = registry.validate("user", &user_data);

// 5. For async validation with environment dependencies
use postmortem::effect::StringSchemaExt;

struct DatabaseEnv {
    connection_string: String,
}

let email_schema = Schema::string()
    .min_len(1)
    .validate_with_env(|value, path, env: &DatabaseEnv| {
        // Access database via environment
        let email = value.as_str().unwrap_or("");

        // Check uniqueness (simplified example)
        if email == "taken@example.com" {
            Validation::Failure(SchemaErrors::single(
                SchemaError::new(path.clone(), "email already exists")
            ))
        } else {
            Validation::Success(())
        }
    });

let db_env = DatabaseEnv {
    connection_string: "postgres://localhost/mydb".to_string(),
};

let result = email_schema.validate_with_env(
    &json!("user@example.com"),
    &JsonPath::root(),
    &db_env
);
```

This Effect-based approach provides:
- **Dependency injection** — Pass environment dependencies explicitly
- **Testability** — Mock filesystems and databases in tests
- **Flexibility** — Support different storage backends and async operations
- **Error accumulation** — All validation errors collected, even across I/O operations

Note: The Effect integration uses a simplified API compatible with stillwater 0.12. Instead of returning `Effect<E, Er, R>` types, functions accept environment parameters directly and return `Result` or `Validation` types. This provides the same dependency injection benefits with a more straightforward API.

## Design Philosophy

**postmortem** is built on functional programming principles:

- **Pure validation** — Schemas are immutable, validation has no side effects
- **Composition** — Build complex validators from simple primitives
- **Error accumulation** — Uses applicative functors (stillwater's `Validation` type) to collect all errors

This design makes schemas:
- Easy to test in isolation
- Safe to use concurrently (schemas are `Send + Sync`)
- Simple to compose and reuse

## Documentation

Full API documentation is available at [docs.rs/postmortem](https://docs.rs/postmortem).

## License

MIT — Glen Baker <iepathos@gmail.com>

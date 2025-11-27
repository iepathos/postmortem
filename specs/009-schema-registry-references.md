---
number: 9
title: Schema Registry and References
category: foundation
priority: medium
status: draft
dependencies: [1, 2, 3, 4, 5, 7]
created: 2025-11-26
---

# Specification 009: Schema Registry and References

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: Specs 001-005, 007 (Core Types, Basic Schemas, Combinators)

## Context

Complex applications often have schemas that reference each other. A `User` schema might be used in both `CreateUserRequest` and `UserResponse`. A `Comment` might reference itself for replies. This specification enables schema reuse through a registry system with named schema references.

The registry also forms the foundation for JSON Schema and OpenAPI generation, where schemas are typically defined in a shared definitions/components section.

## Objective

Implement a schema registry that:
1. Stores named schemas for reuse
2. Enables schema references via `$ref`-style syntax
3. Resolves references at validation time
4. Detects and handles circular references
5. Validates reference integrity at registration time

## Requirements

### Functional Requirements

1. **Schema Registry**
   - `SchemaRegistry::new()` creates empty registry
   - `.register(name, schema)` adds named schema
   - `.get(name)` retrieves schema by name
   - Schema names must be unique
   - Registry is the validation entry point when using refs

2. **Schema References**
   - `Schema::ref_(name)` creates a reference to a named schema
   - References resolve to registered schema at validation time
   - Missing reference produces clear error
   - References work in any schema position (fields, arrays, combinators)

3. **Reference Validation**
   - `.validate_refs()` checks all references resolve
   - Returns list of unresolved references
   - Can be called after all registrations complete

4. **Circular Reference Handling**
   - Detect circular references during validation
   - Support recursive schemas (e.g., tree structures)
   - Maximum depth protection to prevent infinite loops
   - Clear error when max depth exceeded

5. **Registry-Based Validation**
   - `registry.validate(schema_name, value)` validates against named schema
   - Proper error handling for missing schemas
   - Thread-safe read access for validation

### Non-Functional Requirements

- Thread-safe registry (Send + Sync)
- Efficient reference resolution (no repeated lookups)
- Clear error messages for reference issues
- Support for recursive schemas without stack overflow

## Acceptance Criteria

- [ ] `SchemaRegistry::new()` creates empty registry
- [ ] `.register("User", schema)` adds schema to registry
- [ ] `.register()` returns error for duplicate names
- [ ] `Schema::ref_("User")` creates reference
- [ ] Reference validates using referenced schema
- [ ] Missing reference produces error with code `missing_reference`
- [ ] `.validate_refs()` returns unresolved reference names
- [ ] Circular references are detected
- [ ] Recursive schemas work (with depth limit)
- [ ] Max depth error includes path and limit
- [ ] Registry is thread-safe for concurrent validation

## Technical Details

### Implementation Approach

```rust
use std::sync::Arc;
use parking_lot::RwLock;

pub struct SchemaRegistry {
    schemas: Arc<RwLock<HashMap<String, Arc<dyn SchemaLike>>>>,
    max_depth: usize,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self {
            schemas: Arc::new(RwLock::new(HashMap::new())),
            max_depth: 100,
        }
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn register<S: SchemaLike + 'static>(
        &self,
        name: impl Into<String>,
        schema: S,
    ) -> Result<(), RegistryError> {
        let name = name.into();
        let mut schemas = self.schemas.write();

        if schemas.contains_key(&name) {
            return Err(RegistryError::DuplicateName(name));
        }

        schemas.insert(name, Arc::new(schema));
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn SchemaLike>> {
        self.schemas.read().get(name).cloned()
    }

    pub fn validate_refs(&self) -> Vec<String> {
        let schemas = self.schemas.read();
        let mut unresolved = Vec::new();

        for schema in schemas.values() {
            self.collect_unresolved_refs(schema.as_ref(), &schemas, &mut unresolved);
        }

        unresolved.sort();
        unresolved.dedup();
        unresolved
    }

    pub fn validate(
        &self,
        schema_name: &str,
        value: &Value,
    ) -> Result<Validation<ValidatedValue, SchemaErrors>, RegistryError> {
        let schema = self.get(schema_name)
            .ok_or_else(|| RegistryError::SchemaNotFound(schema_name.to_string()))?;

        let context = ValidationContext::new(self, self.max_depth);
        Ok(schema.validate_with_context(value, &JsonPath::root(), &context))
    }
}

pub struct RefSchema {
    name: String,
}

impl Schema {
    pub fn ref_(name: impl Into<String>) -> RefSchema {
        RefSchema { name: name.into() }
    }
}

impl SchemaLike for RefSchema {
    fn validate_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<ValidatedValue, SchemaErrors> {
        // Check depth
        if context.depth() > context.max_depth() {
            return Validation::invalid(SchemaErrors::single(
                SchemaError::new(path.clone(), format!(
                    "maximum reference depth {} exceeded at path '{}'",
                    context.max_depth(),
                    path
                ))
                .with_code("max_depth_exceeded")
            ));
        }

        // Resolve reference
        let schema = match context.registry().get(&self.name) {
            Some(s) => s,
            None => {
                return Validation::invalid(SchemaErrors::single(
                    SchemaError::new(
                        path.clone(),
                        format!("schema '{}' not found in registry", self.name),
                    )
                    .with_code("missing_reference")
                ))
            }
        };

        // Validate with incremented depth
        schema.validate_with_context(value, path, &context.increment_depth())
    }
}

pub struct ValidationContext<'a> {
    registry: &'a SchemaRegistry,
    depth: usize,
    max_depth: usize,
}

impl<'a> ValidationContext<'a> {
    fn new(registry: &'a SchemaRegistry, max_depth: usize) -> Self {
        Self { registry, depth: 0, max_depth }
    }

    fn increment_depth(&self) -> Self {
        Self {
            registry: self.registry,
            depth: self.depth + 1,
            max_depth: self.max_depth,
        }
    }

    fn depth(&self) -> usize { self.depth }
    fn max_depth(&self) -> usize { self.max_depth }
    fn registry(&self) -> &SchemaRegistry { self.registry }
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("schema '{0}' already registered")]
    DuplicateName(String),

    #[error("schema '{0}' not found")]
    SchemaNotFound(String),
}
```

### Architecture Changes

- Create `src/registry.rs` for SchemaRegistry
- Add `RefSchema` type
- Extend SchemaLike trait with context-aware validation
- Add ValidationContext for depth tracking

### Data Structures

- `SchemaRegistry`: Thread-safe map of named schemas
- `RefSchema`: Schema that references another by name
- `ValidationContext`: Carries registry and depth info during validation
- `RegistryError`: Error type for registry operations

### APIs and Interfaces

```rust
// Registry
SchemaRegistry::new() -> SchemaRegistry
SchemaRegistry::with_max_depth(self, depth: usize) -> Self
SchemaRegistry::register<S>(self, name: &str, schema: S) -> Result<(), RegistryError>
SchemaRegistry::get(&self, name: &str) -> Option<Arc<dyn SchemaLike>>
SchemaRegistry::validate_refs(&self) -> Vec<String>
SchemaRegistry::validate(&self, name: &str, value: &Value) -> Result<Validation<...>, RegistryError>

// Reference schema
Schema::ref_(name: &str) -> RefSchema
```

## Dependencies

- **Prerequisites**: Specs 001-005, 007
- **Affected Components**: Schema trait (context-aware validation)
- **External Dependencies**:
  - `parking_lot` for RwLock (or std::sync::RwLock)

## Testing Strategy

- **Unit Tests**:
  - Registry creation and registration
  - Duplicate name rejection
  - Schema retrieval
  - Reference resolution
  - Missing reference error
  - validate_refs detection

- **Integration Tests**:
  - Recursive schema validation
  - Circular reference detection
  - Max depth enforcement
  - Complex schema graphs

- **Edge Cases**:
  - Self-referencing schema
  - Mutually recursive schemas
  - Empty registry
  - Very deep nesting

## Documentation Requirements

- **Code Documentation**: Examples of schema reuse
- **User Documentation**: Guide to registry patterns
- **Architecture Updates**: Document reference resolution

## Implementation Notes

- Use Arc for shared schema ownership
- RwLock allows concurrent reads during validation
- ValidationContext is passed through all validation calls
- Consider lazy reference resolution for better error messages
- Default max_depth of 100 should handle most use cases

## Migration and Compatibility

No migration needed - new feature. However, SchemaLike trait gains a new method which may require updates to existing schema types.

## Files to Create/Modify

```
src/registry.rs
src/schema/ref.rs
src/validation/context.rs
tests/registry_test.rs
tests/references_test.rs
```

## Example Usage

```rust
use postmortem::{Schema, SchemaRegistry};

// Create registry
let registry = SchemaRegistry::new();

// Register base schemas
registry.register("Email", Schema::string().email()).unwrap();
registry.register("UserId", Schema::integer().positive()).unwrap();

// Register schemas that use references
registry.register("User", Schema::object()
    .field("id", Schema::ref_("UserId"))
    .field("email", Schema::ref_("Email"))
    .optional("name", Schema::string())
).unwrap();

// Recursive schema (comments with replies)
registry.register("Comment", Schema::object()
    .field("text", Schema::string().min_len(1))
    .field("author_id", Schema::ref_("UserId"))
    .optional("replies", Schema::array(Schema::ref_("Comment")))
).unwrap();

// Validate references are all resolvable
let unresolved = registry.validate_refs();
assert!(unresolved.is_empty());

// Use registry for validation
let result = registry.validate("User", &json!({
    "id": 42,
    "email": "user@example.com",
    "name": "Alice"
}));

match result {
    Ok(Validation::Valid(user)) => println!("Valid user: {:?}", user),
    Ok(Validation::Invalid(errors)) => println!("Invalid: {:?}", errors),
    Err(e) => println!("Registry error: {}", e),
}

// Deeply nested recursive structure
let comment_tree = json!({
    "text": "Root comment",
    "author_id": 1,
    "replies": [{
        "text": "Reply 1",
        "author_id": 2,
        "replies": [{
            "text": "Nested reply",
            "author_id": 3,
            "replies": []
        }]
    }]
});

let result = registry.validate("Comment", &comment_tree);
assert!(matches!(result, Ok(Validation::Valid(_))));
```

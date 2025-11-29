---
number: 9
title: Schema Registry and References
category: foundation
priority: medium
status: ready
dependencies: [1, 2, 3, 4, 5, 7]
created: 2025-11-26
updated: 2025-11-28
---

# Specification 009: Schema Registry and References

**Category**: foundation
**Priority**: medium
**Status**: ready
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

### Core Functionality
- [ ] `SchemaRegistry::new()` creates empty registry
- [ ] `.register("User", schema)` adds schema to registry
- [ ] `.register()` returns error for duplicate names with `RegistryError::DuplicateName`
- [ ] `Schema::ref_("User")` creates reference schema
- [ ] Reference validates using referenced schema when used with registry
- [ ] RefSchema.validate() without registry produces error with code `missing_registry`
- [ ] Missing reference during validation produces error with code `missing_reference`
- [ ] `.validate_refs()` returns list of unresolved reference names
- [ ] `.validate_refs()` on valid registry returns empty vec

### Trait Extension
- [ ] SchemaLike trait has `validate_with_context()` method with default impl
- [ ] SchemaLike trait has `collect_refs()` method with default impl
- [ ] Default `validate_with_context()` delegates to `validate()`
- [ ] All existing schemas work without changes (backward compatible)

### Depth Tracking
- [ ] Depth increments only when following a reference (not for containers)
- [ ] Max depth error includes path, limit, and code `max_depth_exceeded`
- [ ] Max depth enforced at boundary (depth < max_depth passes, depth >= max_depth fails)
- [ ] Recursive schemas work within depth limit
- [ ] Self-referencing schemas detect cycles

### Container Support
- [ ] References work in object fields
- [ ] References work in array items
- [ ] References work in AnyOf/AllOf/OneOf branches
- [ ] Container schemas implement collect_refs() to traverse children

### Thread Safety
- [ ] Registry is thread-safe for concurrent validation
- [ ] Concurrent registration is serialized correctly
- [ ] Mixed registration and validation is safe
- [ ] ValidationContext uses Arc (no lifetime constraints)

### Integration
- [ ] Complex schema graphs with multiple reference paths validate correctly
- [ ] Mutually recursive schemas (A->B->A) work within depth limit
- [ ] Deep nesting (50+ levels) within limit works
- [ ] All existing tests pass with trait changes

## Technical Details

### SchemaLike Trait Extension

The existing `SchemaLike` trait needs to support context-aware validation for references. This is done by adding a new method with a default implementation that delegates to the existing `validate()` method:

```rust
pub trait SchemaLike: Send + Sync {
    // Existing method - used for standalone validation without registry
    fn validate(
        &self,
        value: &Value,
        path: &JsonPath
    ) -> Validation<ValidatedValue, SchemaErrors>;

    // New method - used when validating with registry and references
    // Default implementation preserves backward compatibility
    fn validate_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<ValidatedValue, SchemaErrors> {
        // Default: ignore context, delegate to existing validate()
        self.validate(value, path)
    }

    // New method - collect all schema references for validation
    // Default implementation: no references
    fn collect_refs(&self, _refs: &mut Vec<String>) {
        // Most schemas don't have references
    }
}
```

**Migration Strategy**:
- Existing schemas automatically work via default `validate_with_context()` implementation
- Only `RefSchema` and container schemas (Object, Array, AnyOf, etc.) need custom implementations
- No breaking changes - all existing code continues to work

### Implementation Approach

```rust
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;

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

    /// Check all registered schemas for unresolved references.
    /// Returns list of reference names that don't exist in registry.
    /// Should be called after all schemas are registered.
    pub fn validate_refs(&self) -> Vec<String> {
        let schemas = self.schemas.read();
        let mut all_refs = Vec::new();

        // Collect all references from all schemas
        for schema in schemas.values() {
            schema.collect_refs(&mut all_refs);
        }

        // Find references that don't exist in registry
        let mut unresolved = Vec::new();
        for ref_name in all_refs {
            if !schemas.contains_key(&ref_name) {
                unresolved.push(ref_name);
            }
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

        let context = ValidationContext::new(Arc::new(self.clone()), self.max_depth);
        Ok(schema.validate_with_context(value, &JsonPath::root(), &context))
    }
}

impl Clone for SchemaRegistry {
    fn clone(&self) -> Self {
        Self {
            schemas: Arc::clone(&self.schemas),
            max_depth: self.max_depth,
        }
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
    fn validate(
        &self,
        _value: &Value,
        path: &JsonPath,
    ) -> Validation<ValidatedValue, SchemaErrors> {
        use stillwater::validation::failure;

        // Cannot validate reference without registry
        failure(SchemaErrors::single(
            SchemaError::new(
                path.clone(),
                format!(
                    "reference to '{}' cannot be validated without a registry. \
                     Use SchemaRegistry::validate() instead",
                    self.name
                ),
            )
            .with_code("missing_registry")
        ))
    }

    fn validate_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<ValidatedValue, SchemaErrors> {
        use stillwater::validation::failure;

        // Check depth before resolving
        if context.depth() >= context.max_depth() {
            return failure(SchemaErrors::single(
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
                return failure(SchemaErrors::single(
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

    fn collect_refs(&self, refs: &mut Vec<String>) {
        refs.push(self.name.clone());
    }
}

/// Validation context carries registry and depth tracking information.
/// Uses Arc for registry to avoid lifetime constraints.
pub struct ValidationContext {
    registry: Arc<SchemaRegistry>,
    depth: usize,
    max_depth: usize,
}

impl ValidationContext {
    pub fn new(registry: Arc<SchemaRegistry>, max_depth: usize) -> Self {
        Self { registry, depth: 0, max_depth }
    }

    pub fn increment_depth(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            depth: self.depth + 1,
            max_depth: self.max_depth,
        }
    }

    pub fn depth(&self) -> usize { self.depth }
    pub fn max_depth(&self) -> usize { self.max_depth }
    pub fn registry(&self) -> &SchemaRegistry { &self.registry }
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("schema '{0}' already registered")]
    DuplicateName(String),

    #[error("schema '{0}' not found")]
    SchemaNotFound(String),
}
```

### Depth Tracking Semantics

Depth is tracked **per reference traversal**, not per validation path:

- **Depth increments**: Only when following a `Schema::ref_()` to another schema
- **Depth does not increment**: When validating object fields, array items, or combinator branches

**Examples**:
```rust
// Depth 1: User -> (ref UserId)
Schema::object()
    .field("id", Schema::ref_("UserId"))  // depth=1 when validating UserId

// Depth 2: User -> Comment -> (ref UserId)
Schema::object()
    .field("comment", Schema::ref_("Comment"))  // depth=1 for Comment
    .field("author", Schema::ref_("UserId"))    // depth=2 for UserId within Comment

// NOT depth tracking: Array items don't increment depth
Schema::array(Schema::ref_("Item"))  // Each item validates at same depth

// Recursive: Comment -> replies -> Comment -> replies -> Comment...
// depth=1, 2, 3... until max_depth reached
Schema::object()
    .optional("replies", Schema::array(Schema::ref_("Comment")))
```

**Rationale**:
- Prevents infinite loops in circular references
- Allows reasonable depth limits (100) for typical schemas
- Focuses on reference chain length, not data structure depth

### Registry Lifecycle and Mutability

The registry follows an **append-only** model after creation:

- **Creation**: `SchemaRegistry::new()` creates empty registry
- **Registration Phase**: Schemas added via `.register()` - writes are serialized via RwLock
- **Validation Phase**: Multiple threads can validate concurrently (read-only access)
- **No Removal**: Once registered, schemas cannot be removed or updated

**Thread Safety**:
- Concurrent registration: Serialized (one at a time)
- Concurrent validation: Fully parallel (many threads)
- Mixed registration + validation: Safe but registration blocks all validators

**Rationale**:
- Simpler semantics - no schema versioning needed
- Better performance - no need to track schema invalidation
- Safer - prevents accidental schema changes during validation
- Typical usage: Register all schemas at startup, then validate

### Container Schema Implementation

Container schemas (Object, Array, AnyOf, etc.) need custom `collect_refs()` implementations:

```rust
impl SchemaLike for ObjectSchema {
    fn collect_refs(&self, refs: &mut Vec<String>) {
        // Collect from all fields
        for field_schema in self.fields.values() {
            field_schema.collect_refs(refs);
        }
    }

    fn validate_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<ValidatedValue, SchemaErrors> {
        // Validate each field with context (not shown - similar to existing validate)
        // Pass context through to nested schema validations
    }
}

impl SchemaLike for ArraySchema {
    fn collect_refs(&self, refs: &mut Vec<String>) {
        self.item_schema.collect_refs(refs);
    }

    fn validate_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<ValidatedValue, SchemaErrors> {
        // Validate items with context
        // Note: Array items don't increment depth, only refs do
    }
}

impl SchemaLike for AnyOfSchema {
    fn collect_refs(&self, refs: &mut Vec<String>) {
        for schema in &self.schemas {
            schema.collect_refs(refs);
        }
    }
}
```

### Architecture Changes

- Create `src/registry.rs` for SchemaRegistry
- Add `src/schema/ref_schema.rs` for RefSchema type
- Update `src/validation/context.rs` to add ValidationContext
- Extend `src/schema/mod.rs` SchemaLike trait with new methods (default impls)
- Update container schemas (Object, Array, combinators) to implement collect_refs and validate_with_context

### Data Structures

- `SchemaRegistry`: Thread-safe append-only map of named schemas
- `RefSchema`: Schema that references another by name
- `ValidationContext`: Carries registry reference and depth counter
- `RegistryError`: Error type for registry operations (duplicate names, not found)

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
  - Missing reference error with code `missing_reference`
  - validate_refs detection of unresolved references
  - RefSchema.validate() returns `missing_registry` error
  - Depth tracking increments only on refs
  - Max depth error with code `max_depth_exceeded`

- **Integration Tests**:
  - Recursive schema validation (Comment with replies)
  - Self-referencing schema
  - Mutually recursive schemas (A refs B, B refs A)
  - Max depth enforcement at boundary (99 passes, 100 fails)
  - Complex schema graphs with multiple reference paths
  - References in different positions:
    - Object fields
    - Array items
    - AnyOf/AllOf/OneOf branches
    - Nested combinations

- **Thread Safety Tests**:
  - Concurrent validation from multiple threads
  - Concurrent registration (serialized correctly)
  - Mixed registration and validation
  - No data races under concurrent load

- **Edge Cases**:
  - Empty registry
  - Validating with unregistered schema name
  - Very deep nesting (at and beyond max_depth)
  - Schema with reference to itself
  - Typo in reference name caught by validate_refs
  - Zero-length reference chain

- **Performance Tests**:
  - Deep recursive structures (depth 50-90)
  - Wide schema graphs (100+ schemas)
  - Repeated validations (Arc caching efficiency)

## Documentation Requirements

- **Code Documentation**: Examples of schema reuse
- **User Documentation**: Guide to registry patterns
- **Architecture Updates**: Document reference resolution

## Implementation Notes

- Use Arc for shared schema ownership
- RwLock allows concurrent reads during validation
- ValidationContext is passed through all validation calls
- Default max_depth of 100 should handle most use cases
- **Object Safety**: SchemaLike trait must remain object-safe for `Arc<dyn SchemaLike>`
  - All trait methods must use `&self` receiver
  - No generic type parameters on trait methods (only on trait itself)
  - No associated types that reference Self
  - Existing trait is already object-safe, new methods preserve this
- ValidationContext uses Arc instead of lifetime to avoid borrowing issues
- Container schemas need to implement both validate_with_context and collect_refs to properly support nested references

## Migration and Compatibility

### Backward Compatibility

This is a **non-breaking addition**:
- Existing `SchemaLike` implementors get default `validate_with_context()` that delegates to `validate()`
- Existing `SchemaLike` implementors get default `collect_refs()` that returns no references
- All existing validation code continues to work unchanged
- New functionality (refs) only available when using `SchemaRegistry`

### Migration Steps for Container Schemas

Container schemas need updates to pass context through:

**Before** (current implementation):
```rust
impl SchemaLike for ObjectSchema {
    fn validate(&self, value: &Value, path: &JsonPath) -> Validation<...> {
        // validate fields using field_schema.validate(...)
    }
}
```

**After** (supports references):
```rust
impl SchemaLike for ObjectSchema {
    fn validate(&self, value: &Value, path: &JsonPath) -> Validation<...> {
        // Keep existing implementation for backward compatibility
        // Or delegate to validate_with_context with empty context
    }

    fn validate_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext
    ) -> Validation<...> {
        // Similar logic but call field_schema.validate_with_context(...)
        // Pass context through to enable ref resolution
    }

    fn collect_refs(&self, refs: &mut Vec<String>) {
        for field_schema in self.fields.values() {
            field_schema.collect_refs(refs);
        }
    }
}
```

**Schemas requiring updates**:
- ObjectSchema (specs/003)
- ArraySchema (specs/004)
- AnyOf, AllOf, OneOf (specs/007)
- NotSchema (specs/007)

**Schemas that don't need updates** (default implementation sufficient):
- StringSchema, IntegerSchema, NumberSchema, BooleanSchema (specs/001-002)
- NullSchema, ConstSchema, EnumSchema (specs/005)

### Testing Migration

- Keep existing tests unchanged - they validate backward compatibility
- Add new test files for registry and reference features
- Ensure all existing tests pass with trait changes

## Implementation Stages

Break implementation into focused stages:

### Stage 1: Core Infrastructure
- Add `validate_with_context()` and `collect_refs()` to SchemaLike trait with default impls
- Create ValidationContext in `src/validation/context.rs`
- Create basic SchemaRegistry in `src/registry.rs` (register, get, clone)
- Tests: Trait defaults work, registry stores and retrieves schemas

### Stage 2: Reference Schema
- Implement RefSchema in `src/schema/ref_schema.rs`
- Implement both validate() and validate_with_context() for RefSchema
- Add Schema::ref_() constructor
- Tests: RefSchema errors without registry, resolves with context

### Stage 3: Reference Discovery
- Implement SchemaRegistry::validate_refs()
- Implement collect_refs() for RefSchema
- Tests: Unresolved references detected

### Stage 4: Container Schema Updates
- Update ObjectSchema to pass context through
- Update ArraySchema to pass context through
- Implement collect_refs() for Object and Array
- Tests: References work in object fields and array items

### Stage 5: Combinator Updates and Depth Tracking
- Update AnyOf, AllOf, OneOf to pass context
- Implement depth tracking and max_depth enforcement
- Implement collect_refs() for combinators
- Tests: Recursive schemas, max depth enforcement, circular refs

## Files to Create/Modify

**New Files**:
```
src/registry.rs                  (Stage 1)
src/schema/ref_schema.rs         (Stage 2)
tests/registry_test.rs           (Stages 1-3)
tests/references_test.rs         (Stages 2-5)
tests/recursive_schemas_test.rs  (Stage 5)
```

**Modified Files**:
```
src/schema/mod.rs                (Stage 1 - trait changes)
src/validation/context.rs        (Stage 1 - or create if doesn't exist)
src/schema/object.rs             (Stage 4)
src/schema/array.rs              (Stage 4)
src/schema/combinators.rs        (Stage 5)
src/lib.rs                       (All stages - exports)
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
    Ok(Validation::Success(user)) => println!("Valid user: {:?}", user),
    Ok(Validation::Failure(errors)) => println!("Invalid: {:?}", errors),
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
assert!(matches!(result, Ok(Validation::Success(_))));
```

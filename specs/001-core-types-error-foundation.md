---
number: 1
title: Core Types and Error Foundation
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-11-26
---

# Specification 001: Core Types and Error Foundation

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The postmortem library requires a robust foundation of error types and path tracking to accumulate validation errors. Unlike typical validation libraries that short-circuit on the first error, postmortem must collect ALL validation errors to provide comprehensive feedback to users. This requires careful design of error accumulation strategies and JSON path tracking for precise error location reporting.

This specification establishes the core types that all other validation features will build upon, integrating with stillwater's `Validation` type for applicative error accumulation.

## Objective

Create the foundational error types and path tracking system that enables:
1. Accumulation of all validation errors (not just the first)
2. Precise error location via JSON paths (e.g., `body.users[0].email`)
3. Rich error context including expected vs actual values
4. Seamless integration with stillwater's Validation type

## Requirements

### Functional Requirements

1. **JsonPath Type**
   - Represent paths to values in nested JSON structures
   - Support field access segments (e.g., `user`, `email`)
   - Support array index segments (e.g., `[0]`, `[42]`)
   - Support appending segments to build paths incrementally
   - Format paths as human-readable strings (e.g., `users[0].email`)

2. **SchemaError Type**
   - Capture single validation failure with full context
   - Include the path where the error occurred
   - Include human-readable error message
   - Include the actual value that failed validation (got)
   - Include what was expected (expected)
   - Include machine-readable error code for programmatic handling

3. **SchemaErrors Type**
   - Wrap `NonEmptyVec<SchemaError>` to guarantee at least one error
   - Implement `Semigroup` for combining errors from multiple validations
   - Support iteration over contained errors
   - Provide methods to query errors by path or code

4. **Validation Integration**
   - All validation operations return `Validation<T, SchemaErrors>`
   - Errors accumulate automatically via Validation's applicative instance
   - Support mapping over successful values
   - Support flat_map for dependent validations

### Non-Functional Requirements

- Error types must implement `Display` for human-readable output
- Error types must implement `Debug` for developer debugging
- Error types must be `Send + Sync` for concurrent use
- Minimal allocations during path building operations
- Clear, actionable error messages by default

## Acceptance Criteria

- [ ] `JsonPath` can represent paths like `users[0].email`
- [ ] `JsonPath::push_field("name")` creates a new path with appended field
- [ ] `JsonPath::push_index(0)` creates a new path with appended index
- [ ] `JsonPath::root()` creates an empty path for the root value
- [ ] `JsonPath` displays as dot-notation with bracket indices
- [ ] `SchemaError` contains path, message, got, expected, and code fields
- [ ] `SchemaErrors` wraps `NonEmptyVec<SchemaError>`
- [ ] `SchemaErrors` implements `Semigroup` to combine error sets
- [ ] `SchemaErrors::iter()` provides iteration over errors
- [ ] All types implement `Display`, `Debug`, `Clone`, `PartialEq`
- [ ] All types are `Send + Sync`
- [ ] Unit tests cover path construction and formatting
- [ ] Unit tests cover error combination via Semigroup
- [ ] Integration with `Validation<T, SchemaErrors>` works correctly

## Technical Details

### Implementation Approach

```rust
// JsonPath segment types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PathSegment {
    Field(String),
    Index(usize),
}

// Immutable path with efficient appending
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct JsonPath {
    segments: Vec<PathSegment>,
}

impl JsonPath {
    pub fn root() -> Self { Self::default() }

    pub fn push_field(&self, name: impl Into<String>) -> Self {
        let mut segments = self.segments.clone();
        segments.push(PathSegment::Field(name.into()));
        Self { segments }
    }

    pub fn push_index(&self, index: usize) -> Self {
        let mut segments = self.segments.clone();
        segments.push(PathSegment::Index(index));
        Self { segments }
    }
}

impl Display for JsonPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, segment) in self.segments.iter().enumerate() {
            match segment {
                PathSegment::Field(name) => {
                    if i > 0 { write!(f, ".")?; }
                    write!(f, "{}", name)?;
                }
                PathSegment::Index(idx) => write!(f, "[{}]", idx)?,
            }
        }
        Ok(())
    }
}
```

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaError {
    pub path: JsonPath,
    pub message: String,
    pub got: Option<String>,
    pub expected: Option<String>,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchemaErrors(NonEmptyVec<SchemaError>);

impl Semigroup for SchemaErrors {
    fn combine(self, other: Self) -> Self {
        SchemaErrors(self.0.combine(other.0))
    }
}
```

### Architecture Changes

- Create `src/error/` module for error types
- Create `src/path.rs` for JsonPath implementation
- Establish pattern for all validation to return `Validation<T, SchemaErrors>`

### Data Structures

- `PathSegment`: Enum for field/index segments
- `JsonPath`: Vector of segments with display formatting
- `SchemaError`: Struct with path, message, got, expected, code
- `SchemaErrors`: Newtype wrapper around NonEmptyVec

### APIs and Interfaces

```rust
// Path construction
JsonPath::root() -> JsonPath
JsonPath::push_field(&self, name: impl Into<String>) -> JsonPath
JsonPath::push_index(&self, index: usize) -> JsonPath

// Error construction
SchemaError::new(path: JsonPath, message: impl Into<String>) -> SchemaError
SchemaError::with_code(self, code: impl Into<String>) -> SchemaError
SchemaError::with_got(self, got: impl Into<String>) -> SchemaError
SchemaError::with_expected(self, expected: impl Into<String>) -> SchemaError

// Error collection
SchemaErrors::single(error: SchemaError) -> SchemaErrors
SchemaErrors::iter(&self) -> impl Iterator<Item = &SchemaError>
SchemaErrors::at_path(&self, path: &JsonPath) -> Vec<&SchemaError>
SchemaErrors::with_code(&self, code: &str) -> Vec<&SchemaError>
```

## Dependencies

- **Prerequisites**: None (this is the foundation)
- **Affected Components**: None (new code)
- **External Dependencies**:
  - `stillwater` crate for `Validation` and `NonEmptyVec` types
  - `thiserror` for error derive macros

## Testing Strategy

- **Unit Tests**:
  - Path construction and formatting
  - Error creation with all fields
  - SchemaErrors combination via Semigroup
  - Iteration and querying methods

- **Integration Tests**:
  - Use with stillwater's Validation type
  - Error accumulation across multiple validations

- **Property Tests** (if proptest available):
  - Path display/parse round-trip
  - Semigroup associativity

## Documentation Requirements

- **Code Documentation**: Rustdoc for all public types and methods
- **User Documentation**: Examples of path construction and error handling
- **Architecture Updates**: Document error accumulation pattern

## Implementation Notes

- Use `Arc<str>` or `String` for string fields based on allocation patterns
- Consider `SmallVec` for PathSegments if most paths are short
- SchemaErrors must never be empty (enforced by NonEmptyVec)
- Error codes should be snake_case identifiers (e.g., `min_length_violated`)

## Migration and Compatibility

No migration needed - this is new code. However, the API should be designed for stability as all other specs depend on these types.

## Files to Create

```
src/lib.rs
src/error/mod.rs
src/error/schema_error.rs
src/path.rs
tests/error_test.rs
tests/path_test.rs
```

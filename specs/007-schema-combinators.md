---
number: 7
title: Schema Combinators
category: foundation
priority: high
status: draft
dependencies: [1, 2, 3, 4, 5]
created: 2025-11-26
---

# Specification 007: Schema Combinators

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Specs 001-005 (Core Types, String, Integer, Object, Array)

## Context

Real-world data validation often requires composing schemas in complex ways. A field might accept multiple types (union), require all constraints from several schemas (intersection), or be nullable. This specification defines schema combinators for these composition patterns.

The combinators follow JSON Schema semantics: `oneOf` (exactly one matches), `anyOf` (at least one matches), and `allOf` (all must match). These enable discriminated unions, flexible type unions, and schema merging.

## Objective

Implement schema combinators that:
1. `one_of` - exactly one schema must match (discriminated unions)
2. `any_of` - at least one schema must match (flexible unions)
3. `all_of` - all schemas must match (intersection/merging)
4. `optional/nullable` - value can be null
5. Provide clear errors indicating which branches failed and why

## Requirements

### Functional Requirements

1. **One-Of Combinator**
   - `Schema::one_of(schemas)` - exactly one schema must match
   - Validates value against all schemas
   - Succeeds only if exactly one matches
   - Error if none match (with all branch errors)
   - Error if multiple match (ambiguous)
   - Ideal for discriminated unions

2. **Any-Of Combinator**
   - `Schema::any_of(schemas)` - at least one schema must match
   - Validates until first match found
   - Error if none match (with all branch errors)
   - More permissive than one_of

3. **All-Of Combinator**
   - `Schema::all_of(schemas)` - all schemas must match
   - Validates against all schemas
   - Accumulates errors from all failing schemas
   - Useful for schema composition/intersection

4. **Optional/Nullable**
   - `.optional()` - wraps schema to allow null
   - Null values pass validation
   - Non-null values validated against inner schema

5. **Error Reporting**
   - Combinator errors show which branches were attempted
   - Each branch's errors are preserved
   - Clear indication of combinator type in error

### Non-Functional Requirements

- Short-circuit evaluation where appropriate (any_of)
- Clear error messages for each combinator type
- Efficient validation without redundant checks
- Type-safe validated output

## Acceptance Criteria

- [ ] `Schema::one_of([s1, s2])` requires exactly one match
- [ ] `one_of` reports "none matched" with branch errors
- [ ] `one_of` reports "multiple matched" when ambiguous
- [ ] `Schema::any_of([s1, s2])` requires at least one match
- [ ] `any_of` stops at first match (short-circuit)
- [ ] `any_of` reports all branch errors when none match
- [ ] `Schema::all_of([s1, s2])` requires all to match
- [ ] `all_of` accumulates errors from all failing schemas
- [ ] `.optional()` allows null values
- [ ] `.optional()` validates non-null against inner schema
- [ ] Combinator errors include branch information
- [ ] Nested combinators work correctly

## Technical Details

### Implementation Approach

```rust
use std::sync::Arc;
use serde_json::Value;
use crate::path::JsonPath;
use crate::error::SchemaErrors;
use stillwater::Validation;

// Type alias for validation function
type ValidatorFn = Arc<dyn Fn(&Value, &JsonPath) -> Validation<Value, SchemaErrors> + Send + Sync>;

pub enum CombinatorSchema {
    OneOf {
        schemas: Vec<ValidatorFn>,
    },
    AnyOf {
        schemas: Vec<ValidatorFn>,
    },
    AllOf {
        schemas: Vec<ValidatorFn>,
    },
    Optional {
        inner: ValidatorFn,
    },
}

impl Schema {
    pub fn one_of<I>(schemas: I) -> CombinatorSchema
    where
        I: IntoIterator<Item = Box<dyn ValueValidator>>,
    {
        let validators: Vec<_> = schemas
            .into_iter()
            .map(|schema| {
                Arc::new(move |value: &Value, path: &JsonPath| {
                    schema.validate_value(value, path)
                }) as ValidatorFn
            })
            .collect();
        CombinatorSchema::OneOf { schemas: validators }
    }

    pub fn any_of<I>(schemas: I) -> CombinatorSchema
    where
        I: IntoIterator<Item = Box<dyn ValueValidator>>,
    {
        let validators: Vec<_> = schemas
            .into_iter()
            .map(|schema| {
                Arc::new(move |value: &Value, path: &JsonPath| {
                    schema.validate_value(value, path)
                }) as ValidatorFn
            })
            .collect();
        CombinatorSchema::AnyOf { schemas: validators }
    }

    pub fn all_of<I>(schemas: I) -> CombinatorSchema
    where
        I: IntoIterator<Item = Box<dyn ValueValidator>>,
    {
        let validators: Vec<_> = schemas
            .into_iter()
            .map(|schema| {
                Arc::new(move |value: &Value, path: &JsonPath| {
                    schema.validate_value(value, path)
                }) as ValidatorFn
            })
            .collect();
        CombinatorSchema::AllOf { schemas: validators }
    }

    pub fn optional(inner: Box<dyn ValueValidator>) -> CombinatorSchema {
        let validator = Arc::new(move |value: &Value, path: &JsonPath| {
            inner.validate_value(value, path)
        }) as ValidatorFn;
        CombinatorSchema::Optional { inner: validator }
    }
}

impl CombinatorSchema {
    fn validate_one_of(
        schemas: &[ValidatorFn],
        value: &Value,
        path: &JsonPath,
    ) -> Validation<Value, SchemaErrors> {
        use stillwater::validation::{success, failure};

        let results: Vec<_> = schemas
            .iter()
            .enumerate()
            .map(|(i, validator)| (i, validator(value, path)))
            .collect();

        let valid: Vec<_> = results
            .iter()
            .enumerate()
            .filter(|(_, (_, r))| r.is_success())
            .collect();

        match valid.len() {
            0 => {
                // None matched - report with count
                let error = SchemaError::new(
                    path.clone(),
                    format!("value did not match any of {} schemas", schemas.len()),
                )
                .with_code("one_of_none_matched");

                failure(SchemaErrors::single(error))
            }
            1 => {
                // Exactly one matched - success
                let (_, (_, result)) = valid.into_iter().next().unwrap();
                // Extract the value from the successful result
                match result {
                    Validation::Success(v) => success(v.clone()),
                    _ => unreachable!(),
                }
            }
            n => {
                // Multiple matched - ambiguous
                let indices: Vec<_> = valid.iter().map(|(_, (i, _))| i).collect();
                let error = SchemaError::new(
                    path.clone(),
                    format!("value matched {} schemas (indices {:?}), expected exactly one", n, indices),
                )
                .with_code("one_of_multiple_matched");

                failure(SchemaErrors::single(error))
            }
        }
    }

    fn validate_any_of(
        schemas: &[ValidatorFn],
        value: &Value,
        path: &JsonPath,
    ) -> Validation<Value, SchemaErrors> {
        use stillwater::validation::{success, failure};

        for validator in schemas {
            match validator(value, path) {
                Validation::Success(v) => return success(v),
                Validation::Failure(_) => continue,
            }
        }

        // None matched
        let error = SchemaError::new(
            path.clone(),
            format!("value did not match any of {} schemas", schemas.len()),
        )
        .with_code("any_of_none_matched");

        failure(SchemaErrors::single(error))
    }

    fn validate_all_of(
        schemas: &[ValidatorFn],
        value: &Value,
        path: &JsonPath,
    ) -> Validation<Value, SchemaErrors> {
        use stillwater::validation::{success, failure};

        let mut all_errors = Vec::new();
        let mut last_valid = None;

        for validator in schemas {
            match validator(value, path) {
                Validation::Success(v) => last_valid = Some(v),
                Validation::Failure(e) => all_errors.extend(e.into_iter()),
            }
        }

        if all_errors.is_empty() {
            success(last_valid.unwrap_or_else(|| value.clone()))
        } else {
            failure(SchemaErrors::from_vec(all_errors).unwrap())
        }
    }

    fn validate_optional(
        inner: &ValidatorFn,
        value: &Value,
        path: &JsonPath,
    ) -> Validation<Value, SchemaErrors> {
        use stillwater::validation::success;

        if value.is_null() {
            success(Value::Null)
        } else {
            inner(value, path)
        }
    }
}
```

### Architecture Changes

- Create `src/schema/combinators.rs` for combinator types
- Add combinator constructors to Schema

### Data Structures

- `CombinatorSchema`: Enum of combinator types
- Each variant holds vector of schemas or single inner schema

### APIs and Interfaces

```rust
// Combinator constructors
impl Schema {
    pub fn one_of<I>(schemas: I) -> CombinatorSchema
    where
        I: IntoIterator<Item = Box<dyn ValueValidator>>;

    pub fn any_of<I>(schemas: I) -> CombinatorSchema
    where
        I: IntoIterator<Item = Box<dyn ValueValidator>>;

    pub fn all_of<I>(schemas: I) -> CombinatorSchema
    where
        I: IntoIterator<Item = Box<dyn ValueValidator>>;

    pub fn optional(inner: Box<dyn ValueValidator>) -> CombinatorSchema;
}

// Validation - CombinatorSchema implements SchemaLike
impl SchemaLike for CombinatorSchema {
    type Output = Value;

    fn validate(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        match self {
            CombinatorSchema::OneOf { schemas } => Self::validate_one_of(schemas, value, path),
            CombinatorSchema::AnyOf { schemas } => Self::validate_any_of(schemas, value, path),
            CombinatorSchema::AllOf { schemas } => Self::validate_all_of(schemas, value, path),
            CombinatorSchema::Optional { inner } => Self::validate_optional(inner, value, path),
        }
    }

    fn validate_to_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        self.validate(value, path)
    }
}
```

## Dependencies

- **Prerequisites**: Specs 001-005 (all basic types)
- **Affected Components**:
  - Schema module (new combinators.rs)
  - Schema traits (add Send + Sync bounds to SchemaLike)
- **External Dependencies**: None

## Prerequisites

Before implementing combinators, the `SchemaLike` trait must be updated to support thread-safe trait objects:

```rust
// In src/schema/traits.rs - UPDATE EXISTING TRAIT
pub trait SchemaLike: Send + Sync {
    type Output;
    fn validate(&self, value: &Value, path: &JsonPath) -> Validation<Self::Output, SchemaErrors>;
    fn validate_to_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors>;
}

// ADD NEW TRAIT for type-erased validation
pub trait ValueValidator: Send + Sync {
    fn validate_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors>;
}

// Blanket implementation
impl<S: SchemaLike> ValueValidator for S {
    fn validate_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        self.validate_to_value(value, path)
    }
}
```

**Rationale**:
- `Send + Sync` bounds enable `SchemaLike` to be used in trait objects (`Box<dyn SchemaLike>`)
- `ValueValidator` provides type erasure for combinators, hiding the different `Output` types
- All existing schema types (StringSchema, IntegerSchema, etc.) already satisfy `Send + Sync`

## Testing Strategy

- **Unit Tests**:
  - one_of with exactly one match
  - one_of with no matches
  - one_of with multiple matches
  - any_of with first match
  - any_of with later match
  - any_of with no matches
  - all_of with all passing
  - all_of with some failing
  - optional with null
  - optional with non-null valid
  - optional with non-null invalid

- **Integration Tests**:
  - Nested combinators
  - Combinators with object schemas
  - Discriminated unions with one_of

- **Edge Cases**:
  - Empty schemas list
  - Single schema in combinator
  - Deeply nested combinators

## Documentation Requirements

- **Code Documentation**: Examples for each combinator
- **User Documentation**: Guide to union types
- **Architecture Updates**: None

## Implementation Notes

- **Thread Safety**: Adding `Send + Sync` to `SchemaLike` is a breaking change, but all existing schema types already satisfy these bounds
- `one_of` must validate all schemas to detect ambiguity (cannot short-circuit)
- `any_of` can and should short-circuit on first match for efficiency
- `all_of` must validate all to accumulate all errors
- Error messages should help users understand which variant to fix
- The `ValueValidator` trait provides type erasure, allowing combinators to work with heterogeneous schemas
- Using `Arc<Fn>` instead of `Box<dyn Trait>` allows the validators to be cloned and shared efficiently
- Empty schema lists should be handled gracefully (likely an error or always succeed/fail)

## Migration and Compatibility

**Breaking Change**: Adding `Send + Sync` bounds to `SchemaLike` trait is technically a breaking change.

**Impact**: Low - all existing schema implementations (StringSchema, IntegerSchema, ObjectSchema, ArraySchema) already satisfy `Send + Sync` automatically. Users implementing custom SchemaLike types would need to ensure their types are thread-safe.

**Migration Path**: If any custom schema types exist that are not `Send + Sync`, wrap them with appropriate synchronization primitives (Arc, Mutex, etc.) or refactor to remove non-thread-safe state.

## Files to Create/Modify

```
src/schema/traits.rs          (MODIFY - add Send + Sync bounds, add ValueValidator trait)
src/schema/combinators.rs     (CREATE - new combinator implementation)
src/schema/mod.rs              (MODIFY - export ValueValidator, add combinator constructors)
src/lib.rs                     (MODIFY - re-export ValueValidator)
tests/combinators_test.rs      (CREATE - comprehensive combinator tests)
```

## Example Usage

```rust
use postmortem::{Schema, JsonPath};
use serde_json::json;

// Discriminated union (tagged) - NOTE: String enum matching would need a separate feature
let circle_schema = Schema::object()
    .field("type", Schema::string())  // In practice, add validation that type == "circle"
    .field("radius", Schema::integer().positive());

let rectangle_schema = Schema::object()
    .field("type", Schema::string())  // In practice, add validation that type == "rectangle"
    .field("width", Schema::integer().positive())
    .field("height", Schema::integer().positive());

let shape = Schema::one_of(vec![
    Box::new(circle_schema) as Box<dyn ValueValidator>,
    Box::new(rectangle_schema) as Box<dyn ValueValidator>,
]);

// Flexible type (string or integer ID)
let id = Schema::any_of(vec![
    Box::new(Schema::string().min_len(1)) as Box<dyn ValueValidator>,
    Box::new(Schema::integer().positive()) as Box<dyn ValueValidator>,
]);

// Schema intersection
let named_entity = Schema::object()
    .field("name", Schema::string().min_len(1));

let timestamped = Schema::object()
    .field("created_at", Schema::string());  // datetime() would be from spec 006

let named_and_timestamped = Schema::all_of(vec![
    Box::new(named_entity) as Box<dyn ValueValidator>,
    Box::new(timestamped) as Box<dyn ValueValidator>,
]);

// Nullable field using optional combinator
let user = Schema::object()
    .field("email", Schema::string())
    .optional("nickname", Schema::string());  // Uses ObjectSchema's optional() method

// Or for a standalone optional schema
let optional_string = Schema::optional(
    Box::new(Schema::string().min_len(1)) as Box<dyn ValueValidator>
);

// Validation
let result = shape.validate(&json!({
    "type": "circle",
    "radius": 5
}), &JsonPath::root());
assert!(result.is_success());

let result = id.validate(&json!("abc-123"), &JsonPath::root());
assert!(result.is_success());

let result = id.validate(&json!(42), &JsonPath::root());
assert!(result.is_success());

let result = optional_string.validate(&json!(null), &JsonPath::root());
assert!(result.is_success());
```

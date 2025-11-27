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
pub enum CombinatorSchema {
    OneOf {
        schemas: Vec<Box<dyn SchemaLike>>,
    },
    AnyOf {
        schemas: Vec<Box<dyn SchemaLike>>,
    },
    AllOf {
        schemas: Vec<Box<dyn SchemaLike>>,
    },
    Optional {
        inner: Box<dyn SchemaLike>,
    },
}

impl Schema {
    pub fn one_of<I>(schemas: I) -> CombinatorSchema
    where
        I: IntoIterator<Item = Box<dyn SchemaLike>>,
    {
        CombinatorSchema::OneOf {
            schemas: schemas.into_iter().collect(),
        }
    }

    pub fn any_of<I>(schemas: I) -> CombinatorSchema
    where
        I: IntoIterator<Item = Box<dyn SchemaLike>>,
    {
        CombinatorSchema::AnyOf {
            schemas: schemas.into_iter().collect(),
        }
    }

    pub fn all_of<I>(schemas: I) -> CombinatorSchema
    where
        I: IntoIterator<Item = Box<dyn SchemaLike>>,
    {
        CombinatorSchema::AllOf {
            schemas: schemas.into_iter().collect(),
        }
    }
}

impl CombinatorSchema {
    fn validate_one_of(
        &self,
        schemas: &[Box<dyn SchemaLike>],
        value: &Value,
        path: &JsonPath,
    ) -> Validation<ValidatedValue, SchemaErrors> {
        let results: Vec<_> = schemas
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.validate(value, path)))
            .collect();

        let valid: Vec<_> = results
            .iter()
            .filter(|(_, r)| r.is_valid())
            .collect();

        match valid.len() {
            0 => {
                // None matched - collect all branch errors
                let branch_errors: Vec<_> = results
                    .into_iter()
                    .filter_map(|(i, r)| match r {
                        Validation::Invalid(e) => Some((i, e)),
                        _ => None,
                    })
                    .collect();

                let error = SchemaError::new(
                    path.clone(),
                    format!("value did not match any of {} schemas", schemas.len()),
                )
                .with_code("one_of_none_matched");

                Validation::invalid(SchemaErrors::single(error))
            }
            1 => {
                // Exactly one matched - success
                let (_, result) = valid.into_iter().next().unwrap();
                result.clone()
            }
            n => {
                // Multiple matched - ambiguous
                let indices: Vec<_> = valid.iter().map(|(i, _)| i).collect();
                let error = SchemaError::new(
                    path.clone(),
                    format!("value matched {} schemas (indices {:?}), expected exactly one", n, indices),
                )
                .with_code("one_of_multiple_matched");

                Validation::invalid(SchemaErrors::single(error))
            }
        }
    }

    fn validate_any_of(
        &self,
        schemas: &[Box<dyn SchemaLike>],
        value: &Value,
        path: &JsonPath,
    ) -> Validation<ValidatedValue, SchemaErrors> {
        let mut all_errors = Vec::new();

        for schema in schemas {
            match schema.validate(value, path) {
                Validation::Valid(v) => return Validation::valid(v),
                Validation::Invalid(e) => all_errors.extend(e.into_iter()),
            }
        }

        // None matched
        let error = SchemaError::new(
            path.clone(),
            format!("value did not match any of {} schemas", schemas.len()),
        )
        .with_code("any_of_none_matched");

        Validation::invalid(SchemaErrors::single(error))
    }

    fn validate_all_of(
        &self,
        schemas: &[Box<dyn SchemaLike>],
        value: &Value,
        path: &JsonPath,
    ) -> Validation<ValidatedValue, SchemaErrors> {
        let mut all_errors = Vec::new();
        let mut last_valid = None;

        for schema in schemas {
            match schema.validate(value, path) {
                Validation::Valid(v) => last_valid = Some(v),
                Validation::Invalid(e) => all_errors.extend(e.into_iter()),
            }
        }

        if all_errors.is_empty() {
            Validation::valid(last_valid.unwrap())
        } else {
            Validation::invalid(SchemaErrors::from_vec(all_errors).unwrap())
        }
    }

    fn validate_optional(
        &self,
        inner: &dyn SchemaLike,
        value: &Value,
        path: &JsonPath,
    ) -> Validation<ValidatedValue, SchemaErrors> {
        if value.is_null() {
            Validation::valid(ValidatedValue::Null)
        } else {
            inner.validate(value, path)
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
Schema::one_of<I>(schemas: I) -> CombinatorSchema
Schema::any_of<I>(schemas: I) -> CombinatorSchema
Schema::all_of<I>(schemas: I) -> CombinatorSchema

// Optional wrapper (on any schema)
impl<S: SchemaLike> S {
    fn optional(self) -> CombinatorSchema
}

// Or standalone
Schema::optional<S: SchemaLike>(schema: S) -> CombinatorSchema

// Validation
CombinatorSchema::validate(&self, value: &Value, path: &JsonPath) -> Validation<ValidatedValue, SchemaErrors>
```

## Dependencies

- **Prerequisites**: Specs 001-005 (all basic types)
- **Affected Components**: Schema module
- **External Dependencies**: None

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

- `one_of` must validate all schemas to detect ambiguity
- `any_of` can short-circuit on first match
- `all_of` must validate all to accumulate errors
- Consider lazy evaluation for complex nested schemas
- Error messages should help users understand which variant to fix

## Migration and Compatibility

No migration needed - new functionality.

## Files to Create/Modify

```
src/schema/combinators.rs
tests/combinators_test.rs
```

## Example Usage

```rust
use postmortem::Schema;

// Discriminated union (tagged)
let shape = Schema::one_of([
    Schema::object()
        .field("type", Schema::string().one_of(["circle"]))
        .field("radius", Schema::integer().positive()),
    Schema::object()
        .field("type", Schema::string().one_of(["rectangle"]))
        .field("width", Schema::integer().positive())
        .field("height", Schema::integer().positive()),
]);

// Flexible type (string or integer ID)
let id = Schema::any_of([
    Schema::string().min_len(1),
    Schema::integer().positive(),
]);

// Schema intersection
let named_entity = Schema::object()
    .field("name", Schema::string().min_len(1));

let timestamped = Schema::object()
    .field("created_at", Schema::string().datetime());

let named_and_timestamped = Schema::all_of([
    named_entity,
    timestamped,
]);

// Nullable field
let user = Schema::object()
    .field("email", Schema::string().email())
    .field("nickname", Schema::string().optional());  // Can be null

// Validation
let result = shape.validate(&json!({
    "type": "circle",
    "radius": 5
}), &JsonPath::root());
assert!(result.is_valid());

let result = id.validate(&json!("abc-123"), &JsonPath::root());
assert!(result.is_valid());

let result = id.validate(&json!(42), &JsonPath::root());
assert!(result.is_valid());
```

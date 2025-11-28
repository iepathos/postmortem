---
number: 3
title: Integer Schema Validation
category: foundation
priority: critical
status: draft
dependencies: [1]
created: 2025-11-26
---

# Specification 003: Integer Schema Validation

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: Spec 001 (Core Types and Error Foundation)

## Context

Integer validation is essential for API validation, handling things like pagination limits, quantities, ages, and other numeric values. This specification defines the integer schema type with range and sign constraints, following the same builder pattern established by the string schema.

## Objective

Implement an integer schema type that:
1. Validates values are integers (not floats)
2. Applies range constraints (min/max)
3. Validates sign requirements (positive, non-negative, negative)
4. Accumulates all constraint violations
5. Supports custom error messages

## Requirements

### Functional Requirements

1. **Integer Schema Construction**
   - `Schema::integer()` creates a new integer schema
   - Schemas are immutable and composable via builder methods
   - Each constraint returns a new schema (functional builder pattern)

2. **Range Constraints**
   - `.min(n)` - minimum value (inclusive)
   - `.max(n)` - maximum value (inclusive)
   - `.range(start..=end)` - convenience for min/max together
   - Support for i64 values

3. **Sign Constraints**
   - `.positive()` - value must be > 0
   - `.non_negative()` - value must be >= 0
   - `.negative()` - value must be < 0

4. **Custom Error Messages**
   - `.error(message)` - override default error message
   - Custom message applies to the most recent constraint

5. **Validation Behavior**
   - Return `Validation::Failure` with type error if value is not an integer
   - Reject floating point numbers (even 1.0)
   - Accumulate all constraint violations
   - Return `Validation::Success` with validated i64 on success
   - Use stillwater's `success()`/`failure()` helper functions

### Non-Functional Requirements

- Support full i64 range
- Validation should be efficient (no allocations on success)
- Clear error messages with actual and expected values

## Acceptance Criteria

- [ ] `Schema::integer()` creates an integer schema
- [ ] `.min(5)` rejects integers less than 5
- [ ] `.max(10)` rejects integers greater than 10
- [ ] `.range(5..=10)` validates both min and max
- [ ] Both range violations reported for value outside range
- [ ] `.positive()` rejects 0 and negative values
- [ ] `.non_negative()` accepts 0 but rejects negatives
- [ ] `.negative()` rejects 0 and positive values
- [ ] `.error("custom")` overrides default error message
- [ ] Non-integer values produce type error with code `invalid_type`
- [ ] Float values (e.g., 1.0) produce type error
- [ ] Range errors use codes `min_value` and `max_value`
- [ ] Sign errors use codes `positive`, `non_negative`, `negative`
- [ ] All constraint errors accumulate in a single validation call

## Technical Details

### Implementation Approach

```rust
pub struct IntegerSchema {
    constraints: Vec<IntegerConstraint>,
}

enum IntegerConstraint {
    Min { value: i64, message: Option<String> },
    Max { value: i64, message: Option<String> },
    Positive { message: Option<String> },
    NonNegative { message: Option<String> },
    Negative { message: Option<String> },
}

impl Schema {
    pub fn integer() -> IntegerSchema {
        IntegerSchema {
            constraints: vec![],
        }
    }
}

impl IntegerSchema {
    pub fn min(mut self, value: i64) -> Self {
        self.constraints.push(IntegerConstraint::Min {
            value,
            message: None,
        });
        self
    }

    pub fn max(mut self, value: i64) -> Self {
        self.constraints.push(IntegerConstraint::Max {
            value,
            message: None,
        });
        self
    }

    pub fn range(self, range: RangeInclusive<i64>) -> Self {
        self.min(*range.start()).max(*range.end())
    }

    pub fn positive(mut self) -> Self {
        self.constraints.push(IntegerConstraint::Positive {
            message: None,
        });
        self
    }

    pub fn non_negative(mut self) -> Self {
        self.constraints.push(IntegerConstraint::NonNegative {
            message: None,
        });
        self
    }

    pub fn negative(mut self) -> Self {
        self.constraints.push(IntegerConstraint::Negative {
            message: None,
        });
        self
    }

    pub fn error(mut self, message: impl Into<String>) -> Self {
        if let Some(last) = self.constraints.last_mut() {
            match last {
                IntegerConstraint::Min { message: m, .. } => *m = Some(message.into()),
                IntegerConstraint::Max { message: m, .. } => *m = Some(message.into()),
                IntegerConstraint::Positive { message: m } => *m = Some(message.into()),
                IntegerConstraint::NonNegative { message: m } => *m = Some(message.into()),
                IntegerConstraint::Negative { message: m } => *m = Some(message.into()),
            }
        }
        self
    }

    pub fn validate(&self, value: &Value, path: &JsonPath) -> Validation<i64, SchemaErrors> {
        use stillwater::validation::{success, failure};

        // Check for integer (not float)
        let n = match value {
            Value::Number(n) if n.is_i64() => n.as_i64().unwrap(),
            Value::Number(n) if n.is_f64() => {
                return failure(SchemaErrors::single(
                    SchemaError::new(path.clone(), "expected integer, got float")
                        .with_code("invalid_type")
                        .with_got("float")
                        .with_expected("integer")
                ))
            }
            _ => {
                return failure(SchemaErrors::single(
                    SchemaError::new(path.clone(), "expected integer")
                        .with_code("invalid_type")
                        .with_got(value_type_name(value))
                        .with_expected("integer")
                ))
            }
        };

        // Collect all constraint violations
        let errors: Vec<SchemaError> = self.constraints
            .iter()
            .filter_map(|c| self.check_constraint(c, n, path))
            .collect();

        if errors.is_empty() {
            success(n)
        } else {
            failure(SchemaErrors::from_vec(errors).unwrap())
        }
    }
}
```

### Architecture Changes

- Create `src/schema/numeric.rs` for integer (and later float) schemas
- Add integer schema to Schema entry point

### Data Structures

- `IntegerSchema`: Contains vector of constraints
- `IntegerConstraint`: Enum of constraint types

### APIs and Interfaces

```rust
// Construction
Schema::integer() -> IntegerSchema

// Constraints (builder pattern)
IntegerSchema::min(self, value: i64) -> Self
IntegerSchema::max(self, value: i64) -> Self
IntegerSchema::range(self, range: RangeInclusive<i64>) -> Self
IntegerSchema::positive(self) -> Self
IntegerSchema::non_negative(self) -> Self
IntegerSchema::negative(self) -> Self

// Custom error
IntegerSchema::error(self, message: impl Into<String>) -> Self

// Validation
IntegerSchema::validate(&self, value: &Value, path: &JsonPath) -> Validation<i64, SchemaErrors>
```

## Dependencies

- **Prerequisites**: Spec 001 (Core Types and Error Foundation)
- **Affected Components**: Schema module
- **External Dependencies**:
  - `serde_json` for `Value` type

## Testing Strategy

- **Unit Tests**:
  - Integer schema creation
  - Min value validation (pass/fail)
  - Max value validation (pass/fail)
  - Range validation
  - Positive constraint
  - Non-negative constraint (including 0)
  - Negative constraint
  - Custom error messages
  - Type error for non-integers
  - Type error for floats

- **Edge Cases**:
  - i64::MIN and i64::MAX values
  - Zero handling for sign constraints
  - Float with .0 (e.g., 5.0 should fail)

## Documentation Requirements

- **Code Documentation**: Rustdoc with examples for each constraint
- **User Documentation**: Examples of common integer validations
- **Architecture Updates**: None needed beyond spec 002

## Implementation Notes

- Use `is_i64()` to distinguish integers from floats
- `5.0` in JSON is a float and should be rejected
- Range constraints should allow equal min/max (for exact value)
- Consider unsigned integers (u64) support in future if needed

## Migration and Compatibility

No migration needed - this is new code.

## Files to Create/Modify

```
src/schema/numeric.rs
tests/integer_test.rs
```

## Example Usage

```rust
use postmortem::Schema;

// Age validation (0-150)
let age_schema = Schema::integer()
    .non_negative()
    .max(150)
    .error("age must be between 0 and 150");

// Pagination
let page_schema = Schema::integer()
    .positive()
    .error("page must be positive");

let limit_schema = Schema::integer()
    .range(1..=100)
    .error("limit must be between 1 and 100");

// Validation
let result = age_schema.validate(&json!(25), &JsonPath::root());
assert!(result.is_success());

let result = age_schema.validate(&json!(-5), &JsonPath::root());
assert!(result.is_failure());
// Errors: "must be non-negative", "age must be between 0 and 150"
```

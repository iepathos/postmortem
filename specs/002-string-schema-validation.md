---
number: 2
title: String Schema Validation
category: foundation
priority: critical
status: draft
dependencies: [1]
created: 2025-11-26
---

# Specification 002: String Schema Validation

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: Spec 001 (Core Types and Error Foundation)

## Context

String validation is one of the most common validation requirements in API and data validation. Users need to validate string length, patterns, and apply custom error messages. This specification defines the string schema type and its core constraints, establishing the pattern that other schema types will follow.

## Objective

Implement a string schema type that:
1. Validates values are strings
2. Applies length constraints (min/max)
3. Validates against regex patterns
4. Supports custom error messages
5. Accumulates all constraint violations

## Requirements

### Functional Requirements

1. **String Schema Construction**
   - `Schema::string()` creates a new string schema
   - Schemas are immutable and composable via builder methods
   - Each constraint returns a new schema (functional builder pattern)

2. **Length Constraints**
   - `.min_len(n)` - minimum character length
   - `.max_len(n)` - maximum character length
   - Both constraints can be applied simultaneously
   - Clear error messages indicating the constraint and actual length

3. **Pattern Constraints**
   - `.pattern(regex)` - value must match the regex
   - Support for precompiled `Regex` objects
   - Support for string patterns (compiled on schema creation)
   - Clear error message when pattern fails

4. **Custom Error Messages**
   - `.error(message)` - override default error message
   - Custom message applies to the most recent constraint
   - Default messages are clear and actionable

5. **Validation Behavior**
   - Return `Validation::invalid` with type error if value is not a string
   - Accumulate all constraint violations (not just the first)
   - Return `Validation::valid` with the validated string on success

### Non-Functional Requirements

- Regex compilation should happen at schema construction, not validation
- Validation should be efficient for repeated calls
- API should be ergonomic and IDE-friendly
- Clear separation between schema definition and validation execution

## Acceptance Criteria

- [ ] `Schema::string()` creates a string schema
- [ ] `.min_len(5)` rejects strings shorter than 5 characters
- [ ] `.max_len(10)` rejects strings longer than 10 characters
- [ ] Combining `.min_len(5).max_len(10)` validates both constraints
- [ ] Both length violations are reported if string is 2 chars with max 10
- [ ] `.pattern(r"^\d+$")` validates string matches regex
- [ ] Pattern failure includes the pattern in error message
- [ ] `.error("custom message")` overrides default error message
- [ ] Non-string values produce type error with code `invalid_type`
- [ ] Length errors use code `min_length` and `max_length`
- [ ] Pattern errors use code `pattern`
- [ ] All constraint errors accumulate in a single validation call
- [ ] Validated strings are returned on success

## Technical Details

### Implementation Approach

```rust
pub struct StringSchema {
    constraints: Vec<StringConstraint>,
    custom_error: Option<String>,
}

enum StringConstraint {
    MinLength { min: usize, message: Option<String> },
    MaxLength { max: usize, message: Option<String> },
    Pattern { regex: Regex, pattern_str: String, message: Option<String> },
}

impl Schema {
    pub fn string() -> StringSchema {
        StringSchema {
            constraints: vec![],
            custom_error: None,
        }
    }
}

impl StringSchema {
    pub fn min_len(mut self, min: usize) -> Self {
        self.constraints.push(StringConstraint::MinLength {
            min,
            message: None
        });
        self
    }

    pub fn max_len(mut self, max: usize) -> Self {
        self.constraints.push(StringConstraint::MaxLength {
            max,
            message: None
        });
        self
    }

    pub fn pattern(mut self, pattern: &str) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        self.constraints.push(StringConstraint::Pattern {
            regex,
            pattern_str: pattern.to_string(),
            message: None,
        });
        Ok(self)
    }

    pub fn error(mut self, message: impl Into<String>) -> Self {
        // Apply to most recent constraint or schema-level
        if let Some(last) = self.constraints.last_mut() {
            match last {
                StringConstraint::MinLength { message: m, .. } => *m = Some(message.into()),
                StringConstraint::MaxLength { message: m, .. } => *m = Some(message.into()),
                StringConstraint::Pattern { message: m, .. } => *m = Some(message.into()),
            }
        } else {
            self.custom_error = Some(message.into());
        }
        self
    }

    pub fn validate(&self, value: &Value, path: &JsonPath) -> Validation<String, SchemaErrors> {
        // First check if it's a string
        let s = match value.as_str() {
            Some(s) => s,
            None => return Validation::invalid(SchemaErrors::single(
                SchemaError::new(path.clone(), "expected string")
                    .with_code("invalid_type")
                    .with_got(value_type_name(value))
                    .with_expected("string")
            )),
        };

        // Collect all constraint violations
        let errors: Vec<SchemaError> = self.constraints
            .iter()
            .filter_map(|c| self.check_constraint(c, s, path))
            .collect();

        if errors.is_empty() {
            Validation::valid(s.to_string())
        } else {
            Validation::invalid(SchemaErrors::from_vec(errors).unwrap())
        }
    }
}
```

### Architecture Changes

- Create `src/schema/mod.rs` as schema module root
- Create `src/schema/string.rs` for string schema implementation
- Define `Schema` as entry point for schema construction

### Data Structures

- `StringSchema`: Contains constraints and custom error
- `StringConstraint`: Enum of constraint types with optional messages

### APIs and Interfaces

```rust
// Construction
Schema::string() -> StringSchema

// Constraints (builder pattern)
StringSchema::min_len(self, min: usize) -> Self
StringSchema::max_len(self, max: usize) -> Self
StringSchema::pattern(self, pattern: &str) -> Result<Self, regex::Error>

// Custom error
StringSchema::error(self, message: impl Into<String>) -> Self

// Validation
StringSchema::validate(&self, value: &Value, path: &JsonPath) -> Validation<String, SchemaErrors>
```

## Dependencies

- **Prerequisites**: Spec 001 (Core Types and Error Foundation)
- **Affected Components**: None (new code)
- **External Dependencies**:
  - `regex` crate for pattern matching
  - `serde_json` for `Value` type

## Testing Strategy

- **Unit Tests**:
  - String schema creation
  - Min length validation (pass/fail cases)
  - Max length validation (pass/fail cases)
  - Combined length validation
  - Pattern validation (pass/fail cases)
  - Custom error messages
  - Type error for non-strings
  - Multiple constraint accumulation

- **Integration Tests**:
  - String validation with path tracking
  - Error message formatting

- **Edge Cases**:
  - Empty string validation
  - Unicode string length (characters vs bytes)
  - Empty pattern
  - Invalid regex patterns

## Documentation Requirements

- **Code Documentation**: Rustdoc with examples for each constraint
- **User Documentation**: Examples of common string validations
- **Architecture Updates**: Document schema builder pattern

## Implementation Notes

- Length should count Unicode scalar values, not bytes
- Regex compilation errors should be handled at schema construction
- Consider caching compiled regex patterns
- Builder methods should be chainable without intermediate variables
- The `.error()` method applies to the most recently added constraint

## Migration and Compatibility

No migration needed - this is new code. The API pattern established here will be followed by all other schema types.

## Files to Create/Modify

```
src/schema/mod.rs
src/schema/string.rs
tests/string_test.rs
```

## Example Usage

```rust
use postmortem::Schema;

// Simple string validation
let schema = Schema::string().min_len(1).max_len(100);

// With pattern
let email_like = Schema::string()
    .pattern(r"@")
    .unwrap()
    .error("must contain @");

// Validation
let result = schema.validate(&json!("hello"), &JsonPath::root());
assert!(result.is_valid());

let result = schema.validate(&json!(""), &JsonPath::root());
assert!(result.is_invalid());
// Error: "length must be at least 1, got 0"
```

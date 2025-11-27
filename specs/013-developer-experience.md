---
number: 13
title: Developer Experience
category: testing
priority: medium
status: draft
dependencies: [1, 2, 3, 4, 5]
created: 2025-11-26
---

# Specification 013: Developer Experience

**Category**: testing
**Priority**: medium
**Status**: draft
**Dependencies**: Specs 001-005 (Core through Array)

## Context

A validation library is only as good as its developer experience. This specification focuses on testing utilities, assertion macros, and documentation that make postmortem pleasant to use in real projects. Good test helpers reduce boilerplate and make validation tests readable and maintainable.

## Objective

Implement developer experience improvements:
1. Test assertion macros for validation results
2. Property-based test generators for formats
3. Comprehensive rustdoc documentation
4. Runnable examples for common use cases

## Requirements

### Functional Requirements

1. **Test Assertion Macros**
   - `assert_valid!(result)` - assert validation succeeded
   - `assert_invalid!(result)` - assert validation failed
   - `assert_error_at!(errors, path)` - assert error at specific path
   - `assert_error_at!(errors, path, code)` - assert error with specific code
   - `assert_error_count!(errors, n)` - assert exact error count

2. **Property Testing Generators** (feature: `proptest`)
   - `valid_email()` - generates valid email strings
   - `invalid_email()` - generates invalid email strings
   - `valid_url()` - generates valid URLs
   - `valid_uuid()` - generates valid UUIDs
   - `valid_date()` - generates ISO 8601 dates
   - `arbitrary_for_schema(schema)` - generate values matching schema

3. **Debug Output Helpers**
   - Pretty-print validation errors
   - Color-coded output in test failures
   - Path highlighting for error location

4. **Documentation**
   - Comprehensive rustdoc on all public items
   - Examples in doc comments
   - Module-level documentation with guides
   - README with quick start

### Non-Functional Requirements

- Test macros provide clear failure messages
- Property generators are efficient
- Documentation builds without warnings
- Examples are tested via doctest

## Acceptance Criteria

- [ ] `assert_valid!(result)` passes for valid results
- [ ] `assert_valid!(result)` shows errors on failure
- [ ] `assert_invalid!(result)` passes for invalid results
- [ ] `assert_error_at!(errors, "email")` finds error at path
- [ ] `assert_error_at!(errors, "email", "invalid_email")` checks code
- [ ] `assert_error_count!(errors, 2)` checks count
- [ ] `valid_email()` generates only valid emails
- [ ] `invalid_email()` generates only invalid emails
- [ ] All public items have rustdoc
- [ ] Examples compile and pass

## Technical Details

### Implementation Approach

```rust
// src/test_helpers.rs

/// Assert that validation result is valid
#[macro_export]
macro_rules! assert_valid {
    ($result:expr) => {
        match &$result {
            $crate::Validation::Valid(_) => {}
            $crate::Validation::Invalid(errors) => {
                panic!(
                    "Expected valid result, got {} error(s):\n{}",
                    errors.len(),
                    $crate::test_helpers::format_errors(errors)
                );
            }
        }
    };
}

/// Assert that validation result is invalid
#[macro_export]
macro_rules! assert_invalid {
    ($result:expr) => {
        match &$result {
            $crate::Validation::Invalid(_) => {}
            $crate::Validation::Valid(v) => {
                panic!(
                    "Expected invalid result, got valid: {:?}",
                    v
                );
            }
        }
    };
    ($result:expr, $count:expr) => {
        match &$result {
            $crate::Validation::Invalid(errors) => {
                assert_eq!(
                    errors.len(),
                    $count,
                    "Expected {} errors, got {}:\n{}",
                    $count,
                    errors.len(),
                    $crate::test_helpers::format_errors(errors)
                );
            }
            $crate::Validation::Valid(v) => {
                panic!(
                    "Expected invalid result with {} errors, got valid: {:?}",
                    $count,
                    v
                );
            }
        }
    };
}

/// Assert error exists at specific path
#[macro_export]
macro_rules! assert_error_at {
    ($errors:expr, $path:expr) => {
        let path_str = $path.to_string();
        let found = $errors.iter().any(|e| e.path.to_string() == path_str);
        if !found {
            panic!(
                "Expected error at path '{}', but no error found there.\nActual errors:\n{}",
                path_str,
                $crate::test_helpers::format_errors($errors)
            );
        }
    };
    ($errors:expr, $path:expr, $code:expr) => {
        let path_str = $path.to_string();
        let code_str = $code.to_string();
        let found = $errors.iter().any(|e| {
            e.path.to_string() == path_str && e.code == code_str
        });
        if !found {
            panic!(
                "Expected error at path '{}' with code '{}', but not found.\nActual errors:\n{}",
                path_str,
                code_str,
                $crate::test_helpers::format_errors($errors)
            );
        }
    };
}

/// Assert exact error count
#[macro_export]
macro_rules! assert_error_count {
    ($errors:expr, $count:expr) => {
        assert_eq!(
            $errors.len(),
            $count,
            "Expected {} errors, got {}:\n{}",
            $count,
            $errors.len(),
            $crate::test_helpers::format_errors($errors)
        );
    };
}

pub mod test_helpers {
    use crate::SchemaErrors;

    /// Format errors for display in test failures
    pub fn format_errors(errors: &SchemaErrors) -> String {
        errors
            .iter()
            .enumerate()
            .map(|(i, e)| {
                format!(
                    "  {}. {} [{}]: {}",
                    i + 1,
                    e.path,
                    e.code,
                    e.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Pretty-print errors with colors (for terminal)
    #[cfg(feature = "colored")]
    pub fn format_errors_colored(errors: &SchemaErrors) -> String {
        use colored::Colorize;

        errors
            .iter()
            .enumerate()
            .map(|(i, e)| {
                format!(
                    "  {}. {} [{}]: {}",
                    (i + 1).to_string().bold(),
                    e.path.to_string().yellow(),
                    e.code.red(),
                    e.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// Property testing generators (feature-gated)
#[cfg(feature = "proptest")]
pub mod generators {
    use proptest::prelude::*;

    /// Generate valid email addresses
    pub fn valid_email() -> impl Strategy<Value = String> {
        (
            "[a-z]{1,10}",           // local part
            "[a-z]{1,10}",           // domain
            prop_oneof!["com", "org", "net", "io"],
        )
            .prop_map(|(local, domain, tld)| format!("{}@{}.{}", local, domain, tld))
    }

    /// Generate invalid email addresses
    pub fn invalid_email() -> impl Strategy<Value = String> {
        prop_oneof![
            // Missing @
            "[a-z]{5,10}".prop_map(|s| s),
            // Missing domain
            "[a-z]{5}@".prop_map(|s| s),
            // Double @
            "[a-z]{3}@@[a-z]{3}\\.[a-z]{3}".prop_map(|s| s),
            // Invalid characters
            "[a-z]{3}@[a-z]{3}\\.[a-z]{3} ".prop_map(|s| s),
        ]
    }

    /// Generate valid URLs
    pub fn valid_url() -> impl Strategy<Value = String> {
        (
            prop_oneof!["http", "https"],
            "[a-z]{1,10}",
            prop_oneof!["com", "org", "net", "io"],
            proptest::option::of("[a-z]{1,10}"),
        )
            .prop_map(|(scheme, domain, tld, path)| {
                match path {
                    Some(p) => format!("{}://{}.{}/{}", scheme, domain, tld, p),
                    None => format!("{}://{}.{}", scheme, domain, tld),
                }
            })
    }

    /// Generate valid UUIDs
    pub fn valid_uuid() -> impl Strategy<Value = String> {
        "[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}"
    }

    /// Generate valid ISO 8601 dates
    pub fn valid_date() -> impl Strategy<Value = String> {
        (1970i32..2100, 1u32..13, 1u32..29)
            .prop_map(|(y, m, d)| format!("{:04}-{:02}-{:02}", y, m, d))
    }

    /// Generate valid ISO 8601 datetimes
    pub fn valid_datetime() -> impl Strategy<Value = String> {
        (valid_date(), 0u32..24, 0u32..60, 0u32..60)
            .prop_map(|(date, h, m, s)| {
                format!("{}T{:02}:{:02}:{:02}Z", date, h, m, s)
            })
    }

    /// Generate positive integers
    pub fn positive_integer() -> impl Strategy<Value = i64> {
        1i64..i64::MAX
    }

    /// Generate non-negative integers
    pub fn non_negative_integer() -> impl Strategy<Value = i64> {
        0i64..i64::MAX
    }

    /// Generate strings within length bounds
    pub fn string_with_length(min: usize, max: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(proptest::char::any(), min..=max)
            .prop_map(|chars| chars.into_iter().collect())
    }
}
```

### Documentation Structure

```rust
//! # postmortem
//!
//! Runtime schema validation with comprehensive error accumulation.
//!
//! ## Quick Start
//!
//! ```rust
//! use postmortem::{Schema, JsonPath, assert_valid, assert_invalid};
//! use serde_json::json;
//!
//! // Define a schema
//! let user_schema = Schema::object()
//!     .field("email", Schema::string().email())
//!     .field("age", Schema::integer().min(0).max(150));
//!
//! // Validate data
//! let result = user_schema.validate(&json!({
//!     "email": "user@example.com",
//!     "age": 25
//! }), &JsonPath::root());
//!
//! assert_valid!(result);
//! ```
//!
//! ## Key Features
//!
//! - **Error Accumulation**: Collects ALL validation errors, not just the first
//! - **Clear Paths**: Error locations like `users[0].email` for easy debugging
//! - **stillwater Integration**: Native `Validation` type for functional composition
//! - **Schema Interop**: Export to JSON Schema and OpenAPI formats
//!
//! ## Schema Types
//!
//! - [`Schema::string()`] - String validation with formats
//! - [`Schema::integer()`] - Integer validation with ranges
//! - [`Schema::object()`] - Object validation with fields
//! - [`Schema::array()`] - Array validation with items
//! - [`Schema::one_of()`] - Union types (exactly one match)
//! - [`Schema::any_of()`] - Union types (at least one match)
//!
//! ## Testing Utilities
//!
//! ```rust
//! use postmortem::{assert_valid, assert_invalid, assert_error_at};
//!
//! // Assert validation passes
//! assert_valid!(result);
//!
//! // Assert validation fails with specific errors
//! assert_invalid!(result);
//! assert_error_at!(errors, "email", "invalid_email");
//! ```
```

### Architecture Changes

- Create `src/test_helpers.rs` for test utilities
- Create `src/generators.rs` for property testing (feature-gated)
- Add comprehensive rustdoc to all modules

### Data Structures

- Macros for test assertions
- Strategy types for property testing

### APIs and Interfaces

```rust
// Test assertion macros
assert_valid!(result)
assert_invalid!(result)
assert_invalid!(result, count)
assert_error_at!(errors, path)
assert_error_at!(errors, path, code)
assert_error_count!(errors, count)

// Helper functions
test_helpers::format_errors(&errors) -> String
test_helpers::format_errors_colored(&errors) -> String

// Property generators (feature: proptest)
generators::valid_email() -> impl Strategy<Value = String>
generators::invalid_email() -> impl Strategy<Value = String>
generators::valid_url() -> impl Strategy<Value = String>
generators::valid_uuid() -> impl Strategy<Value = String>
generators::valid_date() -> impl Strategy<Value = String>
generators::valid_datetime() -> impl Strategy<Value = String>
generators::positive_integer() -> impl Strategy<Value = i64>
generators::string_with_length(min, max) -> impl Strategy<Value = String>
```

## Dependencies

- **Prerequisites**: Specs 001-005
- **Affected Components**: None (new utilities)
- **External Dependencies** (optional):
  - `proptest` for property testing
  - `colored` for colored output

## Testing Strategy

- **Unit Tests**:
  - Macro behavior with valid/invalid results
  - Error formatting output
  - Generator output validity

- **Property Tests**:
  - Generated emails are valid/invalid as expected
  - Generated URLs are valid
  - Generated dates are valid ISO 8601

- **Doc Tests**:
  - All examples in rustdoc compile and pass

## Documentation Requirements

- **Code Documentation**: Comprehensive rustdoc
- **User Documentation**: README with examples
- **Architecture Updates**: Module documentation

## Implementation Notes

- Macros should work in both sync and async tests
- Error formatting should be readable even with many errors
- Property generators should be composable
- Consider `#[track_caller]` for better panic locations

## Migration and Compatibility

No migration needed - new test utilities.

## Files to Create/Modify

```
src/test_helpers.rs
src/generators.rs
src/lib.rs (documentation)
examples/basic_validation.rs
examples/cross_field.rs
examples/api_validation.rs
examples/schema_registry.rs
```

## Example Usage

```rust
use postmortem::{Schema, JsonPath, assert_valid, assert_invalid, assert_error_at};
use serde_json::json;

#[test]
fn test_user_validation() {
    let schema = Schema::object()
        .field("email", Schema::string().email())
        .field("age", Schema::integer().min(0));

    // Valid user
    let result = schema.validate(&json!({
        "email": "test@example.com",
        "age": 25
    }), &JsonPath::root());
    assert_valid!(result);

    // Invalid user - multiple errors
    let result = schema.validate(&json!({
        "email": "not-an-email",
        "age": -5
    }), &JsonPath::root());

    assert_invalid!(result, 2);  // Expect 2 errors

    if let Validation::Invalid(errors) = result {
        assert_error_at!(errors, "email", "invalid_email");
        assert_error_at!(errors, "age", "min_value");
    }
}

// Property-based testing
#[cfg(feature = "proptest")]
mod prop_tests {
    use super::*;
    use postmortem::generators::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn valid_emails_pass_validation(email in valid_email()) {
            let schema = Schema::string().email();
            let result = schema.validate(&json!(email), &JsonPath::root());
            assert_valid!(result);
        }

        #[test]
        fn invalid_emails_fail_validation(email in invalid_email()) {
            let schema = Schema::string().email();
            let result = schema.validate(&json!(email), &JsonPath::root());
            assert_invalid!(result);
        }
    }
}
```

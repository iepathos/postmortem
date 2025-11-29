---
number: 8
title: Cross-Field Validation
category: foundation
priority: high
status: draft
dependencies: [1, 4]
created: 2025-11-26
---

# Specification 008: Cross-Field Validation

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Specs 001, 004 (Core Types, Object Schema)

## Context

Many validation requirements involve relationships between fields rather than individual field constraints. Examples include:
- Date range validation (start_date must be before end_date)
- Conditional requirements (if payment_method is "card", card_number is required)
- Mutual exclusion (either email OR phone is required, not both)
- Field value dependencies (confirmation must match password)

This specification extends object schemas with cross-field validation capabilities.

## Objective

Enable validation of relationships between fields:
1. Object-level custom validators with access to all fields
2. Conditional field requirements
3. Field comparison constraints
4. Mutual exclusion rules
5. Clear error messages indicating involved fields

## Requirements

### Functional Requirements

1. **Object Custom Validation**
   - `.custom(fn)` on ObjectSchema
   - Custom function receives entire validated object
   - Returns `Validation<(), SchemaErrors>` using stillwater's constructors
   - Use `success(())` for passing, `failure(errors)` for failing
   - Runs after all field validations pass
   - Multiple custom validators can be chained

2. **Validation Context**
   - Access to all field values
   - Access to current path for error construction
   - Helper methods for common patterns

3. **Common Patterns (Helper Methods)**
   - `.require_if(field, condition, required_field)` - conditional required
   - `.mutually_exclusive(field1, field2)` - at most one present
   - `.at_least_one_of(fields)` - at least one present
   - `.equal_fields(field1, field2)` - values must match

4. **Date/Number Comparisons**
   - `.field_less_than(field1, field2)` - field1 < field2
   - `.field_less_or_equal(field1, field2)` - field1 <= field2
   - Works for comparable types (numbers, dates as strings)

5. **Error Reporting**
   - Cross-field errors should reference all involved fields
   - Clear error messages explaining the relationship
   - Appropriate error codes for each pattern

### Non-Functional Requirements

- Cross-field validation runs after field-level validation
- If field validation fails, cross-field may be skipped (configurable)
- Error paths should indicate all involved fields
- Efficient for objects with many cross-field rules

## Acceptance Criteria

- [ ] `.custom(fn)` on ObjectSchema receives validated object
- [ ] Custom validator errors accumulate with field errors
- [ ] `.require_if("type", |v| v == "card", "card_number")` works
- [ ] `.mutually_exclusive("email", "phone")` rejects both present
- [ ] `.at_least_one_of(["email", "phone"])` requires at least one
- [ ] `.equal_fields("password", "confirm")` validates equality
- [ ] `.field_less_than("start", "end")` validates ordering
- [ ] Cross-field errors have descriptive codes
- [ ] Multiple cross-field rules accumulate errors
- [ ] Cross-field validation skips if field validation fails

## Technical Details

### ValidatedObject Type

The `ValidatedObject` type provides safe access to validated field values:

```rust
/// Represents an object that has passed field-level validation.
/// All field values have been validated according to their schemas.
pub struct ValidatedObject {
    fields: HashMap<String, Value>,
}

impl ValidatedObject {
    /// Get a field value by name. Returns None if field doesn't exist.
    pub fn get(&self, field: &str) -> Option<&Value>;

    /// Check if a field exists and is not null.
    pub fn has(&self, field: &str) -> bool {
        self.get(field).is_some_and(|v| !v.is_null())
    }

    /// Get a field as a specific type.
    pub fn get_as<T>(&self, field: &str) -> Option<T>
    where Value: TryInto<T>;
}
```

**Important behavior**:
- Optional fields that weren't provided: `get()` returns `None`
- Optional fields explicitly set to `null`: `get()` returns `Some(Value::Null)`
- Use `has()` to check for non-null presence

### Type Alias for Validators

```rust
type CrossFieldValidator = Box<dyn Fn(&ValidatedObject, &JsonPath) -> Validation<(), SchemaErrors> + 'static>;
```

### Implementation Approach

```rust
impl ObjectSchema {
    pub fn custom<F>(mut self, validator: F) -> Self
    where
        F: Fn(&ValidatedObject, &JsonPath) -> Validation<(), SchemaErrors> + 'static,
    {
        self.cross_field_validators.push(Box::new(validator));
        self
    }

    /// Configure whether to skip cross-field validation if field validation fails.
    /// Default: true (skip cross-field when fields are invalid)
    pub fn skip_cross_field_on_errors(mut self, skip: bool) -> Self {
        self.skip_on_field_errors = skip;
        self
    }

    pub fn require_if<P>(
        mut self,
        condition_field: impl Into<String>,
        predicate: P,
        required_field: impl Into<String>,
    ) -> Self
    where
        P: Fn(&Value) -> bool + 'static,
    {
        let condition_field = condition_field.into();
        let required_field = required_field.into();

        self.custom(move |obj, path| {
            use stillwater::validation::{success, failure};

            let condition_value = obj.get(&condition_field);
            let required_value = obj.get(&required_field);

            match (condition_value, required_value) {
                (Some(cv), None) if predicate(cv) => {
                    failure(SchemaErrors::single(
                        SchemaError::new(
                            path.push_field(&required_field),
                            format!("'{}' is required when '{}' matches condition", required_field, condition_field),
                        )
                        .with_code("conditional_required")
                    ))
                }
                _ => success(()),
            }
        })
    }

    pub fn mutually_exclusive(
        mut self,
        field1: impl Into<String>,
        field2: impl Into<String>,
    ) -> Self {
        let field1 = field1.into();
        let field2 = field2.into();

        self.custom(move |obj, path| {
            use stillwater::validation::{success, failure};

            let has_field1 = obj.get(&field1).is_some_and(|v| !v.is_null());
            let has_field2 = obj.get(&field2).is_some_and(|v| !v.is_null());

            if has_field1 && has_field2 {
                failure(SchemaErrors::single(
                    SchemaError::new(
                        path.clone(),
                        format!("'{}' and '{}' are mutually exclusive", field1, field2),
                    )
                    .with_code("mutually_exclusive")
                ))
            } else {
                success(())
            }
        })
    }

    pub fn at_least_one_of<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let fields: Vec<String> = fields.into_iter().map(Into::into).collect();

        self.custom(move |obj, path| {
            use stillwater::validation::{success, failure};

            let has_any = fields.iter().any(|f| {
                obj.get(f).is_some_and(|v| !v.is_null())
            });

            if has_any {
                success(())
            } else {
                failure(SchemaErrors::single(
                    SchemaError::new(
                        path.clone(),
                        format!("at least one of {:?} is required", fields),
                    )
                    .with_code("at_least_one_required")
                ))
            }
        })
    }

    pub fn equal_fields(
        mut self,
        field1: impl Into<String>,
        field2: impl Into<String>,
    ) -> Self {
        let field1 = field1.into();
        let field2 = field2.into();

        self.custom(move |obj, path| {
            use stillwater::validation::{success, failure};

            let value1 = obj.get(&field1);
            let value2 = obj.get(&field2);

            match (value1, value2) {
                (Some(v1), Some(v2)) if v1 != v2 => {
                    failure(SchemaErrors::single(
                        SchemaError::new(
                            path.push_field(&field2),
                            format!("'{}' must match '{}'", field2, field1),
                        )
                        .with_code("fields_not_equal")
                    ))
                }
                _ => success(()),
            }
        })
    }

    pub fn field_less_than(
        mut self,
        field1: impl Into<String>,
        field2: impl Into<String>,
    ) -> Self {
        let field1 = field1.into();
        let field2 = field2.into();

        self.custom(move |obj, path| {
            use stillwater::validation::{success, failure};

            let value1 = obj.get(&field1);
            let value2 = obj.get(&field2);

            // Compare as numbers if both are numbers, as strings otherwise
            match (value1, value2) {
                (Some(Value::Number(n1)), Some(Value::Number(n2))) => {
                    // Safely convert to f64, skip validation if conversion fails
                    let Some(f1) = n1.as_f64() else { return success(()); };
                    let Some(f2) = n2.as_f64() else { return success(()); };

                    if f1 >= f2 {
                        failure(SchemaErrors::single(
                            SchemaError::new(
                                path.push_field(&field1),
                                format!("'{}' must be less than '{}'", field1, field2),
                            )
                            .with_code("field_not_less_than")
                        ))
                    } else {
                        success(())
                    }
                }
                (Some(Value::String(s1)), Some(Value::String(s2))) => {
                    if s1 >= s2 {
                        failure(SchemaErrors::single(
                            SchemaError::new(
                                path.push_field(&field1),
                                format!("'{}' must be less than '{}'", field1, field2),
                            )
                            .with_code("field_not_less_than")
                        ))
                    } else {
                        success(())
                    }
                }
                // Skip validation if:
                // - Fields don't exist
                // - Fields are null
                // - Type mismatch (e.g., comparing number to string)
                _ => success(()),
            }
        })
    }

    pub fn field_less_or_equal(
        mut self,
        field1: impl Into<String>,
        field2: impl Into<String>,
    ) -> Self {
        let field1 = field1.into();
        let field2 = field2.into();

        self.custom(move |obj, path| {
            use stillwater::validation::{success, failure};

            let value1 = obj.get(&field1);
            let value2 = obj.get(&field2);

            match (value1, value2) {
                (Some(Value::Number(n1)), Some(Value::Number(n2))) => {
                    let Some(f1) = n1.as_f64() else { return success(()); };
                    let Some(f2) = n2.as_f64() else { return success(()); };

                    if f1 > f2 {
                        failure(SchemaErrors::single(
                            SchemaError::new(
                                path.push_field(&field1),
                                format!("'{}' must be less than or equal to '{}'", field1, field2),
                            )
                            .with_code("field_not_less_or_equal")
                        ))
                    } else {
                        success(())
                    }
                }
                (Some(Value::String(s1)), Some(Value::String(s2))) => {
                    if s1 > s2 {
                        failure(SchemaErrors::single(
                            SchemaError::new(
                                path.push_field(&field1),
                                format!("'{}' must be less than or equal to '{}'", field1, field2),
                            )
                            .with_code("field_not_less_or_equal")
                        ))
                    } else {
                        success(())
                    }
                }
                _ => success(()),
            }
        })
    }

    // In validate method, after field validation:
    fn run_cross_field_validation(
        &self,
        validated: &ValidatedObject,
        path: &JsonPath,
    ) -> Vec<SchemaError> {
        if self.skip_on_field_errors && !self.field_errors.is_empty() {
            return vec![];
        }

        let mut errors = Vec::new();
        for validator in &self.cross_field_validators {
            if let Validation::Failure(e) = validator(validated, path) {
                errors.extend(e.into_iter());
            }
        }
        errors
    }

    // Main validate method showing error accumulation
    pub fn validate(&self, value: &Value, path: &JsonPath)
        -> Validation<ValidatedObject, SchemaErrors>
    {
        use stillwater::validation::{success, failure};

        // Step 1: Validate each field according to its schema
        let mut field_errors = Vec::new();
        let mut validated_fields = HashMap::new();

        for (field_name, field_schema) in &self.fields {
            match value.get(field_name) {
                Some(field_value) => {
                    let field_path = path.push_field(field_name);
                    match field_schema.validate(field_value, &field_path) {
                        Validation::Success(v) => {
                            validated_fields.insert(field_name.clone(), v);
                        }
                        Validation::Failure(e) => {
                            field_errors.extend(e.into_iter());
                        }
                    }
                }
                None if self.required_fields.contains(field_name) => {
                    field_errors.push(SchemaError::new(
                        path.push_field(field_name),
                        format!("'{}' is required", field_name),
                    ).with_code("required"));
                }
                None => {
                    // Optional field not provided - skip
                }
            }
        }

        // Step 2: Run cross-field validation (only if configured to do so)
        let validated_obj = ValidatedObject { fields: validated_fields };
        let cross_errors = self.run_cross_field_validation(&validated_obj, path);

        // Step 3: Accumulate all errors
        field_errors.extend(cross_errors);

        // Step 4: Return result
        if field_errors.is_empty() {
            success(validated_obj)
        } else {
            failure(SchemaErrors::from(field_errors))
        }
    }
}
```

### Error Path Strategy

Cross-field validation errors use different path strategies depending on the rule:

- **Object-level rules** (e.g., `mutually_exclusive`, `at_least_one_of`): Attach error to the object path, as the error involves the object as a whole
- **Field-level rules** (e.g., `equal_fields`, `field_less_than`): Attach error to the specific field path that failed the constraint
- **Conditional rules** (e.g., `require_if`): Attach error to the field that is required but missing

This strategy ensures errors appear in the most logical location for developers to fix them.

### Error Codes Reference

| Error Code | Pattern | Description |
|------------|---------|-------------|
| `conditional_required` | `require_if` | Field is required when condition is met |
| `mutually_exclusive` | `mutually_exclusive` | Both fields present when only one allowed |
| `at_least_one_required` | `at_least_one_of` | None of the required fields present |
| `fields_not_equal` | `equal_fields` | Fields don't match when they should |
| `field_not_less_than` | `field_less_than` | First field not less than second |
| `field_not_less_or_equal` | `field_less_or_equal` | First field not less than or equal to second |

### Null vs Missing Field Behavior

Cross-field validators distinguish between missing and null fields:

```rust
// Missing field
{"email": "user@example.com"}  // phone is missing
obj.get("phone") // returns None

// Null field
{"email": "user@example.com", "phone": null}  // phone is explicitly null
obj.get("phone") // returns Some(Value::Null)

// For convenience, use has() to check for non-null presence
obj.has("phone")  // false in both cases above
```

**Helper method behavior**:
- `mutually_exclusive`: Treats `null` as absent (allows both if one is null)
- `at_least_one_of`: Requires at least one non-null value
- `require_if`: Only validates if the required field is missing entirely
- `equal_fields`: Only validates if both fields exist (skips if either missing/null)
- `field_less_than`: Only validates if both fields exist and are comparable types

### Type Mismatch Handling

When comparing fields of different types:

```rust
// Comparing number to string - validation is skipped
{"start": 100, "end": "200"}
field_less_than("start", "end")  // No error - types don't match

// Comparing null to value - validation is skipped
{"start": null, "end": 200}
field_less_than("start", "end")  // No error - start is null
```

**Rationale**: Type mismatches indicate a schema design issue, not a validation error. Field-level schemas should ensure correct types before cross-field validation runs.

### Architecture Changes

- Extend ObjectSchema with:
  - `cross_field_validators: Vec<CrossFieldValidator>`
  - `skip_on_field_errors: bool` (default: `true`)
- Add helper methods for common patterns
- Update validation to run cross-field after field validation

### Data Structures

```rust
type CrossFieldValidator = Box<dyn Fn(&ValidatedObject, &JsonPath) -> Validation<(), SchemaErrors> + 'static>;

pub struct ValidatedObject {
    fields: HashMap<String, Value>,
}
```

### APIs and Interfaces

```rust
// Custom cross-field validation
ObjectSchema::custom<F>(self, validator: F) -> Self

// Configuration
ObjectSchema::skip_cross_field_on_errors(self, skip: bool) -> Self

// Common patterns
ObjectSchema::require_if<P>(self, field: &str, predicate: P, required: &str) -> Self
ObjectSchema::mutually_exclusive(self, field1: &str, field2: &str) -> Self
ObjectSchema::at_least_one_of<I>(self, fields: I) -> Self
ObjectSchema::equal_fields(self, field1: &str, field2: &str) -> Self
ObjectSchema::field_less_than(self, field1: &str, field2: &str) -> Self
ObjectSchema::field_less_or_equal(self, field1: &str, field2: &str) -> Self
```

### Performance Considerations

- **Validator storage**: Each helper method adds one boxed closure to the validator list
- **Execution time**: O(n) where n = number of validators; validators run sequentially
- **Memory**: Validators are stored in a `Vec`, minimal overhead per validator
- **Skip optimization**: When `skip_on_field_errors` is true, cross-field validation is bypassed entirely if any field errors exist, saving computation
- **Recommended limits**: While there's no hard limit, objects with >20 cross-field validators may indicate over-complex validation logic that should be refactored

## Dependencies

- **Prerequisites**: Specs 001, 004
- **Affected Components**: ObjectSchema
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Custom validator execution
  - require_if with various conditions
  - mutually_exclusive combinations
  - at_least_one_of scenarios
  - equal_fields matching/non-matching
  - field comparison (numbers and strings)
  - Cross-field error accumulation
  - skip_cross_field_on_errors configuration

- **Integration Tests**:
  - Multiple cross-field rules on same object
  - Cross-field with nested objects
  - Skip behavior on field errors (enabled/disabled)
  - All cross-field errors reported when multiple rules fail
  - Validator execution order (verify deterministic behavior)

- **Edge Cases**:
  - Missing fields in comparisons
  - Null field values (explicit null vs missing)
  - Type mismatches in comparisons (number vs string)
  - Number conversion failures (values outside f64 range)
  - Empty string comparisons
  - Validators that capture and use external state
  - at_least_one_of with all fields null vs all fields missing
  - mutually_exclusive with one field null, one field present

- **Null Handling Tests**:
  ```rust
  // at_least_one_of: all null should fail
  {"email": null, "phone": null}  // Error: at least one required

  // at_least_one_of: one null, one missing should fail
  {"email": null}  // Error: at least one required

  // mutually_exclusive: both null should pass
  {"email": null, "phone": null}  // OK

  // mutually_exclusive: one null, one present should pass
  {"email": "user@example.com", "phone": null}  // OK
  ```

## Documentation Requirements

- **Code Documentation**:
  - Doc comments for each helper method with examples
  - ValidatedObject API documentation
  - Error code reference in module docs
  - Null vs missing field behavior
  - Type mismatch handling rationale

- **User Documentation**:
  - Guide to cross-field validation patterns
  - When to use custom vs helper methods
  - Best practices for error messages
  - Performance considerations for many validators

- **Architecture Updates**:
  - Validation execution order (field → cross-field)
  - Error accumulation strategy
  - Skip behavior configuration

## Implementation Notes

- **Validation execution order**: Field validation runs first, then cross-field validation
- **Skip behavior**: By default, cross-field validation is skipped if field validation fails (configurable via `skip_cross_field_on_errors`)
- **Validated values**: Cross-field validators receive validated values, not raw input
- **Date comparison**: Works via string comparison (ISO 8601 dates are lexicographically sortable)
- **Error codes**: Must be consistent and machine-readable for client-side handling
- **Performance**: Cross-field validators execute in sequence (O(n) where n = number of validators)
- **Null handling**: Use `obj.has(field)` to check for non-null presence, `obj.get(field)` for raw access
- **Type safety**: Field-level schemas should enforce types; cross-field comparisons skip type mismatches
- **MSRV consideration**: `is_some_and()` requires Rust 1.70+; verify project MSRV compatibility

## Migration and Compatibility

No migration needed - extends object schema.

## Files to Create/Modify

```
src/schema/object.rs (extend)
src/validation/context.rs
tests/cross_field_test.rs
```

## Example Usage

```rust
use postmortem::Schema;

// Date range validation
let date_range = Schema::object()
    .field("start_date", Schema::string().date())
    .field("end_date", Schema::string().date())
    .field_less_than("start_date", "end_date");

// Conditional required
let payment = Schema::object()
    .field("method", Schema::string().one_of(["card", "bank", "cash"]))
    .optional("card_number", Schema::string().pattern(r"^\d{16}$").unwrap())
    .optional("bank_account", Schema::string())
    .require_if("method", |v| v == json!("card"), "card_number")
    .require_if("method", |v| v == json!("bank"), "bank_account");

// Contact info (at least one required)
let contact = Schema::object()
    .optional("email", Schema::string().email())
    .optional("phone", Schema::string().pattern(r"^\+\d{10,15}$").unwrap())
    .at_least_one_of(["email", "phone"]);

// Password confirmation
let registration = Schema::object()
    .field("password", Schema::string().min_len(8))
    .field("confirm_password", Schema::string())
    .equal_fields("password", "confirm_password");

// Custom cross-field validation
let order = Schema::object()
    .field("quantity", Schema::integer().positive())
    .field("unit_price", Schema::integer().non_negative())
    .field("total", Schema::integer().non_negative())
    .custom(|obj, path| {
        use stillwater::validation::{success, failure};

        let qty = obj.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);
        let price = obj.get("unit_price").and_then(|v| v.as_i64()).unwrap_or(0);
        let total = obj.get("total").and_then(|v| v.as_i64()).unwrap_or(0);

        if qty * price != total {
            failure(SchemaErrors::single(
                SchemaError::new(path.push_field("total"), "total must equal quantity * unit_price")
                    .with_code("invalid_total")
            ))
        } else {
            success(())
        }
    });

// Validation
let result = date_range.validate(&json!({
    "start_date": "2024-12-01",
    "end_date": "2024-01-01"  // Before start! Should fail validation
}), &JsonPath::root());
// Error: 'start_date' must be less than 'end_date'
```

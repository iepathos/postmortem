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
   - Returns `Validation<(), SchemaErrors>`
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
            let condition_value = obj.get(&condition_field);
            let required_value = obj.get(&required_field);

            match (condition_value, required_value) {
                (Some(cv), None) if predicate(cv) => {
                    Validation::invalid(SchemaErrors::single(
                        SchemaError::new(
                            path.push_field(&required_field),
                            format!("'{}' is required when '{}' matches condition", required_field, condition_field),
                        )
                        .with_code("conditional_required")
                    ))
                }
                _ => Validation::valid(()),
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
            let has_field1 = obj.get(&field1).is_some_and(|v| !v.is_null());
            let has_field2 = obj.get(&field2).is_some_and(|v| !v.is_null());

            if has_field1 && has_field2 {
                Validation::invalid(SchemaErrors::single(
                    SchemaError::new(
                        path.clone(),
                        format!("'{}' and '{}' are mutually exclusive", field1, field2),
                    )
                    .with_code("mutually_exclusive")
                ))
            } else {
                Validation::valid(())
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
            let has_any = fields.iter().any(|f| {
                obj.get(f).is_some_and(|v| !v.is_null())
            });

            if has_any {
                Validation::valid(())
            } else {
                Validation::invalid(SchemaErrors::single(
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
            let value1 = obj.get(&field1);
            let value2 = obj.get(&field2);

            match (value1, value2) {
                (Some(v1), Some(v2)) if v1 != v2 => {
                    Validation::invalid(SchemaErrors::single(
                        SchemaError::new(
                            path.push_field(&field2),
                            format!("'{}' must match '{}'", field2, field1),
                        )
                        .with_code("fields_not_equal")
                    ))
                }
                _ => Validation::valid(()),
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
            let value1 = obj.get(&field1);
            let value2 = obj.get(&field2);

            // Compare as numbers if both are numbers, as strings otherwise
            match (value1, value2) {
                (Some(Value::Number(n1)), Some(Value::Number(n2))) => {
                    let f1 = n1.as_f64().unwrap_or(f64::NAN);
                    let f2 = n2.as_f64().unwrap_or(f64::NAN);
                    if f1 >= f2 {
                        Validation::invalid(SchemaErrors::single(
                            SchemaError::new(
                                path.push_field(&field1),
                                format!("'{}' must be less than '{}'", field1, field2),
                            )
                            .with_code("field_not_less_than")
                        ))
                    } else {
                        Validation::valid(())
                    }
                }
                (Some(Value::String(s1)), Some(Value::String(s2))) => {
                    if s1 >= s2 {
                        Validation::invalid(SchemaErrors::single(
                            SchemaError::new(
                                path.push_field(&field1),
                                format!("'{}' must be less than '{}'", field1, field2),
                            )
                            .with_code("field_not_less_than")
                        ))
                    } else {
                        Validation::valid(())
                    }
                }
                _ => Validation::valid(()), // Skip if fields don't exist or aren't comparable
            }
        })
    }

    // In validate method, after field validation:
    fn run_cross_field_validation(
        &self,
        validated: &ValidatedObject,
        path: &JsonPath,
        skip_on_field_errors: bool,
        field_errors: &[SchemaError],
    ) -> Vec<SchemaError> {
        if skip_on_field_errors && !field_errors.is_empty() {
            return vec![];
        }

        let mut errors = Vec::new();
        for validator in &self.cross_field_validators {
            if let Validation::Invalid(e) = validator(validated, path) {
                errors.extend(e.into_iter());
            }
        }
        errors
    }
}
```

### Architecture Changes

- Extend ObjectSchema with cross-field validator storage
- Add helper methods for common patterns
- Update validation to run cross-field after field validation

### Data Structures

- Cross-field validator: `Box<dyn Fn(&ValidatedObject, &JsonPath) -> Validation<(), SchemaErrors>>`
- ValidatedObject provides field access

### APIs and Interfaces

```rust
// Custom cross-field validation
ObjectSchema::custom<F>(self, validator: F) -> Self

// Common patterns
ObjectSchema::require_if<P>(self, field: &str, predicate: P, required: &str) -> Self
ObjectSchema::mutually_exclusive(self, field1: &str, field2: &str) -> Self
ObjectSchema::at_least_one_of<I>(self, fields: I) -> Self
ObjectSchema::equal_fields(self, field1: &str, field2: &str) -> Self
ObjectSchema::field_less_than(self, field1: &str, field2: &str) -> Self
ObjectSchema::field_less_or_equal(self, field1: &str, field2: &str) -> Self
```

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

- **Integration Tests**:
  - Multiple cross-field rules
  - Cross-field with nested objects
  - Skip behavior on field errors

- **Edge Cases**:
  - Missing fields in comparisons
  - Null field values
  - Type mismatches in comparisons

## Documentation Requirements

- **Code Documentation**: Examples for each pattern
- **User Documentation**: Guide to cross-field validation
- **Architecture Updates**: Document validation execution order

## Implementation Notes

- Field validation runs first, then cross-field
- Consider whether to skip cross-field on field errors (configurable)
- Cross-field validators receive validated values, not raw input
- Date comparison works via string comparison (ISO 8601 is lexicographically sortable)
- Error codes should be consistent and machine-readable

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
        let qty = obj.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);
        let price = obj.get("unit_price").and_then(|v| v.as_i64()).unwrap_or(0);
        let total = obj.get("total").and_then(|v| v.as_i64()).unwrap_or(0);

        if qty * price != total {
            Validation::invalid(SchemaErrors::single(
                SchemaError::new(path.push_field("total"), "total must equal quantity * unit_price")
                    .with_code("invalid_total")
            ))
        } else {
            Validation::valid(())
        }
    });

// Validation
let result = date_range.validate(&json!({
    "start_date": "2024-12-01",
    "end_date": "2024-01-01"  // Before start!
}), &JsonPath::root());
// Error: 'start_date' must be less than 'end_date'
```

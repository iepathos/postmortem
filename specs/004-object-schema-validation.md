---
number: 4
title: Object Schema Validation
category: foundation
priority: critical
status: draft
dependencies: [1, 2, 3]
created: 2025-11-26
---

# Specification 004: Object Schema Validation

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: Specs 001, 002, 003 (Core Types, String Schema, Integer Schema)

## Context

Object schemas are the backbone of API validation, representing JSON objects with typed fields. This specification enables validation of structured data with required and optional fields, default values, and nested validation with proper path tracking.

The object schema must accumulate errors from all fields, not just stop at the first invalid field. This is critical for providing comprehensive feedback to API consumers.

## Objective

Implement an object schema type that:
1. Defines required and optional fields with their schemas
2. Applies default values for missing optional fields
3. Controls handling of additional/unknown properties
4. Tracks paths through nested objects for error reporting
5. Accumulates all field validation errors

## Requirements

### Functional Requirements

1. **Object Schema Construction**
   - `Schema::object()` creates a new empty object schema
   - Fields are added via builder methods
   - Schema is immutable after construction

2. **Field Definition**
   - `.field(name, schema)` - add a required field
   - `.optional(name, schema)` - add an optional field
   - `.default(name, schema, value)` - optional field with default
   - Field names must be unique

3. **Additional Properties**
   - `.additional_properties(false)` - reject unknown fields
   - `.additional_properties(true)` - allow and ignore unknown fields (default)
   - `.additional_properties(schema)` - validate unknown fields against schema

4. **Nested Validation**
   - Field schemas can be any schema type (string, integer, object, array)
   - Error paths include field names (e.g., `user.email`)
   - All field errors are accumulated

5. **Validation Output**
   - Returns `Validation::Success` with validated object on success
   - Returns `Validation::Failure` with accumulated errors on failure
   - Missing optional fields are omitted or use defaults
   - Unknown fields handled per additional_properties setting
   - Use stillwater's `success()`/`failure()` helper functions

### Non-Functional Requirements

- Path tracking must be accurate for arbitrarily deep nesting
- All field errors must be collected (not just first failure)
- Clear distinction between missing required field and invalid value
- Efficient validation of large objects

## Acceptance Criteria

- [ ] `Schema::object()` creates an object schema
- [ ] `.field("name", string_schema)` adds required string field
- [ ] Missing required field produces error with code `required`
- [ ] `.optional("name", schema)` allows field to be absent
- [ ] `.default("name", schema, value)` uses default when absent
- [ ] Default value is validated against field schema
- [ ] `.additional_properties(false)` rejects unknown fields
- [ ] `.additional_properties(schema)` validates unknown fields
- [ ] Nested object errors include full path (e.g., `address.city`)
- [ ] Multiple field errors are accumulated
- [ ] Non-object values produce type error with code `invalid_type`
- [ ] Error paths are correctly built for all field types
- [ ] Validated object structure is returned on success

## Technical Details

### Implementation Approach

```rust
pub struct ObjectSchema {
    fields: IndexMap<String, FieldDef>,
    additional_properties: AdditionalProperties,
}

struct FieldDef {
    schema: Box<dyn SchemaLike>,
    required: bool,
    default: Option<Value>,
}

enum AdditionalProperties {
    Allow,
    Deny,
    Validate(Box<dyn SchemaLike>),
}

impl Schema {
    pub fn object() -> ObjectSchema {
        ObjectSchema {
            fields: IndexMap::new(),
            additional_properties: AdditionalProperties::Allow,
        }
    }
}

impl ObjectSchema {
    pub fn field(mut self, name: impl Into<String>, schema: impl SchemaLike + 'static) -> Self {
        self.fields.insert(name.into(), FieldDef {
            schema: Box::new(schema),
            required: true,
            default: None,
        });
        self
    }

    pub fn optional(mut self, name: impl Into<String>, schema: impl SchemaLike + 'static) -> Self {
        self.fields.insert(name.into(), FieldDef {
            schema: Box::new(schema),
            required: false,
            default: None,
        });
        self
    }

    pub fn default(
        mut self,
        name: impl Into<String>,
        schema: impl SchemaLike + 'static,
        default: Value,
    ) -> Self {
        self.fields.insert(name.into(), FieldDef {
            schema: Box::new(schema),
            required: false,
            default: Some(default),
        });
        self
    }

    pub fn additional_properties(mut self, setting: impl Into<AdditionalPropertiesSetting>) -> Self {
        self.additional_properties = setting.into();
        self
    }

    pub fn validate(&self, value: &Value, path: &JsonPath) -> Validation<ValidatedObject, SchemaErrors> {
        use stillwater::validation::{success, failure};

        let obj = match value.as_object() {
            Some(o) => o,
            None => return failure(SchemaErrors::single(
                SchemaError::new(path.clone(), "expected object")
                    .with_code("invalid_type")
                    .with_got(value_type_name(value))
                    .with_expected("object")
            )),
        };

        let mut errors = Vec::new();
        let mut validated = Map::new();

        // Check defined fields
        for (name, field_def) in &self.fields {
            let field_path = path.push_field(name);

            match obj.get(name) {
                Some(field_value) => {
                    match field_def.schema.validate(field_value, &field_path) {
                        Validation::Valid(v) => { validated.insert(name.clone(), v); }
                        Validation::Invalid(e) => errors.extend(e.into_iter()),
                    }
                }
                None if field_def.required => {
                    errors.push(SchemaError::new(field_path, format!("required field '{}' is missing", name))
                        .with_code("required")
                        .with_expected("value"));
                }
                None => {
                    if let Some(default) = &field_def.default {
                        validated.insert(name.clone(), default.clone());
                    }
                }
            }
        }

        // Check additional properties
        for (key, value) in obj {
            if !self.fields.contains_key(key) {
                let field_path = path.push_field(key);
                match &self.additional_properties {
                    AdditionalProperties::Allow => { /* ignore */ }
                    AdditionalProperties::Deny => {
                        errors.push(SchemaError::new(field_path, format!("unknown field '{}'", key))
                            .with_code("additional_property"));
                    }
                    AdditionalProperties::Validate(schema) => {
                        match schema.validate(value, &field_path) {
                            Validation::Valid(v) => { validated.insert(key.clone(), v); }
                            Validation::Invalid(e) => errors.extend(e.into_iter()),
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            success(ValidatedObject(validated))
        } else {
            failure(SchemaErrors::from_vec(errors).unwrap())
        }
    }
}
```

### Architecture Changes

- Create `src/schema/object.rs` for object schema
- Define `SchemaLike` trait for schema polymorphism
- Create `ValidatedObject` wrapper type for validated output

### Data Structures

- `ObjectSchema`: Map of field definitions + additional properties setting
- `FieldDef`: Schema, required flag, optional default value
- `AdditionalProperties`: Enum for unknown field handling
- `ValidatedObject`: Type-safe wrapper for validated object

### APIs and Interfaces

```rust
// Construction
Schema::object() -> ObjectSchema

// Fields
ObjectSchema::field(self, name: impl Into<String>, schema: impl SchemaLike) -> Self
ObjectSchema::optional(self, name: impl Into<String>, schema: impl SchemaLike) -> Self
ObjectSchema::default(self, name: impl Into<String>, schema: impl SchemaLike, default: Value) -> Self

// Additional properties
ObjectSchema::additional_properties(self, setting: impl Into<AdditionalPropertiesSetting>) -> Self

// Validation
ObjectSchema::validate(&self, value: &Value, path: &JsonPath) -> Validation<ValidatedObject, SchemaErrors>
```

## Dependencies

- **Prerequisites**: Specs 001, 002, 003 (for basic types to compose)
- **Affected Components**: Schema module
- **External Dependencies**:
  - `indexmap` for ordered field iteration

## Testing Strategy

- **Unit Tests**:
  - Object schema with single required field
  - Object with multiple fields
  - Missing required field error
  - Optional field handling
  - Default value application
  - Additional properties deny
  - Additional properties schema validation
  - Type error for non-objects

- **Integration Tests**:
  - Nested objects (3+ levels)
  - Error path accuracy for nested structures
  - Multiple field errors accumulated

- **Edge Cases**:
  - Empty object schema
  - Empty input object
  - Unicode field names
  - Deeply nested structures

## Documentation Requirements

- **Code Documentation**: Rustdoc with nested object examples
- **User Documentation**: Guide to object validation patterns
- **Architecture Updates**: Document SchemaLike trait

## Implementation Notes

- Use `IndexMap` to preserve field definition order
- Field order affects error message ordering
- Default values should be validated at schema construction time
- Consider lazy default validation if construction-time is problematic
- The `SchemaLike` trait enables schema composition

## Migration and Compatibility

No migration needed - this is new code.

## Files to Create/Modify

```
src/schema/object.rs
src/schema/traits.rs (for SchemaLike)
tests/object_test.rs
tests/nested_test.rs
```

## Example Usage

```rust
use postmortem::Schema;

// User schema
let user_schema = Schema::object()
    .field("id", Schema::integer().positive())
    .field("email", Schema::string().min_len(1))
    .optional("name", Schema::string())
    .default("role", Schema::string(), json!("user"));

// Nested address schema
let address_schema = Schema::object()
    .field("street", Schema::string().min_len(1))
    .field("city", Schema::string().min_len(1))
    .field("zip", Schema::string().pattern(r"^\d{5}$").unwrap());

let full_user = Schema::object()
    .field("user", user_schema)
    .field("address", address_schema)
    .additional_properties(false);

// Validation with nested errors
let result = full_user.validate(&json!({
    "user": { "id": -1, "email": "" },
    "address": { "city": "NYC" }
}), &JsonPath::root());

// Errors:
// - user.id: must be positive
// - user.email: length must be at least 1
// - address.street: required field 'street' is missing
// - address.zip: required field 'zip' is missing
```

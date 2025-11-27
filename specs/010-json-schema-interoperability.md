---
number: 10
title: JSON Schema Interoperability
category: compatibility
priority: medium
status: draft
dependencies: [1, 2, 3, 4, 5, 6, 7, 9]
created: 2025-11-26
---

# Specification 010: JSON Schema Interoperability

**Category**: compatibility
**Priority**: medium
**Status**: draft
**Dependencies**: Specs 001-007, 009 (Core through Registry)

## Context

JSON Schema is the industry standard for describing JSON data structures. Many tools and services use JSON Schema for documentation, code generation, and validation. postmortem should be able to:
1. Export schemas to JSON Schema format for documentation/tooling
2. Import JSON Schema definitions for validation

This enables integration with existing JSON Schema ecosystems while using postmortem's superior error accumulation for validation.

## Objective

Implement bidirectional JSON Schema interoperability:
1. Generate JSON Schema from postmortem schemas
2. Parse JSON Schema into postmortem schemas
3. Support common JSON Schema drafts (draft-07, 2019-09, 2020-12)
4. Round-trip preservation of common features

## Requirements

### Functional Requirements

1. **JSON Schema Generation**
   - `schema.to_json_schema()` generates JSON Schema object
   - Support for all basic types (string, integer, number, boolean, null)
   - Support for object and array schemas
   - Support for constraints (minLength, maximum, pattern, etc.)
   - Support for format annotations
   - Support for combinators (oneOf, anyOf, allOf)
   - Support for $ref from registry schemas

2. **JSON Schema Parsing**
   - `Schema::from_json_schema(value)` parses JSON Schema
   - Handle common draft versions
   - Graceful handling of unsupported features
   - Return warnings for features that can't be represented

3. **Registry Integration**
   - Registry can export all schemas as definitions
   - Registry can import from $defs/$definitions
   - References map to $ref properly

4. **Format Mapping**
   - Map postmortem formats to JSON Schema formats
   - email → "email"
   - url → "uri"
   - uuid → "uuid"
   - date → "date"
   - datetime → "date-time"
   - etc.

### Non-Functional Requirements

- Generated schemas should be valid JSON Schema
- Support validation against JSON Schema meta-schema
- Preserve as much information as possible in round-trips
- Clear documentation of supported/unsupported features

## Acceptance Criteria

- [ ] `schema.to_json_schema()` returns valid JSON Schema
- [ ] String constraints map to minLength, maxLength, pattern
- [ ] Integer constraints map to minimum, maximum
- [ ] Object schemas generate properties and required
- [ ] Array schemas generate items, minItems, maxItems
- [ ] Formats map to format annotation
- [ ] Combinators generate oneOf, anyOf, allOf
- [ ] Registry refs generate $ref
- [ ] `Schema::from_json_schema()` parses basic schemas
- [ ] Unsupported features generate warnings
- [ ] Round-trip preserves common constraints

## Technical Details

### Implementation Approach

```rust
pub trait ToJsonSchema {
    fn to_json_schema(&self) -> serde_json::Value;
}

impl ToJsonSchema for StringSchema {
    fn to_json_schema(&self) -> Value {
        let mut schema = json!({ "type": "string" });

        for constraint in &self.constraints {
            match constraint {
                StringConstraint::MinLength { min, .. } => {
                    schema["minLength"] = json!(min);
                }
                StringConstraint::MaxLength { max, .. } => {
                    schema["maxLength"] = json!(max);
                }
                StringConstraint::Pattern { pattern_str, .. } => {
                    schema["pattern"] = json!(pattern_str);
                }
                StringConstraint::Format { format, .. } => {
                    schema["format"] = json!(format.to_json_schema_format());
                }
                StringConstraint::OneOf { values, .. } => {
                    schema["enum"] = json!(values);
                }
                _ => {}
            }
        }

        schema
    }
}

impl ToJsonSchema for IntegerSchema {
    fn to_json_schema(&self) -> Value {
        let mut schema = json!({ "type": "integer" });

        for constraint in &self.constraints {
            match constraint {
                IntegerConstraint::Min { value, .. } => {
                    schema["minimum"] = json!(value);
                }
                IntegerConstraint::Max { value, .. } => {
                    schema["maximum"] = json!(value);
                }
                IntegerConstraint::Positive { .. } => {
                    schema["exclusiveMinimum"] = json!(0);
                }
                IntegerConstraint::NonNegative { .. } => {
                    schema["minimum"] = json!(0);
                }
                IntegerConstraint::Negative { .. } => {
                    schema["exclusiveMaximum"] = json!(0);
                }
                _ => {}
            }
        }

        schema
    }
}

impl ToJsonSchema for ObjectSchema {
    fn to_json_schema(&self) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for (name, field) in &self.fields {
            properties.insert(name.clone(), field.schema.to_json_schema());
            if field.required {
                required.push(name.clone());
            }
        }

        let mut schema = json!({
            "type": "object",
            "properties": properties,
        });

        if !required.is_empty() {
            schema["required"] = json!(required);
        }

        match &self.additional_properties {
            AdditionalProperties::Deny => {
                schema["additionalProperties"] = json!(false);
            }
            AdditionalProperties::Validate(s) => {
                schema["additionalProperties"] = s.to_json_schema();
            }
            AdditionalProperties::Allow => {}
        }

        schema
    }
}

impl ToJsonSchema for ArraySchema {
    fn to_json_schema(&self) -> Value {
        let mut schema = json!({
            "type": "array",
            "items": self.item_schema.to_json_schema(),
        });

        for constraint in &self.constraints {
            match constraint {
                ArrayConstraint::MinLength { min, .. } => {
                    schema["minItems"] = json!(min);
                }
                ArrayConstraint::MaxLength { max, .. } => {
                    schema["maxItems"] = json!(max);
                }
                ArrayConstraint::Unique { .. } => {
                    schema["uniqueItems"] = json!(true);
                }
                _ => {}
            }
        }

        schema
    }
}

impl ToJsonSchema for CombinatorSchema {
    fn to_json_schema(&self) -> Value {
        match self {
            CombinatorSchema::OneOf { schemas } => {
                json!({
                    "oneOf": schemas.iter().map(|s| s.to_json_schema()).collect::<Vec<_>>()
                })
            }
            CombinatorSchema::AnyOf { schemas } => {
                json!({
                    "anyOf": schemas.iter().map(|s| s.to_json_schema()).collect::<Vec<_>>()
                })
            }
            CombinatorSchema::AllOf { schemas } => {
                json!({
                    "allOf": schemas.iter().map(|s| s.to_json_schema()).collect::<Vec<_>>()
                })
            }
            CombinatorSchema::Optional { inner } => {
                json!({
                    "oneOf": [
                        { "type": "null" },
                        inner.to_json_schema()
                    ]
                })
            }
        }
    }
}

impl ToJsonSchema for RefSchema {
    fn to_json_schema(&self) -> Value {
        json!({
            "$ref": format!("#/$defs/{}", self.name)
        })
    }
}

// Registry export
impl SchemaRegistry {
    pub fn to_json_schema(&self) -> Value {
        let schemas = self.schemas.read();
        let mut defs = serde_json::Map::new();

        for (name, schema) in schemas.iter() {
            defs.insert(name.clone(), schema.to_json_schema());
        }

        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": defs
        })
    }

    pub fn export_schema(&self, name: &str) -> Option<Value> {
        let schema = self.get(name)?;
        let base = self.to_json_schema();

        let mut result = schema.to_json_schema();
        result["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
        result["$defs"] = base["$defs"].clone();

        Some(result)
    }
}

// JSON Schema parsing
#[derive(Debug)]
pub struct ParseWarning {
    pub path: String,
    pub message: String,
}

pub struct ParseResult {
    pub schema: Box<dyn SchemaLike>,
    pub warnings: Vec<ParseWarning>,
}

impl Schema {
    pub fn from_json_schema(value: &Value) -> Result<ParseResult, ParseError> {
        let mut warnings = Vec::new();
        let schema = parse_json_schema(value, &mut warnings)?;
        Ok(ParseResult { schema, warnings })
    }
}

fn parse_json_schema(
    value: &Value,
    warnings: &mut Vec<ParseWarning>,
) -> Result<Box<dyn SchemaLike>, ParseError> {
    // Handle $ref
    if let Some(ref_path) = value.get("$ref").and_then(Value::as_str) {
        let name = ref_path.strip_prefix("#/$defs/")
            .or_else(|| ref_path.strip_prefix("#/definitions/"))
            .ok_or_else(|| ParseError::UnsupportedRef(ref_path.to_string()))?;
        return Ok(Box::new(Schema::ref_(name)));
    }

    // Handle combinators
    if let Some(schemas) = value.get("oneOf").and_then(Value::as_array) {
        let parsed: Result<Vec<_>, _> = schemas.iter()
            .map(|s| parse_json_schema(s, warnings))
            .collect();
        return Ok(Box::new(Schema::one_of(parsed?)));
    }

    // ... similar for anyOf, allOf

    // Handle type-based schemas
    match value.get("type").and_then(Value::as_str) {
        Some("string") => parse_string_schema(value, warnings),
        Some("integer") => parse_integer_schema(value, warnings),
        Some("number") => parse_number_schema(value, warnings),
        Some("boolean") => Ok(Box::new(Schema::boolean())),
        Some("null") => Ok(Box::new(Schema::null())),
        Some("object") => parse_object_schema(value, warnings),
        Some("array") => parse_array_schema(value, warnings),
        _ => Err(ParseError::UnknownType),
    }
}
```

### Architecture Changes

- Create `src/interop/mod.rs` for interoperability
- Create `src/interop/json_schema.rs` for JSON Schema support
- Add `ToJsonSchema` trait to all schema types

### Data Structures

- `ToJsonSchema` trait for generation
- `ParseResult` with schema and warnings
- `ParseWarning` for unsupported features
- `ParseError` for fatal parsing issues

### APIs and Interfaces

```rust
// Trait for JSON Schema generation
trait ToJsonSchema {
    fn to_json_schema(&self) -> serde_json::Value;
}

// Schema methods
impl<S: SchemaLike> S {
    fn to_json_schema(&self) -> Value;
}

// Static parsing
Schema::from_json_schema(value: &Value) -> Result<ParseResult, ParseError>

// Registry methods
SchemaRegistry::to_json_schema(&self) -> Value
SchemaRegistry::export_schema(&self, name: &str) -> Option<Value>
SchemaRegistry::import_json_schema(&self, value: &Value) -> Result<Vec<ParseWarning>, ParseError>
```

## Dependencies

- **Prerequisites**: Specs 001-007, 009
- **Affected Components**: All schema types
- **External Dependencies**:
  - `serde_json` for JSON manipulation

## Testing Strategy

- **Unit Tests**:
  - Generation for each schema type
  - Generation for each constraint type
  - Parsing basic schemas
  - Warning generation for unsupported features

- **Integration Tests**:
  - Complex nested schemas
  - Registry export/import
  - Round-trip tests

- **Validation Tests**:
  - Generated schemas validate against JSON Schema meta-schema
  - Parsed schemas work for validation

## Documentation Requirements

- **Code Documentation**: Examples of import/export
- **User Documentation**: Supported feature matrix
- **Architecture Updates**: Document interop layer

## Implementation Notes

- Use draft 2020-12 as default output format
- Accept multiple drafts for input
- Preserve original schema structure in generation
- Handle $defs and definitions (older name)
- Custom validators cannot be exported (warn user)

## Migration and Compatibility

No migration needed. Feature matrix:

**Fully Supported**:
- type, properties, required, additionalProperties
- items, minItems, maxItems, uniqueItems
- minLength, maxLength, pattern, format
- minimum, maximum, exclusiveMinimum, exclusiveMaximum
- oneOf, anyOf, allOf
- $ref, $defs

**Partially Supported** (warnings):
- enum (only for strings/integers)
- default (import only)
- const

**Not Supported** (warnings):
- if/then/else
- dependentSchemas
- unevaluatedProperties
- Custom keywords

## Files to Create/Modify

```
src/interop/mod.rs
src/interop/json_schema.rs
tests/json_schema_test.rs
```

## Example Usage

```rust
use postmortem::{Schema, SchemaRegistry};

// Create schema
let user = Schema::object()
    .field("id", Schema::integer().positive())
    .field("email", Schema::string().email())
    .optional("name", Schema::string().max_len(100));

// Export to JSON Schema
let json_schema = user.to_json_schema();
println!("{}", serde_json::to_string_pretty(&json_schema)?);
// {
//   "type": "object",
//   "properties": {
//     "id": { "type": "integer", "exclusiveMinimum": 0 },
//     "email": { "type": "string", "format": "email" },
//     "name": { "type": "string", "maxLength": 100 }
//   },
//   "required": ["id", "email"]
// }

// Registry export
let registry = SchemaRegistry::new();
registry.register("User", user).unwrap();
registry.register("UserId", Schema::integer().positive()).unwrap();

let full_schema = registry.export_schema("User").unwrap();
// Includes $defs with all referenced schemas

// Import from JSON Schema
let json_schema_input = json!({
    "type": "object",
    "properties": {
        "count": { "type": "integer", "minimum": 0 }
    },
    "required": ["count"]
});

let ParseResult { schema, warnings } = Schema::from_json_schema(&json_schema_input)?;

for warning in warnings {
    eprintln!("Warning: {} at {}", warning.message, warning.path);
}

// Use imported schema for validation
let result = schema.validate(&json!({ "count": 5 }), &JsonPath::root());
```

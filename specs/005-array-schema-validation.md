---
number: 5
title: Array Schema Validation
category: foundation
priority: critical
status: draft
dependencies: [1, 2, 3, 4]
created: 2025-11-26
---

# Specification 005: Array Schema Validation

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: Specs 001, 002, 003, 004 (Core Types, String, Integer, Object)

## Context

Array validation is essential for handling lists of items in API payloads - user lists, product arrays, batch operations, etc. This specification defines array schema validation with item schemas, length constraints, and uniqueness requirements.

The array schema must validate every item and accumulate all errors with proper index-based paths (e.g., `users[0].email`, `users[2].name`).

## Objective

Implement an array schema type that:
1. Validates each item against an item schema
2. Applies length constraints (min/max)
3. Validates uniqueness requirements
4. Tracks paths through array indices for error reporting
5. Accumulates all item validation errors

## Requirements

### Functional Requirements

1. **Array Schema Construction**
   - `Schema::array(item_schema)` creates array schema with item type
   - Item schema can be any schema type
   - Builder methods for constraints

2. **Length Constraints**
   - `.min_len(n)` - minimum number of items
   - `.max_len(n)` - maximum number of items
   - `.non_empty()` - convenience for min_len(1)

3. **Uniqueness Constraints**
   - `.unique()` - all items must be distinct (by equality)
   - `.unique_by(key_fn)` - items must be unique by extracted key

4. **Item Validation**
   - Each item validated against the item schema
   - Error paths include array index (e.g., `items[0]`)
   - All item errors are accumulated

5. **Validation Output**
   - Returns validated array with typed items
   - Original order preserved

### Non-Functional Requirements

- Path tracking must use bracket notation for indices
- All item errors must be collected (validate every item)
- Efficient uniqueness checking (hash-based)
- Clear error messages for duplicate detection

## Acceptance Criteria

- [ ] `Schema::array(Schema::string())` creates string array schema
- [ ] `.min_len(1)` rejects empty arrays
- [ ] `.max_len(10)` rejects arrays with more than 10 items
- [ ] `.non_empty()` is equivalent to `.min_len(1)`
- [ ] `.unique()` rejects arrays with duplicate items
- [ ] `.unique_by(|item| item.id)` rejects duplicates by key
- [ ] Item validation errors include index path (e.g., `[2]`)
- [ ] Multiple item errors are accumulated
- [ ] Non-array values produce type error with code `invalid_type`
- [ ] Length errors use codes `min_length` and `max_length`
- [ ] Uniqueness errors use code `unique` with duplicate indices
- [ ] Nested object errors have full path (e.g., `users[0].email`)

## Technical Details

### Implementation Approach

```rust
pub struct ArraySchema<S: SchemaLike> {
    item_schema: S,
    constraints: Vec<ArrayConstraint>,
}

enum ArrayConstraint {
    MinLength { min: usize, message: Option<String> },
    MaxLength { max: usize, message: Option<String> },
    Unique { message: Option<String> },
    UniqueBy { key_fn: Box<dyn Fn(&Value) -> Value>, message: Option<String> },
}

impl Schema {
    pub fn array<S: SchemaLike>(item_schema: S) -> ArraySchema<S> {
        ArraySchema {
            item_schema,
            constraints: vec![],
        }
    }
}

impl<S: SchemaLike> ArraySchema<S> {
    pub fn min_len(mut self, min: usize) -> Self {
        self.constraints.push(ArrayConstraint::MinLength { min, message: None });
        self
    }

    pub fn max_len(mut self, max: usize) -> Self {
        self.constraints.push(ArrayConstraint::MaxLength { max, message: None });
        self
    }

    pub fn non_empty(self) -> Self {
        self.min_len(1)
    }

    pub fn unique(mut self) -> Self {
        self.constraints.push(ArrayConstraint::Unique { message: None });
        self
    }

    pub fn unique_by<F>(mut self, key_fn: F) -> Self
    where
        F: Fn(&Value) -> Value + 'static,
    {
        self.constraints.push(ArrayConstraint::UniqueBy {
            key_fn: Box::new(key_fn),
            message: None,
        });
        self
    }

    pub fn validate(&self, value: &Value, path: &JsonPath) -> Validation<Vec<ValidatedValue>, SchemaErrors> {
        let arr = match value.as_array() {
            Some(a) => a,
            None => return Validation::invalid(SchemaErrors::single(
                SchemaError::new(path.clone(), "expected array")
                    .with_code("invalid_type")
                    .with_got(value_type_name(value))
                    .with_expected("array")
            )),
        };

        let mut errors = Vec::new();

        // Check length constraints first
        for constraint in &self.constraints {
            match constraint {
                ArrayConstraint::MinLength { min, message } if arr.len() < *min => {
                    errors.push(SchemaError::new(
                        path.clone(),
                        message.clone().unwrap_or_else(||
                            format!("array must have at least {} items, got {}", min, arr.len())
                        )
                    ).with_code("min_length"));
                }
                ArrayConstraint::MaxLength { max, message } if arr.len() > *max => {
                    errors.push(SchemaError::new(
                        path.clone(),
                        message.clone().unwrap_or_else(||
                            format!("array must have at most {} items, got {}", max, arr.len())
                        )
                    ).with_code("max_length"));
                }
                _ => {}
            }
        }

        // Validate each item
        let mut validated_items = Vec::with_capacity(arr.len());
        for (index, item) in arr.iter().enumerate() {
            let item_path = path.push_index(index);
            match self.item_schema.validate(item, &item_path) {
                Validation::Valid(v) => validated_items.push(v),
                Validation::Invalid(e) => errors.extend(e.into_iter()),
            }
        }

        // Check uniqueness constraints
        for constraint in &self.constraints {
            match constraint {
                ArrayConstraint::Unique { message } => {
                    let duplicates = find_duplicates(arr, |v| v.clone());
                    for (dup_value, indices) in duplicates {
                        errors.push(SchemaError::new(
                            path.clone(),
                            message.clone().unwrap_or_else(||
                                format!("duplicate value at indices {:?}", indices)
                            )
                        ).with_code("unique"));
                    }
                }
                ArrayConstraint::UniqueBy { key_fn, message } => {
                    let duplicates = find_duplicates(arr, key_fn);
                    for (key, indices) in duplicates {
                        errors.push(SchemaError::new(
                            path.clone(),
                            message.clone().unwrap_or_else(||
                                format!("duplicate key at indices {:?}", indices)
                            )
                        ).with_code("unique"));
                    }
                }
                _ => {}
            }
        }

        if errors.is_empty() {
            Validation::valid(validated_items)
        } else {
            Validation::invalid(SchemaErrors::from_vec(errors).unwrap())
        }
    }
}

fn find_duplicates<F>(arr: &[Value], key_fn: F) -> Vec<(Value, Vec<usize>)>
where
    F: Fn(&Value) -> Value,
{
    let mut seen: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, item) in arr.iter().enumerate() {
        let key = key_fn(item);
        let key_str = serde_json::to_string(&key).unwrap();
        seen.entry(key_str).or_default().push(i);
    }
    seen.into_iter()
        .filter(|(_, indices)| indices.len() > 1)
        .map(|(key, indices)| (serde_json::from_str(&key).unwrap(), indices))
        .collect()
}
```

### Architecture Changes

- Create `src/schema/array.rs` for array schema
- Array schema is generic over item schema type

### Data Structures

- `ArraySchema<S>`: Generic over item schema type
- `ArrayConstraint`: Enum of constraint types including uniqueness

### APIs and Interfaces

```rust
// Construction
Schema::array<S: SchemaLike>(item_schema: S) -> ArraySchema<S>

// Constraints
ArraySchema::min_len(self, min: usize) -> Self
ArraySchema::max_len(self, max: usize) -> Self
ArraySchema::non_empty(self) -> Self
ArraySchema::unique(self) -> Self
ArraySchema::unique_by<F>(self, key_fn: F) -> Self

// Validation
ArraySchema::validate(&self, value: &Value, path: &JsonPath) -> Validation<Vec<ValidatedValue>, SchemaErrors>
```

## Dependencies

- **Prerequisites**: Specs 001-004 (for composable schemas)
- **Affected Components**: Schema module
- **External Dependencies**: None beyond existing

## Testing Strategy

- **Unit Tests**:
  - Array of strings
  - Array of integers
  - Array of objects
  - Empty array handling
  - Min length constraint
  - Max length constraint
  - Non-empty constraint
  - Unique constraint
  - Unique-by constraint

- **Integration Tests**:
  - Arrays of objects with nested errors
  - Full path tracking through arrays
  - Multiple items failing validation

- **Edge Cases**:
  - Empty array with various constraints
  - Single item array
  - Large arrays (performance)
  - Null items in array
  - Mixed types in array (should fail item validation)

## Documentation Requirements

- **Code Documentation**: Rustdoc with examples
- **User Documentation**: Common array validation patterns
- **Architecture Updates**: None needed

## Implementation Notes

- Use generic type parameter for item schema to preserve type info
- Uniqueness check uses JSON serialization for key comparison
- Consider streaming validation for very large arrays
- Index paths use bracket notation: `items[0]`, not `items.0`

## Migration and Compatibility

No migration needed - this is new code.

## Files to Create/Modify

```
src/schema/array.rs
tests/array_test.rs
```

## Example Usage

```rust
use postmortem::Schema;

// Simple string array
let tags_schema = Schema::array(Schema::string().min_len(1))
    .non_empty()
    .max_len(10)
    .unique();

// Array of users with unique IDs
let users_schema = Schema::array(
    Schema::object()
        .field("id", Schema::integer().positive())
        .field("email", Schema::string().min_len(1))
)
.unique_by(|user| user.get("id").cloned().unwrap_or(Value::Null));

// Validation
let result = tags_schema.validate(&json!(["rust", "rust", ""]), &JsonPath::root());
// Errors:
// - [2]: length must be at least 1
// - duplicate value at indices [0, 1]

let result = users_schema.validate(&json!([
    { "id": 1, "email": "a@example.com" },
    { "id": 1, "email": "b@example.com" }
]), &JsonPath::root());
// Error: duplicate key at indices [0, 1]
```

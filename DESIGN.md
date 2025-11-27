# postmortem

> Examine your data's cause of death—all of them, not just the first.

A runtime schema validation library that accumulates ALL validation errors, providing complete diagnostics for invalid data.

## Why "postmortem"?

**premortem** validates configuration at startup—predicting how your app will die before it runs.

**postmortem** validates data at runtime—examining what arrived and finding everything wrong with it.

```
premortem                          postmortem
    │                                   │
    ▼                                   ▼
┌─────────────┐                 ┌─────────────────┐
│   Config    │                 │  API Request    │
│   Files     │  ─── startup ─▶ │  User Input     │ ─── runtime ─▶
│   Env Vars  │                 │  External Data  │
└─────────────┘                 └─────────────────┘
    │                                   │
    ▼                                   ▼
 All config errors              All schema errors
 before app runs                before data processed
```

Traditional validation gives you a frustrating loop:

```
POST /users
Error: "email" is invalid

POST /users  (fixed email)
Error: "age" must be positive

POST /users  (fixed age)
Error: "roles" cannot be empty

# Three round trips to find three problems
```

**postmortem** examines the entire payload:

```
POST /users
Validation errors (3):
  [body.email] "not-an-email" is not a valid email address
  [body.age] value -5 must be >= 0
  [body.roles] array cannot be empty

# One response. All errors. Clear paths.
```

## Design Philosophy

Built on stillwater's core beliefs:

### 1. Pure Core, Imperative Shell

Schema definitions are pure data. Schema validation is a pure function. I/O happens at boundaries.

```rust
// PURE: Schema definition (no I/O, no side effects)
let user_schema = Schema::object()
    .field("email", Schema::string().email())
    .field("age", Schema::integer().range(0..=150))
    .field("roles", Schema::array(Schema::string()).min_len(1));

// PURE: Validation (Value in, Validation out)
let result: Validation<ValidatedValue, SchemaErrors> =
    user_schema.validate(&json_value);

// IMPERATIVE SHELL: Loading schemas from files (Effect boundary)
fn load_schema(path: &str) -> Effect<Schema, SchemaError, SchemaEnv> {
    File::read(path)
        .and_then(|content| Schema::parse(&content))
        .context("Loading schema")
}
```

### 2. Error Accumulation (Fail Completely, Not Fast)

Every validation check runs. Every error is collected. The user gets a complete picture.

```rust
// Standard approach: stops at first error ❌
fn validate(input: &Value) -> Result<User, Error> {
    let email = validate_email(input.get("email"))?;  // Stops here
    let age = validate_age(input.get("age"))?;        // Never reached
    let roles = validate_roles(input.get("roles"))?;  // Never reached
    Ok(User { email, age, roles })
}

// postmortem: accumulates ALL errors ✓
let result = user_schema.validate(&input);
// Err(SchemaErrors([
//   { path: "email", message: "invalid email", got: "bad" },
//   { path: "age", message: "must be >= 0", got: -5 },
//   { path: "roles", message: "cannot be empty", got: [] },
// ]))
```

### 3. Composition Over Complexity

Build complex schemas from simple, reusable pieces.

```rust
// Small, focused schemas
let email_schema = Schema::string()
    .email()
    .error("Must be a valid email address");

let age_schema = Schema::integer()
    .range(0..=150)
    .error("Age must be between 0 and 150");

let role_schema = Schema::string()
    .one_of(["admin", "user", "guest"])
    .error("Must be a valid role");

// Compose into larger schemas
let user_schema = Schema::object()
    .field("email", email_schema)
    .field("age", age_schema)
    .field("roles", Schema::array(role_schema).min_len(1));

// Extend for specific use cases
let admin_user_schema = user_schema.clone()
    .field("admin_level", Schema::integer().range(1..=5))
    .field("permissions", Schema::array(permission_schema));
```

### 4. Types Guide, Don't Restrict

Clear, simple types. No macro magic. Predictable behavior.

```rust
// Core types are straightforward
Schema              // Schema definition
SchemaError         // Single validation error with path and context
SchemaErrors        // Collection of errors (NonEmptyVec<SchemaError>)
ValidatedValue      // Validated data wrapper

// Validation result uses stillwater's Validation type
Validation<ValidatedValue, SchemaErrors>
```

### 5. Pragmatism Over Purity

Works with real Rust patterns. Integrates with serde. No fighting the ecosystem.

```rust
// Works with serde_json::Value
let result = schema.validate(&json_value);

// Works with any Deserialize type
let user: User = schema.validate_and_deserialize(&json_value)?;

// Generates OpenAPI/JSON Schema
let openapi = schema.to_openapi();
let json_schema = schema.to_json_schema();
```

---

## Core Types

### Schema

The fundamental building block—a description of valid data.

```rust
pub struct Schema {
    kind: SchemaKind,
    validators: Vec<Box<dyn Validator>>,
    error_message: Option<String>,
    metadata: SchemaMetadata,
}

pub enum SchemaKind {
    // Primitives
    String(StringConstraints),
    Integer(IntegerConstraints),
    Float(FloatConstraints),
    Boolean,
    Null,

    // Compounds
    Object(ObjectSchema),
    Array(ArraySchema),

    // Combinators
    OneOf(Vec<Schema>),      // Exactly one must match
    AnyOf(Vec<Schema>),      // At least one must match
    AllOf(Vec<Schema>),      // All must match
    Optional(Box<Schema>),   // Nullable/missing allowed

    // References
    Ref(String),             // Reference to named schema
}
```

### SchemaError

A single validation error with full context.

```rust
pub struct SchemaError {
    /// JSON path to the error (e.g., "body.users[0].email")
    pub path: JsonPath,

    /// Human-readable error message
    pub message: String,

    /// The actual value that failed validation
    pub got: Option<Value>,

    /// Expected type or constraint
    pub expected: Option<String>,

    /// Error code for programmatic handling
    pub code: ErrorCode,
}

impl Display for SchemaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.path, self.message)?;
        if let Some(got) = &self.got {
            write!(f, " (got: {})", got)?;
        }
        Ok(())
    }
}
```

### SchemaErrors

A non-empty collection of errors (uses stillwater's NonEmptyVec).

```rust
pub struct SchemaErrors(NonEmptyVec<SchemaError>);

impl SchemaErrors {
    /// Get all errors for a specific path
    pub fn for_path(&self, path: &str) -> Vec<&SchemaError>;

    /// Group errors by path
    pub fn by_path(&self) -> HashMap<JsonPath, Vec<&SchemaError>>;

    /// Convert to API-friendly format
    pub fn to_api_response(&self) -> ApiErrorResponse;
}
```

### ValidatedValue

A wrapper indicating data has passed validation.

```rust
pub struct ValidatedValue {
    value: Value,
    schema: Schema,
}

impl ValidatedValue {
    /// Deserialize to a concrete type (infallible after validation)
    pub fn deserialize<T: DeserializeOwned>(&self) -> T;

    /// Get the underlying value
    pub fn into_value(self) -> Value;

    /// Get a reference to the value
    pub fn value(&self) -> &Value;
}
```

---

## Schema Builder API

### String Schema

```rust
Schema::string()
    // Built-in formats
    .email()                    // RFC 5322 email
    .url()                      // Valid URL
    .uuid()                     // UUID v4
    .date()                     // ISO 8601 date
    .datetime()                 // ISO 8601 datetime
    .ip()                       // IPv4 or IPv6
    .ipv4()                     // IPv4 only
    .ipv6()                     // IPv6 only

    // Length constraints
    .min_len(1)                 // Minimum length
    .max_len(100)               // Maximum length
    .len(10)                    // Exact length

    // Pattern matching
    .pattern(r"^\d{3}-\d{4}$")  // Regex pattern
    .starts_with("prefix_")
    .ends_with("_suffix")
    .contains("substring")

    // Enumeration
    .one_of(["active", "inactive", "pending"])

    // Transform before validation
    .trim()                     // Trim whitespace
    .lowercase()                // Convert to lowercase

    // Custom validation
    .custom(|s| {
        if s.chars().all(|c| c.is_alphanumeric()) {
            Ok(())
        } else {
            Err("must be alphanumeric")
        }
    })

    // Error customization
    .error("Must be a valid email address")
```

### Integer Schema

```rust
Schema::integer()
    // Range constraints
    .range(1..=100)             // Inclusive range
    .min(1)                     // Minimum value
    .max(100)                   // Maximum value
    .positive()                 // > 0
    .non_negative()             // >= 0
    .negative()                 // < 0

    // Divisibility
    .multiple_of(5)             // Must be divisible by 5

    // Enumeration
    .one_of([1, 2, 3, 5, 8, 13])

    // Custom
    .custom(|n| if n % 2 == 0 { Ok(()) } else { Err("must be even") })

    .error("Age must be between 1 and 100")
```

### Float Schema

```rust
Schema::float()
    .range(0.0..=1.0)
    .positive()
    .finite()                   // No NaN or Infinity

    .error("Must be a probability between 0 and 1")
```

### Boolean Schema

```rust
Schema::boolean()
    .must_be(true)              // Must be exactly true

    .error("Must accept terms of service")
```

### Array Schema

```rust
Schema::array(item_schema)
    // Length constraints
    .min_len(1)                 // At least one item
    .max_len(100)               // At most 100 items
    .len(5)                     // Exactly 5 items
    .non_empty()                // Shorthand for min_len(1)

    // Item constraints
    .unique()                   // All items must be unique
    .unique_by(|item| item.get("id"))  // Unique by key

    // Custom
    .custom(|arr| {
        if arr.iter().filter(|x| x.is_premium()).count() <= 3 {
            Ok(())
        } else {
            Err("maximum 3 premium items allowed")
        }
    })

    .error("Must have between 1 and 100 items")
```

### Object Schema

```rust
Schema::object()
    // Required fields
    .field("email", Schema::string().email())
    .field("age", Schema::integer().positive())

    // Optional fields
    .field("nickname", Schema::string().optional())

    // Default values
    .field("status", Schema::string().default("pending"))

    // Nested objects
    .field("address", Schema::object()
        .field("street", Schema::string())
        .field("city", Schema::string())
        .field("zip", Schema::string().pattern(r"^\d{5}$")))

    // Additional properties
    .additional_properties(false)           // No extra fields allowed
    .additional_properties(Schema::string()) // Extra fields must be strings

    // Cross-field validation
    .custom(|obj| {
        let start = obj.get("start_date").as_date()?;
        let end = obj.get("end_date").as_date()?;
        if start <= end {
            Ok(())
        } else {
            Err("start_date must be before end_date")
        }
    })

    .error("Invalid user object")
```

### Combinators

```rust
// One of several schemas (discriminated union)
Schema::one_of([
    Schema::object()
        .field("type", Schema::string().must_be("email"))
        .field("address", Schema::string().email()),
    Schema::object()
        .field("type", Schema::string().must_be("phone"))
        .field("number", Schema::string().pattern(r"^\+?\d{10,14}$")),
])

// Any of several schemas
Schema::any_of([
    Schema::string(),
    Schema::integer(),
])

// All constraints must match
Schema::all_of([
    Schema::object().field("id", Schema::integer()),
    Schema::object().field("name", Schema::string()),
])

// Optional (nullable)
Schema::string().optional()  // null or valid string
```

---

## Cross-Field Validation

The `custom` validator on objects enables cross-field validation with error accumulation:

```rust
let order_schema = Schema::object()
    .field("items", Schema::array(item_schema).non_empty())
    .field("discount_code", Schema::string().optional())
    .field("shipping", shipping_schema)
    .custom(|obj| {
        // Multiple cross-field validations, all accumulated
        Validation::all((
            validate_items_total(obj),
            validate_discount_applicability(obj),
            validate_shipping_for_items(obj),
        ))
    });

fn validate_items_total(obj: &Value) -> Validation<(), String> {
    let total: f64 = obj["items"].as_array()
        .map(|items| items.iter()
            .filter_map(|i| i["price"].as_f64())
            .sum())
        .unwrap_or(0.0);

    if total >= 10.0 {
        Validation::success(())
    } else {
        Validation::failure("Order total must be at least $10.00".into())
    }
}

fn validate_discount_applicability(obj: &Value) -> Validation<(), String> {
    if let Some(code) = obj["discount_code"].as_str() {
        let has_eligible_items = obj["items"].as_array()
            .map(|items| items.iter().any(|i| i["discountable"].as_bool() == Some(true)))
            .unwrap_or(false);

        if has_eligible_items {
            Validation::success(())
        } else {
            Validation::failure(format!("Discount code '{}' requires eligible items", code))
        }
    } else {
        Validation::success(())
    }
}
```

---

## Error Path Tracking

Errors include the full JSON path to the problematic value:

```rust
let data = json!({
    "users": [
        { "name": "Alice", "email": "valid@example.com" },
        { "name": "", "email": "invalid" },  // Two errors here
    ]
});

let result = schema.validate(&data);

// Errors:
// [users[1].name] cannot be empty
// [users[1].email] "invalid" is not a valid email address
```

### JsonPath Type

```rust
pub struct JsonPath(Vec<PathSegment>);

pub enum PathSegment {
    Field(String),      // .field_name
    Index(usize),       // [0]
}

impl JsonPath {
    pub fn root() -> Self;
    pub fn field(&self, name: &str) -> Self;
    pub fn index(&self, idx: usize) -> Self;
}

impl Display for JsonPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for segment in &self.0 {
            match segment {
                PathSegment::Field(name) => write!(f, ".{}", name)?,
                PathSegment::Index(idx) => write!(f, "[{}]", idx)?,
            }
        }
        Ok(())
    }
}
```

---

## Integration with stillwater

### Using Validation

postmortem returns `stillwater::Validation` for error accumulation:

```rust
use postmortem::Schema;
use stillwater::Validation;

let schema = Schema::object()
    .field("email", Schema::string().email())
    .field("age", Schema::integer().positive());

// Returns Validation<ValidatedValue, SchemaErrors>
let result = schema.validate(&json_value);

match result {
    Validation::Success(validated) => {
        let user: User = validated.deserialize();
        // ...
    }
    Validation::Failure(errors) => {
        // errors is SchemaErrors (NonEmptyVec<SchemaError>)
        for error in errors.iter() {
            eprintln!("{}", error);
        }
    }
}
```

### Using Effect

Schema loading and validation can be wrapped in Effects:

```rust
use postmortem::{Schema, SchemaRegistry};
use stillwater::Effect;

// Load schema from file (Effect boundary)
fn load_schema(name: &str) -> Effect<Schema, SchemaError, SchemaEnv> {
    Effect::asks(|env: &SchemaEnv| env.schema_dir.clone())
        .and_then(move |dir| {
            let path = format!("{}/{}.json", dir, name);
            File::read(&path)
                .and_then(|content| Schema::parse(&content))
        })
        .context(format!("Loading schema '{}'", name))
}

// Validate request body (Effect + Validation)
fn validate_request<T: DeserializeOwned>(
    schema_name: &str,
    body: Value,
) -> Effect<T, ApiError, AppEnv> {
    Effect::asks(|env: &AppEnv| env.schemas.get(schema_name).clone())
        .and_then(move |schema| {
            match schema.validate(&body) {
                Validation::Success(validated) => {
                    Effect::pure(validated.deserialize())
                }
                Validation::Failure(errors) => {
                    Effect::fail(ApiError::ValidationFailed(errors))
                }
            }
        })
        .context("Validating request body")
}
```

---

## Schema Registry

For applications with many schemas, use a registry:

```rust
pub struct SchemaRegistry {
    schemas: HashMap<String, Schema>,
}

impl SchemaRegistry {
    pub fn new() -> Self;

    /// Register a schema by name
    pub fn register(&mut self, name: &str, schema: Schema) -> &mut Self;

    /// Get a schema by name
    pub fn get(&self, name: &str) -> Option<&Schema>;

    /// Validate all schema references resolve
    pub fn validate_refs(&self) -> Validation<(), Vec<String>>;

    /// Load schemas from a directory
    pub fn load_dir(path: &Path) -> Effect<Self, SchemaError, FsEnv>;
}

// Usage
let mut registry = SchemaRegistry::new();
registry
    .register("email", Schema::string().email())
    .register("user", Schema::object()
        .field("email", Schema::ref_("email"))
        .field("age", Schema::integer().positive()))
    .register("admin", Schema::all_of([
        Schema::ref_("user"),
        Schema::object().field("permissions", Schema::array(Schema::string())),
    ]));

// Validate all refs resolve
registry.validate_refs()?;

// Use
let user_schema = registry.get("user").unwrap();
```

---

## OpenAPI / JSON Schema Generation

Generate standard schema formats for documentation and interoperability:

```rust
// Generate JSON Schema
let json_schema: serde_json::Value = schema.to_json_schema();
// {
//   "type": "object",
//   "properties": {
//     "email": { "type": "string", "format": "email" },
//     "age": { "type": "integer", "minimum": 0 }
//   },
//   "required": ["email", "age"]
// }

// Generate OpenAPI 3.0 schema
let openapi_schema: serde_json::Value = schema.to_openapi();

// Round-trip: parse JSON Schema into postmortem Schema
let schema = Schema::from_json_schema(&json_schema)?;
```

---

## API Error Response Format

Standard format for API responses:

```rust
#[derive(Serialize)]
pub struct ApiValidationError {
    pub error: String,
    pub code: String,
    pub details: Vec<ApiFieldError>,
}

#[derive(Serialize)]
pub struct ApiFieldError {
    pub path: String,
    pub message: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub got: Option<Value>,
}

impl From<SchemaErrors> for ApiValidationError {
    fn from(errors: SchemaErrors) -> Self {
        ApiValidationError {
            error: "Validation failed",
            code: "VALIDATION_ERROR",
            details: errors.iter().map(|e| ApiFieldError {
                path: e.path.to_string(),
                message: e.message.clone(),
                code: e.code.to_string(),
                got: e.got.clone(),
            }).collect(),
        }
    }
}
```

Response format:

```json
{
  "error": "Validation failed",
  "code": "VALIDATION_ERROR",
  "details": [
    {
      "path": "body.email",
      "message": "\"not-an-email\" is not a valid email address",
      "code": "INVALID_EMAIL",
      "got": "not-an-email"
    },
    {
      "path": "body.age",
      "message": "value must be >= 0",
      "code": "OUT_OF_RANGE",
      "got": -5
    }
  ]
}
```

---

## Testing

### Unit Testing Schemas

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use postmortem::test_helpers::*;

    #[test]
    fn valid_user_passes() {
        let schema = user_schema();
        let data = json!({
            "email": "test@example.com",
            "age": 25,
        });

        assert_valid!(schema.validate(&data));
    }

    #[test]
    fn invalid_email_fails() {
        let schema = user_schema();
        let data = json!({
            "email": "not-an-email",
            "age": 25,
        });

        assert_invalid!(schema.validate(&data), errors => {
            assert_eq!(errors.len(), 1);
            assert_error_at!(errors, "email", "INVALID_EMAIL");
        });
    }

    #[test]
    fn multiple_errors_accumulated() {
        let schema = user_schema();
        let data = json!({
            "email": "bad",
            "age": -5,
        });

        assert_invalid!(schema.validate(&data), errors => {
            assert_eq!(errors.len(), 2);
            assert_error_at!(errors, "email");
            assert_error_at!(errors, "age");
        });
    }
}
```

### Property-Based Testing

```rust
#[cfg(test)]
mod proptests {
    use proptest::prelude::*;
    use postmortem::generators::*;

    proptest! {
        #[test]
        fn valid_emails_pass(email in valid_email()) {
            let schema = Schema::string().email();
            let result = schema.validate(&json!(email));
            prop_assert!(result.is_success());
        }

        #[test]
        fn invalid_emails_fail(email in invalid_email()) {
            let schema = Schema::string().email();
            let result = schema.validate(&json!(email));
            prop_assert!(result.is_failure());
        }
    }
}
```

---

## Comparison with Existing Solutions

| Feature | postmortem | validator | garde | jsonschema |
|---------|------------|-----------|-------|------------|
| Error accumulation | ✅ All errors | ❌ First only | ❌ First only | ✅ All errors |
| Works on `Value` | ✅ Yes | ❌ Structs only | ❌ Structs only | ✅ Yes |
| Custom error messages | ✅ Per-field | ✅ Per-field | ✅ Per-field | ❌ Generic |
| Cross-field validation | ✅ With stillwater | ⚠️ Limited | ⚠️ Limited | ❌ No |
| Schema composition | ✅ Combinators | ❌ No | ❌ No | ❌ No |
| OpenAPI generation | ✅ Yes | ❌ No | ❌ No | ✅ JSON Schema |
| Error paths | ✅ Full JSON path | ⚠️ Field name | ⚠️ Field name | ✅ JSON path |
| Runtime schema loading | ✅ Effect-based | ❌ No | ❌ No | ✅ Yes |
| stillwater integration | ✅ Native | ❌ No | ❌ No | ❌ No |

---

## Implementation Roadmap

### Phase 1: Core (MVP)

- [ ] Basic schema types (string, integer, float, boolean, null)
- [ ] Array and object schemas
- [ ] Error accumulation with paths
- [ ] String constraints (min/max len, pattern, formats)
- [ ] Numeric constraints (range, positive, etc.)
- [ ] Object field definitions (required, optional, default)
- [ ] Array constraints (min/max len, unique)
- [ ] stillwater Validation integration

### Phase 2: Composition

- [ ] Schema combinators (one_of, any_of, all_of)
- [ ] Schema references ($ref)
- [ ] SchemaRegistry
- [ ] Custom validators
- [ ] Cross-field validation

### Phase 3: Interoperability

- [ ] JSON Schema generation
- [ ] OpenAPI schema generation
- [ ] JSON Schema parsing (import)
- [ ] API error response formatting

### Phase 4: Developer Experience

- [ ] Test helpers and macros
- [ ] Property-based test generators
- [ ] Schema derive macro (optional)
- [ ] Documentation and examples

### Phase 5: Advanced

- [ ] Async validation (for validators that need I/O)
- [ ] Schema caching/compilation
- [ ] Conditional schemas (if/then/else)
- [ ] Schema versioning/migration

---

## Module Structure

```
postmortem/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── schema/
│   │   ├── mod.rs          # Schema type and builder
│   │   ├── string.rs       # String constraints
│   │   ├── numeric.rs      # Integer/float constraints
│   │   ├── array.rs        # Array constraints
│   │   ├── object.rs       # Object schema
│   │   └── combinators.rs  # one_of, any_of, all_of
│   ├── validation/
│   │   ├── mod.rs          # Validation logic
│   │   ├── validator.rs    # Validator trait
│   │   ├── builtin.rs      # Built-in validators
│   │   └── path.rs         # JsonPath implementation
│   ├── error/
│   │   ├── mod.rs          # Error types
│   │   ├── schema_error.rs # SchemaError
│   │   └── api.rs          # API response formatting
│   ├── registry.rs         # SchemaRegistry
│   ├── interop/
│   │   ├── mod.rs
│   │   ├── json_schema.rs  # JSON Schema conversion
│   │   └── openapi.rs      # OpenAPI conversion
│   └── test_helpers.rs     # Testing utilities
├── tests/
│   ├── integration.rs
│   └── property.rs
└── examples/
    ├── basic_validation.rs
    ├── cross_field.rs
    ├── api_validation.rs
    └── schema_registry.rs
```

---

## License

MIT © Glen Baker <iepathos@gmail.com>

---

*"A postmortem examines everything that went wrong—not just the first thing it finds."*

---
number: 14
title: Effect Integration
category: foundation
priority: low
status: draft
dependencies: [1, 9]
created: 2025-11-26
---

# Specification 014: Effect Integration

**Category**: foundation
**Priority**: low
**Status**: draft
**Dependencies**: Specs 001, 009 (Core Types, Registry)

## Context

stillwater's Effect type enables composable side effects with dependency injection. This specification extends postmortem to support:
1. Loading schemas from files via Effect
2. Async validators for I/O-dependent validation (DB lookups, API calls)
3. Environment-based schema configuration

This is considered lower priority as most validation use cases don't require async I/O, but it enables advanced scenarios like uniqueness checks against a database.

## Objective

Implement Effect integration for:
1. Schema loading from filesystem via Effect
2. Async validation for I/O-bound validators
3. Environment injection for schema configuration
4. Composable async validation pipelines

## Requirements

### Functional Requirements

1. **Effect-Based Schema Loading**
   - `SchemaRegistry::load_dir(path)` returns `Effect<SchemaEnv, SchemaLoadError, ()>`
   - Load JSON Schema files from directory
   - Parse and register each schema
   - Accumulate loading errors

2. **Async Validators**
   - `AsyncValidator` trait for async validation
   - `.async_custom(validator)` on schemas
   - Returns `Effect<Env, E, Validation<(), SchemaErrors>>`
   - Compose with sync validators

3. **Environment Integration**
   - `SchemaEnv` for schema-specific configuration
   - File system abstraction
   - Configurable via environment

4. **Async Validation Flow**
   - Run sync validators first
   - Run async validators if sync passes (optional)
   - Accumulate errors from all validators
   - Support parallel async validation

### Non-Functional Requirements

- Async validation is optional (feature-gated)
- Minimal overhead when not using async features
- Composable with other Effect-based code
- Clear documentation of async patterns

## Acceptance Criteria

- [ ] `SchemaRegistry::load_dir()` returns Effect
- [ ] Loading handles missing files gracefully
- [ ] Loading accumulates parse errors
- [ ] `AsyncValidator` trait defined
- [ ] `.async_custom()` accepts async validators
- [ ] Async validators receive environment
- [ ] Sync and async errors accumulate
- [ ] Optional parallel async validation
- [ ] Works with stillwater's Effect composition

## Technical Details

### Implementation Approach

```rust
use stillwater::{Effect, Env};

// Environment for schema operations
pub trait SchemaEnv: Env {
    type Fs: FileSystem;

    fn filesystem(&self) -> &Self::Fs;
}

pub trait FileSystem {
    fn read_file(&self, path: &Path) -> Effect<Self, IoError, String>;
    fn read_dir(&self, path: &Path) -> Effect<Self, IoError, Vec<PathBuf>>;
}

// Schema loading
impl SchemaRegistry {
    /// Load all JSON Schema files from a directory
    pub fn load_dir<E: SchemaEnv>(
        &self,
        path: impl AsRef<Path>,
    ) -> Effect<E, SchemaLoadError, ()> {
        Effect::from_fn(move |env: &E| {
            let fs = env.filesystem();
            let path = path.as_ref();

            // Read directory contents
            let files = fs.read_dir(path).run(env)?;

            let mut errors = Vec::new();

            for file in files {
                if file.extension() == Some("json".as_ref()) {
                    match self.load_schema_file(file, env) {
                        Ok(()) => {}
                        Err(e) => errors.push(e),
                    }
                }
            }

            if errors.is_empty() {
                Ok(())
            } else {
                Err(SchemaLoadError::Multiple(errors))
            }
        })
    }

    fn load_schema_file<E: SchemaEnv>(
        &self,
        path: PathBuf,
        env: &E,
    ) -> Result<(), SchemaLoadError> {
        let content = env.filesystem()
            .read_file(&path)
            .run(env)
            .map_err(|e| SchemaLoadError::Io(path.clone(), e))?;

        let json: Value = serde_json::from_str(&content)
            .map_err(|e| SchemaLoadError::Parse(path.clone(), e))?;

        let name = path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| SchemaLoadError::InvalidFileName(path.clone()))?;

        let ParseResult { schema, warnings } = Schema::from_json_schema(&json)
            .map_err(|e| SchemaLoadError::Schema(path.clone(), e))?;

        for warning in warnings {
            // Log warnings
            eprintln!("Warning loading {}: {}", path.display(), warning.message);
        }

        self.register(name, schema)
            .map_err(|e| SchemaLoadError::Registry(e))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SchemaLoadError {
    #[error("IO error reading {0}: {1}")]
    Io(PathBuf, IoError),

    #[error("Parse error in {0}: {1}")]
    Parse(PathBuf, serde_json::Error),

    #[error("Schema error in {0}: {1}")]
    Schema(PathBuf, ParseError),

    #[error("Invalid filename: {0}")]
    InvalidFileName(PathBuf),

    #[error("Registry error: {0}")]
    Registry(RegistryError),

    #[error("Multiple errors: {0:?}")]
    Multiple(Vec<SchemaLoadError>),
}

// Async validator trait
pub trait AsyncValidator<E: Env>: Send + Sync {
    fn validate_async(
        &self,
        value: &Value,
        path: &JsonPath,
    ) -> Effect<E, Never, Validation<(), SchemaErrors>>;
}

// Async custom validator support
impl StringSchema {
    pub fn async_custom<E, V>(mut self, validator: V) -> AsyncStringSchema<E>
    where
        E: Env,
        V: AsyncValidator<E> + 'static,
    {
        AsyncStringSchema {
            sync_schema: self,
            async_validators: vec![Box::new(validator)],
        }
    }
}

pub struct AsyncStringSchema<E: Env> {
    sync_schema: StringSchema,
    async_validators: Vec<Box<dyn AsyncValidator<E>>>,
}

impl<E: Env> AsyncStringSchema<E> {
    pub fn validate_async(
        &self,
        value: &Value,
        path: &JsonPath,
    ) -> Effect<E, Never, Validation<String, SchemaErrors>> {
        Effect::from_fn(move |env: &E| {
            use stillwater::validation::{success, failure};

            // Run sync validation first
            let sync_result = self.sync_schema.validate(value, path);

            match sync_result {
                Validation::Failure(errors) => {
                    // If sync fails, return those errors
                    Ok(failure(errors))
                }
                Validation::Success(validated) => {
                    // Run async validators
                    let mut all_errors = Vec::new();

                    for validator in &self.async_validators {
                        let result = validator.validate_async(value, path).run(env)?;
                        if let Validation::Failure(errors) = result {
                            all_errors.extend(errors.into_iter());
                        }
                    }

                    if all_errors.is_empty() {
                        Ok(success(validated))
                    } else {
                        Ok(failure(SchemaErrors::from_vec(all_errors).unwrap()))
                    }
                }
            }
        })
    }
}

// Example: Database uniqueness validator
pub struct UniqueEmailValidator<Db> {
    db: Db,
}

impl<E, Db> AsyncValidator<E> for UniqueEmailValidator<Db>
where
    E: Env,
    Db: EmailLookup,
{
    fn validate_async(
        &self,
        value: &Value,
        path: &JsonPath,
    ) -> Effect<E, Never, Validation<(), SchemaErrors>> {
        Effect::from_fn(move |_env: &E| {
            use stillwater::validation::{success, failure};

            let email = value.as_str().unwrap_or("");

            // This would be an async DB lookup in practice
            let exists = self.db.email_exists(email);

            if exists {
                Ok(failure(SchemaErrors::single(
                    SchemaError::new(path.clone(), "email already exists")
                        .with_code("unique_email")
                )))
            } else {
                Ok(success(()))
            }
        })
    }
}
```

### Architecture Changes

- Create `src/effect/mod.rs` for Effect integration
- Create `src/effect/loading.rs` for schema loading
- Create `src/effect/async_validator.rs` for async validation
- Add async schema wrapper types

### Data Structures

- `SchemaEnv` trait for environment requirements
- `FileSystem` trait for file abstraction
- `AsyncValidator<E>` trait for async validators
- `AsyncStringSchema<E>`, etc. for async schema wrappers
- `SchemaLoadError` for loading errors

### APIs and Interfaces

```rust
// Environment traits
trait SchemaEnv: Env {
    type Fs: FileSystem;
    fn filesystem(&self) -> &Self::Fs;
}

trait FileSystem {
    fn read_file(&self, path: &Path) -> Effect<Self, IoError, String>;
    fn read_dir(&self, path: &Path) -> Effect<Self, IoError, Vec<PathBuf>>;
}

// Schema loading
SchemaRegistry::load_dir<E: SchemaEnv>(&self, path: &Path) -> Effect<E, SchemaLoadError, ()>

// Async validator trait
trait AsyncValidator<E: Env>: Send + Sync {
    fn validate_async(&self, value: &Value, path: &JsonPath) -> Effect<E, Never, Validation<(), SchemaErrors>>;
}

// Async schema methods
StringSchema::async_custom<E, V>(self, validator: V) -> AsyncStringSchema<E>
AsyncStringSchema::validate_async(&self, value: &Value, path: &JsonPath) -> Effect<E, Never, Validation<String, SchemaErrors>>
```

## Dependencies

- **Prerequisites**: Specs 001, 009
- **Affected Components**: Schema types (async wrappers)
- **External Dependencies**:
  - `stillwater` for Effect type

## Testing Strategy

- **Unit Tests**:
  - Schema loading from test directory
  - Async validator execution
  - Error accumulation

- **Integration Tests**:
  - Full Effect composition
  - Multiple async validators
  - Mixed sync/async validation

- **Mocking**:
  - Mock FileSystem for tests
  - Mock database for uniqueness tests

## Documentation Requirements

- **Code Documentation**: Effect usage examples
- **User Documentation**: Async validation guide
- **Architecture Updates**: Document Effect patterns

## Implementation Notes

- Effect integration is optional/feature-gated
- Sync validators always run first
- Async validators only run if sync passes (configurable)
- Consider parallel execution of async validators
- Error handling must preserve Effect semantics

## Migration and Compatibility

No migration needed - new optional feature.

## Files to Create/Modify

```
src/effect/mod.rs
src/effect/loading.rs
src/effect/async_validator.rs
tests/effect_test.rs
```

## Feature Flags

```toml
[features]
effect = ["stillwater"]
```

## Example Usage

```rust
use postmortem::{Schema, SchemaRegistry, AsyncValidator};
use stillwater::{Effect, Env};

// Define environment with database
struct AppEnv {
    db: Database,
    fs: RealFileSystem,
}

impl SchemaEnv for AppEnv {
    type Fs = RealFileSystem;
    fn filesystem(&self) -> &Self::Fs { &self.fs }
}

// Create async uniqueness validator
struct UniqueUsername {
    db: Database,
}

impl AsyncValidator<AppEnv> for UniqueUsername {
    fn validate_async(
        &self,
        value: &Value,
        path: &JsonPath,
    ) -> Effect<AppEnv, Never, Validation<(), SchemaErrors>> {
        Effect::from_fn(|env| {
            use stillwater::validation::{success, failure};

            let username = value.as_str().unwrap_or("");
            let exists = env.db.username_exists(username);

            if exists {
                Ok(failure(SchemaErrors::single(
                    SchemaError::new(path.clone(), "username already taken")
                        .with_code("unique_username")
                )))
            } else {
                Ok(success(()))
            }
        })
    }
}

// Build schema with async validation
let user_schema = Schema::object()
    .field("email", Schema::string()
        .email()
        .async_custom(UniqueEmail::new()))
    .field("username", Schema::string()
        .min_len(3)
        .async_custom(UniqueUsername::new()));

// Load schemas from directory
let registry = SchemaRegistry::new();
let load_effect = registry.load_dir("./schemas");

// Run in Effect context
let env = AppEnv { db, fs: RealFileSystem };
load_effect.run(&env)?;

// Validate with async
let validate_effect = user_schema.validate_async(&json!({
    "email": "new@example.com",
    "username": "newuser"
}), &JsonPath::root());

let result = validate_effect.run(&env)?;
```

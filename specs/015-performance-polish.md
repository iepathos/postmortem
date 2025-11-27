---
number: 15
title: Performance and Polish
category: optimization
priority: low
status: draft
dependencies: [1, 2, 3, 4, 5, 6, 7, 8, 9]
created: 2025-11-26
---

# Specification 015: Performance and Polish

**Category**: optimization
**Priority**: low
**Status**: draft
**Dependencies**: All previous specs (001-014)

## Context

Before release, postmortem needs performance validation, API polish, and final documentation review. This specification covers benchmarking, optimization opportunities, ergonomic improvements, and release preparation.

## Objective

Ensure postmortem is production-ready:
1. Benchmark validation performance
2. Optimize hot paths
3. Polish API ergonomics
4. Complete documentation
5. Prepare for v0.1 release

## Requirements

### Functional Requirements

1. **Performance Benchmarks**
   - Benchmark string validation
   - Benchmark object validation (various sizes)
   - Benchmark array validation
   - Benchmark nested structures
   - Benchmark schema compilation (if applicable)

2. **Optimizations**
   - Lazy regex compilation (compile on first use)
   - Schema compilation (pre-compute validator chain)
   - Reduce allocations in hot paths
   - Efficient path building

3. **API Polish**
   - Review builder method ergonomics
   - Improve type inference where possible
   - Consistent naming conventions
   - IDE-friendly API design

4. **Documentation**
   - Complete rustdoc coverage
   - User guide document
   - Migration guide (from other validation libs)
   - CHANGELOG for release

5. **Release Preparation**
   - Version 0.1.0 prep
   - Cargo.toml metadata
   - CI/CD configuration
   - License and README

### Non-Functional Requirements

- Validation of 1000-field object < 1ms
- Minimal allocations per validation
- Consistent API patterns throughout
- Documentation builds without warnings

## Acceptance Criteria

- [ ] Benchmarks exist for all schema types
- [ ] Regex compiled lazily (once per schema)
- [ ] Path building uses efficient allocation
- [ ] All public items have rustdoc
- [ ] User guide covers common use cases
- [ ] CHANGELOG documents all features
- [ ] CI runs tests, clippy, fmt
- [ ] Version 0.1.0 ready to publish

## Technical Details

### Implementation Approach

```rust
// Benchmark structure
// benches/validation.rs

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use postmortem::{Schema, JsonPath};
use serde_json::json;

fn string_validation_benchmark(c: &mut Criterion) {
    let schema = Schema::string()
        .min_len(1)
        .max_len(100)
        .pattern(r"^[a-zA-Z0-9_]+$").unwrap();

    let valid_string = json!("valid_username_123");
    let path = JsonPath::root();

    c.bench_function("string_validation", |b| {
        b.iter(|| schema.validate(&valid_string, &path))
    });
}

fn object_validation_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("object_validation");

    for field_count in [10, 100, 1000].iter() {
        let schema = build_object_schema(*field_count);
        let value = build_object_value(*field_count);
        let path = JsonPath::root();

        group.bench_with_input(
            BenchmarkId::from_parameter(field_count),
            field_count,
            |b, _| b.iter(|| schema.validate(&value, &path)),
        );
    }

    group.finish();
}

fn nested_object_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_object");

    for depth in [1, 5, 10, 20].iter() {
        let schema = build_nested_schema(*depth);
        let value = build_nested_value(*depth);
        let path = JsonPath::root();

        group.bench_with_input(
            BenchmarkId::from_parameter(depth),
            depth,
            |b, _| b.iter(|| schema.validate(&value, &path)),
        );
    }

    group.finish();
}

fn array_validation_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("array_validation");

    for item_count in [10, 100, 1000].iter() {
        let schema = Schema::array(Schema::integer().positive());
        let value: Vec<_> = (1..=*item_count).collect();
        let json_value = json!(value);
        let path = JsonPath::root();

        group.bench_with_input(
            BenchmarkId::from_parameter(item_count),
            item_count,
            |b, _| b.iter(|| schema.validate(&json_value, &path)),
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    string_validation_benchmark,
    object_validation_benchmark,
    nested_object_benchmark,
    array_validation_benchmark,
);
criterion_main!(benches);

// Lazy regex compilation
use once_cell::sync::Lazy;
use regex::Regex;

pub struct LazyRegex {
    pattern: String,
    compiled: Lazy<Result<Regex, regex::Error>>,
}

impl LazyRegex {
    pub fn new(pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        let pattern_clone = pattern.clone();
        Self {
            pattern,
            compiled: Lazy::new(move || Regex::new(&pattern_clone)),
        }
    }

    pub fn is_match(&self, text: &str) -> Result<bool, &regex::Error> {
        match &*self.compiled {
            Ok(re) => Ok(re.is_match(text)),
            Err(e) => Err(e),
        }
    }
}

// Efficient path building
#[derive(Clone)]
pub struct JsonPath {
    // Use small string optimization or interning
    segments: SmallVec<[PathSegment; 8]>,
}

impl JsonPath {
    pub fn push_field(&self, name: &str) -> Self {
        let mut segments = self.segments.clone();
        segments.push(PathSegment::Field(name.into()));
        Self { segments }
    }

    // Use a cached string for display
    pub fn to_string_cached(&self) -> &str {
        // Cache the formatted string
        // ...
    }
}

// Schema compilation (optional optimization)
pub struct CompiledSchema {
    validators: Vec<Box<dyn Validator>>,
}

impl StringSchema {
    /// Compile schema for repeated validation
    pub fn compile(&self) -> CompiledSchema {
        let validators: Vec<Box<dyn Validator>> = self.constraints
            .iter()
            .map(|c| c.to_validator())
            .collect();

        CompiledSchema { validators }
    }
}

impl CompiledSchema {
    pub fn validate(&self, value: &Value, path: &JsonPath) -> Validation<ValidatedValue, SchemaErrors> {
        // Fast path: run pre-compiled validators
        let mut errors = Vec::new();

        for validator in &self.validators {
            if let Err(e) = validator.validate(value, path) {
                errors.push(e);
            }
        }

        // ...
    }
}
```

### Performance Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Simple string validation | < 100ns | min_len, max_len |
| String with regex | < 1μs | Compiled regex |
| Object (10 fields) | < 10μs | All string fields |
| Object (100 fields) | < 100μs | All string fields |
| Object (1000 fields) | < 1ms | All string fields |
| Array (1000 items) | < 500μs | Integer items |
| Nested (depth 10) | < 50μs | Small objects |

### API Polish Checklist

- [ ] Builder methods return `Self` consistently
- [ ] Error messages are clear and actionable
- [ ] Type parameters have good defaults
- [ ] Methods that can fail use `Result`
- [ ] Trait bounds are minimal
- [ ] Public types implement common traits (Debug, Clone, etc.)
- [ ] No clippy warnings
- [ ] No rustfmt issues

### Architecture Changes

- Add `benches/` directory for criterion benchmarks
- Add schema compilation option
- Optimize path building

### Data Structures

- `LazyRegex` for lazy compilation
- `CompiledSchema` for pre-compiled validation
- Optimized `JsonPath` with SmallVec

### APIs and Interfaces

```rust
// Schema compilation (optional)
StringSchema::compile(&self) -> CompiledSchema
CompiledSchema::validate(&self, value: &Value, path: &JsonPath) -> Validation<...>

// No new public APIs, focus on optimization of existing
```

## Dependencies

- **Prerequisites**: All previous specs
- **Affected Components**: All schema types
- **External Dependencies**:
  - `criterion` for benchmarking
  - `once_cell` for lazy initialization
  - `smallvec` for small vector optimization

## Testing Strategy

- **Benchmarks**:
  - All schema types benchmarked
  - Various input sizes tested
  - Comparison before/after optimization

- **Regression Tests**:
  - Ensure optimizations don't break correctness
  - Test edge cases

## Documentation Requirements

- **Code Documentation**: Complete rustdoc
- **User Documentation**:
  - Getting started guide
  - Schema reference
  - Best practices
- **Architecture Updates**: Performance notes

## Implementation Notes

- Use `#[inline]` judiciously for hot paths
- Consider `#[cold]` for error paths
- Profile before optimizing
- Document any breaking changes

## Migration and Compatibility

This is the final polish stage. Any API changes should be documented in CHANGELOG.

## Files to Create/Modify

```
benches/validation.rs
docs/guide/README.md
docs/guide/getting-started.md
docs/guide/schema-reference.md
docs/guide/best-practices.md
CHANGELOG.md
README.md (update)
Cargo.toml (metadata)
```

## Release Checklist

```markdown
## v0.1.0 Release Checklist

### Code Quality
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code formatted with rustfmt
- [ ] Benchmarks show acceptable performance

### Documentation
- [ ] All public items have rustdoc
- [ ] Examples compile and run
- [ ] README is complete
- [ ] CHANGELOG lists all features

### Metadata
- [ ] Cargo.toml has correct metadata
- [ ] License file present
- [ ] Repository link correct

### Final Steps
- [ ] Create git tag v0.1.0
- [ ] Publish to crates.io
- [ ] Announce release
```

## Cargo.toml Metadata

```toml
[package]
name = "postmortem"
version = "0.1.0"
edition = "2021"
rust-version = "1.70"
description = "Runtime schema validation with comprehensive error accumulation"
license = "MIT OR Apache-2.0"
repository = "https://github.com/memento-mori/postmortem"
documentation = "https://docs.rs/postmortem"
readme = "README.md"
keywords = ["validation", "schema", "json", "api"]
categories = ["data-structures", "web-programming"]

[badges]
maintenance = { status = "actively-developed" }
```

## Example Usage

```rust
// Performance-critical validation
let schema = Schema::object()
    .field("id", Schema::integer().positive())
    .field("email", Schema::string().email())
    .compile();  // Pre-compile for repeated use

// Validate many items efficiently
for item in items {
    let result = schema.validate(&item, &JsonPath::root());
    // ...
}
```

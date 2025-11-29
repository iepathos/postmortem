# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-11-28

### Added

- **Core Schema Types** - String, Integer, Array, and Object schemas with fluent validation API
- **Error Accumulation** - Integration with stillwater's `Validation` type for collecting all validation errors
- **JSON Path Tracking** - Precise error location tracking with `JsonPath` type (e.g., `users[0].email`)
- **Schema Combinators** - `one_of`, `all_of`, and `exactly_one_of` for complex validation logic
- **Schema Registry** - Define and reuse schemas with reference resolution
- **Schema References** - Support for `$ref` style schema references with `RefSchema`
- **Cross-Field Validation** - Custom validation functions for validating field relationships
- **Recursive Schemas** - Support for self-referential data structures through schema registry
- **Thread Safety** - All schemas are `Send + Sync` for concurrent validation
- **Comprehensive Error Types** - `SchemaError` and `SchemaErrors` with detailed validation failure information

### Schema Features

- **String Schema**: `min_len`, `max_len`, `pattern` (regex), custom validation
- **Integer Schema**: `min`, `max`, custom validation
- **Array Schema**: `min_items`, `max_items`, `items` (element schema), custom validation
- **Object Schema**: `required` and `optional` fields, custom validation

### Dependencies

- `stillwater` 0.11 - Functional patterns (Validation, Semigroup)
- `serde_json` 1.0 - JSON value handling
- `thiserror` 2 - Error types
- `regex` 1 - Pattern matching
- `indexmap` 2.12 - Ordered maps for object schemas
- `parking_lot` 0.12 - Efficient synchronization for registry

[Unreleased]: https://github.com/iepathos/postmortem/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/iepathos/postmortem/releases/tag/v0.1.0

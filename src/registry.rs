//! Schema registry for named schema storage and reference resolution.
//!
//! This module provides the [`SchemaRegistry`] type that stores named schemas
//! and enables schema references to be resolved during validation.

use parking_lot::RwLock;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::SchemaErrors;
use crate::path::JsonPath;
use crate::schema::ValueValidator;
use crate::validation::{RegistryAccess, ValidationContext};
use stillwater::Validation;

/// Type alias for the schema storage map.
type SchemaMap = Arc<RwLock<HashMap<String, Arc<dyn ValueValidator>>>>;

/// A thread-safe registry for storing and retrieving named schemas.
///
/// The registry enables schema reuse through references. Schemas can be
/// registered with string names and then referenced from other schemas
/// using `Schema::ref_()`.
///
/// # Thread Safety
///
/// The registry uses `Arc<RwLock<...>>` for thread-safe access:
/// - Multiple threads can validate concurrently (read-only access)
/// - Registration operations are serialized (write access)
///
/// # Example
///
/// ```rust
/// use postmortem::{SchemaRegistry, Schema};
/// use serde_json::json;
///
/// let registry = SchemaRegistry::new();
///
/// // Register base schemas
/// registry.register("Email", Schema::string()).unwrap();
/// registry.register("UserId", Schema::integer().positive()).unwrap();
///
/// // Register schemas that use references
/// registry.register("User", Schema::object()
///     .field("id", Schema::ref_("UserId"))
///     .field("email", Schema::ref_("Email"))
/// ).unwrap();
/// ```
pub struct SchemaRegistry {
    schemas: SchemaMap,
    max_depth: usize,
}

impl SchemaRegistry {
    /// Creates a new empty schema registry with default max depth (100).
    pub fn new() -> Self {
        Self {
            schemas: Arc::new(RwLock::new(HashMap::new())),
            max_depth: 100,
        }
    }

    /// Sets the maximum reference depth for circular reference prevention.
    ///
    /// The default max depth is 100. When validating recursive schemas,
    /// if the reference chain exceeds this depth, validation fails with
    /// a `max_depth_exceeded` error.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::SchemaRegistry;
    ///
    /// let registry = SchemaRegistry::new()
    ///     .with_max_depth(50);
    /// ```
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Registers a schema with the given name.
    ///
    /// Returns an error if a schema with the same name is already registered.
    ///
    /// # Errors
    ///
    /// Returns `RegistryError::DuplicateName` if the name is already registered.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{SchemaRegistry, Schema};
    ///
    /// let registry = SchemaRegistry::new();
    /// registry.register("Email", Schema::string()).unwrap();
    ///
    /// // Duplicate registration fails
    /// assert!(registry.register("Email", Schema::string()).is_err());
    /// ```
    pub fn register<S>(&self, name: impl Into<String>, schema: S) -> Result<(), RegistryError>
    where
        S: ValueValidator + 'static,
    {
        let name = name.into();
        let mut schemas = self.schemas.write();

        if schemas.contains_key(&name) {
            return Err(RegistryError::DuplicateName(name));
        }

        schemas.insert(name, Arc::new(schema));
        Ok(())
    }

    /// Retrieves a schema by name.
    ///
    /// Returns `None` if no schema with the given name is registered.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{SchemaRegistry, Schema};
    ///
    /// let registry = SchemaRegistry::new();
    /// registry.register("Email", Schema::string()).unwrap();
    ///
    /// let schema = registry.get("Email");
    /// assert!(schema.is_some());
    ///
    /// let missing = registry.get("Unknown");
    /// assert!(missing.is_none());
    /// ```
    pub fn get(&self, name: &str) -> Option<Arc<dyn ValueValidator>> {
        self.schemas.read().get(name).cloned()
    }

    /// Validates that all schema references can be resolved.
    ///
    /// Returns a list of reference names that don't exist in the registry.
    /// This should be called after all schemas are registered to ensure
    /// reference integrity.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{SchemaRegistry, Schema};
    ///
    /// let registry = SchemaRegistry::new();
    /// registry.register("User", Schema::object()
    ///     .field("id", Schema::ref_("UserId"))  // UserId not registered!
    /// ).unwrap();
    ///
    /// let unresolved = registry.validate_refs();
    /// assert_eq!(unresolved, vec!["UserId"]);
    /// ```
    pub fn validate_refs(&self) -> Vec<String> {
        let schemas = self.schemas.read();
        let mut all_refs = Vec::new();

        // Collect all references from all schemas
        for schema in schemas.values() {
            schema.collect_refs(&mut all_refs);
        }

        // Find references that don't exist in registry
        let mut unresolved = Vec::new();
        for ref_name in all_refs {
            if !schemas.contains_key(&ref_name) {
                unresolved.push(ref_name);
            }
        }

        unresolved.sort();
        unresolved.dedup();
        unresolved
    }

    /// Validates a value against a named schema.
    ///
    /// This is the main entry point for validation when using the registry.
    /// It looks up the schema by name and validates the value with full
    /// support for schema references and depth tracking.
    ///
    /// # Errors
    ///
    /// Returns `RegistryError::SchemaNotFound` if the schema name doesn't exist.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{SchemaRegistry, Schema};
    /// use serde_json::json;
    ///
    /// let registry = SchemaRegistry::new();
    /// registry.register("User", Schema::object()
    ///     .field("name", Schema::string().min_len(1))
    ///     .field("age", Schema::integer().positive())
    /// ).unwrap();
    ///
    /// let result = registry.validate("User", &json!({
    ///     "name": "Alice",
    ///     "age": 30
    /// })).unwrap();
    ///
    /// assert!(result.is_success());
    /// ```
    pub fn validate(
        &self,
        schema_name: &str,
        value: &Value,
    ) -> Result<Validation<Value, SchemaErrors>, RegistryError> {
        let schema = self
            .get(schema_name)
            .ok_or_else(|| RegistryError::SchemaNotFound(schema_name.to_string()))?;

        let context = ValidationContext::new(Arc::new(self.clone()), self.max_depth);
        Ok(schema.validate_value_with_context(value, &JsonPath::root(), &context))
    }

    /// Exports all registered schemas as a JSON Schema document with $defs.
    ///
    /// Returns a JSON Schema document following draft 2020-12 with all registered
    /// schemas under the `$defs` key.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{SchemaRegistry, Schema};
    ///
    /// let registry = SchemaRegistry::new();
    /// registry.register("UserId", Schema::integer().positive()).unwrap();
    /// registry.register("Email", Schema::string().email()).unwrap();
    ///
    /// let json_schema = registry.to_json_schema();
    /// // Returns:
    /// // {
    /// //   "$schema": "https://json-schema.org/draft/2020-12/schema",
    /// //   "$defs": {
    /// //     "UserId": { "type": "integer", "exclusiveMinimum": 0 },
    /// //     "Email": { "type": "string", "format": "email" }
    /// //   }
    /// // }
    /// ```
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

    /// Exports a single schema as a standalone JSON Schema document.
    ///
    /// Returns a JSON Schema document for the named schema, including all
    /// referenced schemas under `$defs`. Returns `None` if the schema doesn't exist.
    ///
    /// # Example
    ///
    /// ```rust
    /// use postmortem::{SchemaRegistry, Schema};
    ///
    /// let registry = SchemaRegistry::new();
    /// registry.register("UserId", Schema::integer().positive()).unwrap();
    /// registry.register("User", Schema::object()
    ///     .field("id", Schema::ref_("UserId"))
    ///     .field("email", Schema::string().email())
    /// ).unwrap();
    ///
    /// let user_schema = registry.export_schema("User").unwrap();
    /// // Returns a complete JSON Schema with User schema and UserId in $defs
    /// ```
    pub fn export_schema(&self, name: &str) -> Option<Value> {
        let schema = self.get(name)?;
        let base = self.to_json_schema();

        let mut result = schema.to_json_schema();
        result["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
        result["$defs"] = base["$defs"].clone();

        Some(result)
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SchemaRegistry {
    fn clone(&self) -> Self {
        Self {
            schemas: Arc::clone(&self.schemas),
            max_depth: self.max_depth,
        }
    }
}

impl RegistryAccess for SchemaRegistry {
    fn get_schema(&self, name: &str) -> Option<Arc<dyn ValueValidator>> {
        self.get(name)
    }
}

/// Errors that can occur during registry operations.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// Attempted to register a schema with a name that already exists.
    #[error("schema '{0}' already registered")]
    DuplicateName(String),

    /// Attempted to validate with a schema name that doesn't exist.
    #[error("schema '{0}' not found")]
    SchemaNotFound(String),
}

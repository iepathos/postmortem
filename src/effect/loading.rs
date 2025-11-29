//! Schema loading from filesystem via Effect.
//!
//! This module provides Effect-based schema loading that:
//! - Loads JSON Schema files from a directory
//! - Accumulates parsing and validation errors
//! - Integrates with environment abstraction
//!
//! # API Design Note
//!
//! This implementation uses a simplified API compatible with stillwater 0.12,
//! rather than the full Effect<E, Er, R> type mentioned in the original spec.
//! Functions accept environment parameters directly and return Result or
//! Validation types instead of Effect wrappers.
//!
//! This pragmatic approach provides the same dependency injection benefits:
//! - Testability via trait-based abstractions (SchemaEnv, FileSystem)
//! - Flexibility to swap implementations (mock vs real filesystem)
//! - Clear separation of pure logic from I/O operations
//!
//! While simpler than a full Effect system, this design is well-suited for
//! stillwater 0.12's capabilities and provides a clean, ergonomic API.

use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::registry::{RegistryError, SchemaRegistry};
use crate::schema::Schema;

/// Environment trait for schema operations.
///
/// Implement this trait to provide filesystem access and other
/// environment dependencies for schema loading.
pub trait SchemaEnv: Send + Sync {
    /// The filesystem implementation type
    type Fs: FileSystem;

    /// Returns a reference to the filesystem
    fn filesystem(&self) -> &Self::Fs;
}

/// Abstraction for filesystem operations.
///
/// This trait enables testing with mock filesystems and
/// supports different storage backends.
pub trait FileSystem: Send + Sync {
    /// The error type for filesystem operations
    type Error: std::error::Error + Send + Sync + 'static;

    /// Reads the contents of a file as a string.
    fn read_file(&self, path: &Path) -> Result<String, Self::Error>;

    /// Lists all entries in a directory.
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, Self::Error>;
}

/// Errors that can occur during schema loading.
#[derive(Debug, thiserror::Error)]
pub enum SchemaLoadError {
    /// IO error reading a file
    #[error("IO error reading {0}: {1}")]
    Io(PathBuf, Box<dyn std::error::Error + Send + Sync>),

    /// JSON parsing error
    #[error("Parse error in {0}: {1}")]
    Parse(PathBuf, serde_json::Error),

    /// Schema validation error
    #[error("Schema error in {0}: {1}")]
    Schema(PathBuf, String),

    /// Invalid filename
    #[error("Invalid filename: {0}")]
    InvalidFileName(PathBuf),

    /// Registry error
    #[error("Registry error: {0}")]
    Registry(RegistryError),

    /// Multiple errors occurred
    #[error("Multiple errors: {0:?}")]
    Multiple(Vec<SchemaLoadError>),
}

impl SchemaRegistry {
    /// Loads all JSON Schema files from a directory.
    ///
    /// This method:
    /// - Reads all `.json` files from the specified directory
    /// - Parses each file as JSON Schema
    /// - Registers each schema using the filename (without extension) as the name
    /// - Accumulates all errors that occur
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use postmortem::SchemaRegistry;
    /// use postmortem::effect::SchemaEnv;
    ///
    /// let registry = SchemaRegistry::new();
    /// let env = MyEnv::new();
    /// registry.load_dir_with_env("./schemas", &env)?;
    /// ```
    pub fn load_dir_with_env<E: SchemaEnv>(
        &self,
        path: impl AsRef<Path>,
        env: &E,
    ) -> Result<(), SchemaLoadError> {
        let path = path.as_ref();
        let fs = env.filesystem();
        let files = fs
            .read_dir(path)
            .map_err(|e| SchemaLoadError::Io(path.to_path_buf(), Box::new(e)))?;

        let mut errors = Vec::new();

        for file in files {
            if file.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Err(e) = self.load_schema_file(&file, fs) {
                    errors.push(e);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(SchemaLoadError::Multiple(errors))
        }
    }

    fn load_schema_file<Fs: FileSystem>(
        &self,
        path: &Path,
        fs: &Fs,
    ) -> Result<(), SchemaLoadError> {
        let content = fs
            .read_file(path)
            .map_err(|e| SchemaLoadError::Io(path.to_path_buf(), Box::new(e)))?;

        let json: Value = serde_json::from_str(&content)
            .map_err(|e| SchemaLoadError::Parse(path.to_path_buf(), e))?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| SchemaLoadError::InvalidFileName(path.to_path_buf()))?;

        // Parse the JSON Schema and register it with appropriate type
        parse_and_register_schema(self, name, &json, path)?;
        Ok(())
    }
}

/// Helper function to parse and register a schema with the correct type.
///
/// This function handles the type dispatching to ensure we register
/// the correct concrete schema type.
fn parse_and_register_schema(
    registry: &SchemaRegistry,
    name: &str,
    json: &Value,
    path: &Path,
) -> Result<(), SchemaLoadError> {
    let schema_type = json
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            SchemaLoadError::Schema(path.to_path_buf(), "Missing 'type' field".to_string())
        })?;

    match schema_type {
        "string" => {
            let mut schema = Schema::string();

            if let Some(min_len) = json.get("minLength").and_then(|v| v.as_u64()) {
                schema = schema.min_len(min_len as usize);
            }

            if let Some(max_len) = json.get("maxLength").and_then(|v| v.as_u64()) {
                schema = schema.max_len(max_len as usize);
            }

            if let Some(pattern) = json.get("pattern").and_then(|v| v.as_str()) {
                schema = schema
                    .pattern(pattern)
                    .map_err(|e| SchemaLoadError::Schema(path.to_path_buf(), e.to_string()))?;
            }

            registry
                .register(name, schema)
                .map_err(SchemaLoadError::Registry)
        }
        "integer" => {
            let schema = Schema::integer();
            registry
                .register(name, schema)
                .map_err(SchemaLoadError::Registry)
        }
        "object" => {
            let schema = Schema::object();
            registry
                .register(name, schema)
                .map_err(SchemaLoadError::Registry)
        }
        "array" => {
            let schema = Schema::array(Schema::object());
            registry
                .register(name, schema)
                .map_err(SchemaLoadError::Registry)
        }
        _ => Err(SchemaLoadError::Schema(
            path.to_path_buf(),
            format!("Unsupported schema type: {}", schema_type),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Debug)]
    struct MockFileSystemError(String);

    impl std::fmt::Display for MockFileSystemError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for MockFileSystemError {}

    struct MockFileSystem {
        files: HashMap<PathBuf, String>,
    }

    impl MockFileSystem {
        fn new() -> Self {
            Self {
                files: HashMap::new(),
            }
        }

        fn add_file(&mut self, path: impl Into<PathBuf>, content: impl Into<String>) {
            self.files.insert(path.into(), content.into());
        }
    }

    impl FileSystem for MockFileSystem {
        type Error = MockFileSystemError;

        fn read_file(&self, path: &Path) -> Result<String, Self::Error> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| MockFileSystemError(format!("File not found: {}", path.display())))
        }

        fn read_dir(&self, _path: &Path) -> Result<Vec<PathBuf>, Self::Error> {
            Ok(self.files.keys().cloned().collect())
        }
    }

    struct TestEnv {
        fs: MockFileSystem,
    }

    impl SchemaEnv for TestEnv {
        type Fs = MockFileSystem;

        fn filesystem(&self) -> &Self::Fs {
            &self.fs
        }
    }

    #[test]
    fn test_load_string_schema() {
        let mut fs = MockFileSystem::new();
        fs.add_file(
            "test.json",
            r#"{
                "type": "string",
                "minLength": 1,
                "maxLength": 100
            }"#,
        );

        let env = TestEnv { fs };
        let registry = SchemaRegistry::new();

        let result = registry.load_dir_with_env(".", &env);
        assert!(result.is_ok());

        // Verify schema was registered
        assert!(registry.get("test").is_some());
    }

    #[test]
    fn test_load_multiple_schemas() {
        let mut fs = MockFileSystem::new();
        fs.add_file("email.json", r#"{"type": "string"}"#);
        fs.add_file("age.json", r#"{"type": "integer"}"#);

        let env = TestEnv { fs };
        let registry = SchemaRegistry::new();

        let result = registry.load_dir_with_env(".", &env);
        assert!(result.is_ok());

        assert!(registry.get("email").is_some());
        assert!(registry.get("age").is_some());
    }

    #[test]
    fn test_parse_error_accumulation() {
        let mut fs = MockFileSystem::new();
        fs.add_file("valid.json", r#"{"type": "string"}"#);
        fs.add_file("invalid.json", r#"not valid json"#);

        let env = TestEnv { fs };
        let registry = SchemaRegistry::new();

        let result = registry.load_dir_with_env(".", &env);
        assert!(result.is_err());

        // Valid schema should still be registered
        assert!(registry.get("valid").is_some());
    }
}

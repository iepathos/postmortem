//! Schema validation error types.
//!
//! This module provides [`SchemaError`] for single validation failures and
//! [`SchemaErrors`] for accumulating multiple errors.

use std::fmt::{self, Display};

use stillwater::prelude::*;

use crate::path::JsonPath;

/// A single validation error with full context.
///
/// `SchemaError` captures all relevant information about a validation failure:
/// - **path**: Where in the data structure the error occurred
/// - **message**: Human-readable description of the failure
/// - **got**: The actual value that failed validation (optional)
/// - **expected**: What was expected instead (optional)
/// - **code**: Machine-readable error code for programmatic handling
///
/// # Example
///
/// ```rust
/// use postmortem::{JsonPath, SchemaError};
///
/// let error = SchemaError::new(
///     JsonPath::root().push_field("email"),
///     "invalid email format"
/// )
/// .with_code("invalid_email")
/// .with_got("not-an-email")
/// .with_expected("valid email address");
///
/// assert_eq!(error.code, "invalid_email");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaError {
    /// The path to the value that failed validation.
    pub path: JsonPath,
    /// Human-readable error message.
    pub message: String,
    /// The actual value that was received (formatted as string).
    pub got: Option<String>,
    /// Description of what was expected.
    pub expected: Option<String>,
    /// Machine-readable error code (e.g., `min_length_violated`).
    pub code: String,
}

impl SchemaError {
    /// Creates a new schema error with the given path and message.
    ///
    /// The error code defaults to "validation_error". Use `with_code` to set
    /// a more specific code.
    pub fn new(path: JsonPath, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
            got: None,
            expected: None,
            code: "validation_error".to_string(),
        }
    }

    /// Sets the error code and returns self for chaining.
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = code.into();
        self
    }

    /// Sets the "got" (actual value) field and returns self for chaining.
    pub fn with_got(mut self, got: impl Into<String>) -> Self {
        self.got = Some(got.into());
        self
    }

    /// Sets the "expected" field and returns self for chaining.
    pub fn with_expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }
}

impl Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path_str = if self.path.is_root() {
            "(root)".to_string()
        } else {
            self.path.to_string()
        };

        write!(f, "{}: {}", path_str, self.message)?;

        if let Some(ref expected) = self.expected {
            write!(f, " (expected: {})", expected)?;
        }
        if let Some(ref got) = self.got {
            write!(f, " (got: {})", got)?;
        }

        Ok(())
    }
}

impl std::error::Error for SchemaError {}

// SchemaError is Send + Sync since all fields are owned types
// (String, JsonPath with Vec<PathSegment>, Option<String>)
// This is automatically derived, but we add these assertions to ensure
// it remains true if the types change.
const _: () = {
    const fn assert_send<T: Send>() {}
    const fn assert_sync<T: Sync>() {}
    assert_send::<SchemaError>();
    assert_sync::<SchemaError>();
};

/// A non-empty collection of schema validation errors.
///
/// `SchemaErrors` wraps a `NonEmptyVec<SchemaError>` to guarantee that at least
/// one error is present. This is essential for use with `Validation<T, SchemaErrors>`
/// since a failure must have at least one error.
///
/// # Combining Errors
///
/// `SchemaErrors` implements `Semigroup`, allowing errors from multiple
/// validations to be combined:
///
/// ```rust
/// use postmortem::{JsonPath, SchemaError, SchemaErrors};
/// use stillwater::prelude::*;
///
/// let errors1 = SchemaErrors::single(
///     SchemaError::new(JsonPath::root().push_field("name"), "required")
/// );
/// let errors2 = SchemaErrors::single(
///     SchemaError::new(JsonPath::root().push_field("email"), "invalid format")
/// );
///
/// let combined = errors1.combine(errors2);
/// assert_eq!(combined.len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaErrors(NonEmptyVec<SchemaError>);

impl SchemaErrors {
    /// Creates a `SchemaErrors` containing a single error.
    pub fn single(error: SchemaError) -> Self {
        Self(NonEmptyVec::singleton(error))
    }

    /// Creates a `SchemaErrors` from a `NonEmptyVec` of errors.
    pub fn from_non_empty(errors: NonEmptyVec<SchemaError>) -> Self {
        Self(errors)
    }

    /// Returns the number of errors in this collection.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns false since this collection is guaranteed non-empty.
    ///
    /// This method exists for API consistency but always returns false.
    pub fn is_empty(&self) -> bool {
        false // NonEmptyVec is never empty
    }

    /// Returns an iterator over the contained errors.
    pub fn iter(&self) -> impl Iterator<Item = &SchemaError> {
        self.0.iter()
    }

    /// Returns all errors at the specified path.
    pub fn at_path(&self, path: &JsonPath) -> Vec<&SchemaError> {
        self.0.iter().filter(|e| &e.path == path).collect()
    }

    /// Returns all errors with the specified error code.
    pub fn with_code(&self, code: &str) -> Vec<&SchemaError> {
        self.0.iter().filter(|e| e.code == code).collect()
    }

    /// Returns the first error in the collection.
    pub fn first(&self) -> &SchemaError {
        self.0.head()
    }

    /// Converts this collection into a `Vec<SchemaError>`.
    pub fn into_vec(self) -> Vec<SchemaError> {
        self.0.into_vec()
    }

    /// Returns a reference to the underlying `NonEmptyVec`.
    pub fn as_non_empty_vec(&self) -> &NonEmptyVec<SchemaError> {
        &self.0
    }

    /// Creates a `SchemaErrors` from a `Vec<SchemaError>`.
    ///
    /// Returns the `SchemaErrors` if the vec is non-empty, or panics if empty.
    /// Use this when you're certain the vec contains at least one error.
    ///
    /// # Panics
    ///
    /// Panics if the provided vec is empty.
    pub fn from_vec(errors: Vec<SchemaError>) -> Self {
        Self(NonEmptyVec::from_vec(errors).expect("SchemaErrors requires at least one error"))
    }
}

impl Semigroup for SchemaErrors {
    fn combine(self, other: Self) -> Self {
        SchemaErrors(self.0.combine(other.0))
    }
}

impl Display for SchemaErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Validation failed with {} error(s):", self.len())?;
        for (i, error) in self.iter().enumerate() {
            writeln!(f, "  {}. {}", i + 1, error)?;
        }
        Ok(())
    }
}

impl std::error::Error for SchemaErrors {}

impl IntoIterator for SchemaErrors {
    type Item = SchemaError;
    type IntoIter = std::vec::IntoIter<SchemaError>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_vec().into_iter()
    }
}

impl<'a> IntoIterator for &'a SchemaErrors {
    type Item = &'a SchemaError;
    type IntoIter = Box<dyn Iterator<Item = &'a SchemaError> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.0.iter())
    }
}

// SchemaErrors is Send + Sync since it only contains SchemaError which is Send + Sync
const _: () = {
    const fn assert_send<T: Send>() {}
    const fn assert_sync<T: Sync>() {}
    assert_send::<SchemaErrors>();
    assert_sync::<SchemaErrors>();
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_error_creation() {
        let error = SchemaError::new(JsonPath::root().push_field("name"), "field is required");

        assert_eq!(error.path, JsonPath::root().push_field("name"));
        assert_eq!(error.message, "field is required");
        assert_eq!(error.code, "validation_error");
        assert!(error.got.is_none());
        assert!(error.expected.is_none());
    }

    #[test]
    fn test_schema_error_builder() {
        let error = SchemaError::new(JsonPath::root().push_field("age"), "must be positive")
            .with_code("min_value")
            .with_got("-5")
            .with_expected("value >= 0");

        assert_eq!(error.code, "min_value");
        assert_eq!(error.got, Some("-5".to_string()));
        assert_eq!(error.expected, Some("value >= 0".to_string()));
    }

    #[test]
    fn test_schema_error_display() {
        let error = SchemaError::new(JsonPath::root().push_field("email"), "invalid format")
            .with_expected("email address")
            .with_got("not-an-email");

        let display = error.to_string();
        assert!(display.contains("email: invalid format"));
        assert!(display.contains("expected: email address"));
        assert!(display.contains("got: not-an-email"));
    }

    #[test]
    fn test_schema_error_display_root() {
        let error = SchemaError::new(JsonPath::root(), "value is null");
        let display = error.to_string();
        assert!(display.contains("(root): value is null"));
    }

    #[test]
    fn test_schema_errors_single() {
        let error = SchemaError::new(JsonPath::root(), "test");
        let errors = SchemaErrors::single(error.clone());

        assert_eq!(errors.len(), 1);
        assert!(!errors.is_empty());
        assert_eq!(errors.first(), &error);
    }

    #[test]
    fn test_schema_errors_combine() {
        let error1 = SchemaError::new(JsonPath::root().push_field("a"), "error 1");
        let error2 = SchemaError::new(JsonPath::root().push_field("b"), "error 2");

        let errors1 = SchemaErrors::single(error1);
        let errors2 = SchemaErrors::single(error2);
        let combined = errors1.combine(errors2);

        assert_eq!(combined.len(), 2);
    }

    #[test]
    fn test_schema_errors_at_path() {
        let path_a = JsonPath::root().push_field("a");
        let path_b = JsonPath::root().push_field("b");

        let error1 = SchemaError::new(path_a.clone(), "error 1").with_code("code1");
        let error2 = SchemaError::new(path_a.clone(), "error 2").with_code("code2");
        let error3 = SchemaError::new(path_b.clone(), "error 3").with_code("code1");

        let errors = SchemaErrors::single(error1)
            .combine(SchemaErrors::single(error2))
            .combine(SchemaErrors::single(error3));

        let at_a = errors.at_path(&path_a);
        assert_eq!(at_a.len(), 2);

        let at_b = errors.at_path(&path_b);
        assert_eq!(at_b.len(), 1);
    }

    #[test]
    fn test_schema_errors_with_code() {
        let error1 =
            SchemaError::new(JsonPath::root().push_field("a"), "error 1").with_code("required");
        let error2 =
            SchemaError::new(JsonPath::root().push_field("b"), "error 2").with_code("invalid");
        let error3 =
            SchemaError::new(JsonPath::root().push_field("c"), "error 3").with_code("required");

        let errors = SchemaErrors::single(error1)
            .combine(SchemaErrors::single(error2))
            .combine(SchemaErrors::single(error3));

        let required = errors.with_code("required");
        assert_eq!(required.len(), 2);

        let invalid = errors.with_code("invalid");
        assert_eq!(invalid.len(), 1);
    }

    #[test]
    fn test_schema_errors_iteration() {
        let error1 = SchemaError::new(JsonPath::root().push_field("a"), "error 1");
        let error2 = SchemaError::new(JsonPath::root().push_field("b"), "error 2");

        let errors = SchemaErrors::single(error1).combine(SchemaErrors::single(error2));

        let collected: Vec<_> = errors.iter().collect();
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn test_schema_errors_into_iter() {
        let error1 = SchemaError::new(JsonPath::root().push_field("a"), "error 1");
        let error2 = SchemaError::new(JsonPath::root().push_field("b"), "error 2");

        let errors = SchemaErrors::single(error1).combine(SchemaErrors::single(error2));

        let collected: Vec<SchemaError> = errors.into_iter().collect();
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn test_schema_errors_display() {
        let error1 = SchemaError::new(JsonPath::root().push_field("name"), "required");
        let error2 = SchemaError::new(JsonPath::root().push_field("email"), "invalid");

        let errors = SchemaErrors::single(error1).combine(SchemaErrors::single(error2));
        let display = errors.to_string();

        assert!(display.contains("2 error(s)"));
        assert!(display.contains("name: required"));
        assert!(display.contains("email: invalid"));
    }

    #[test]
    fn test_semigroup_associativity() {
        let e1 = SchemaErrors::single(SchemaError::new(JsonPath::root(), "1"));
        let e2 = SchemaErrors::single(SchemaError::new(JsonPath::root(), "2"));
        let e3 = SchemaErrors::single(SchemaError::new(JsonPath::root(), "3"));

        // (e1 <> e2) <> e3
        let left = e1.clone().combine(e2.clone()).combine(e3.clone());
        // e1 <> (e2 <> e3)
        let right = e1.combine(e2.combine(e3));

        // Should have same errors (associativity)
        assert_eq!(left.len(), right.len());
        let left_msgs: Vec<_> = left.iter().map(|e| &e.message).collect();
        let right_msgs: Vec<_> = right.iter().map(|e| &e.message).collect();
        assert_eq!(left_msgs, right_msgs);
    }
}

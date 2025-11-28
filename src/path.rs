//! JSON path representation for locating values in nested structures.
//!
//! This module provides [`JsonPath`] and [`PathSegment`] types for building
//! and representing paths to values in nested JSON-like structures.

use std::fmt::{self, Display};

/// A segment of a JSON path.
///
/// Paths are built from segments that represent either field access or array indexing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PathSegment {
    /// A field/property access (e.g., `user`, `email`)
    Field(String),
    /// An array index access (e.g., `[0]`, `[42]`)
    Index(usize),
}

impl PathSegment {
    /// Creates a new field segment.
    pub fn field(name: impl Into<String>) -> Self {
        PathSegment::Field(name.into())
    }

    /// Creates a new index segment.
    pub fn index(idx: usize) -> Self {
        PathSegment::Index(idx)
    }
}

/// A path to a value in a nested JSON-like structure.
///
/// `JsonPath` represents locations like `users[0].email` and provides
/// methods for building paths incrementally.
///
/// # Example
///
/// ```rust
/// use postmortem::JsonPath;
///
/// let path = JsonPath::root()
///     .push_field("users")
///     .push_index(0)
///     .push_field("email");
///
/// assert_eq!(path.to_string(), "users[0].email");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct JsonPath {
    segments: Vec<PathSegment>,
}

impl JsonPath {
    /// Creates an empty path representing the root value.
    pub fn root() -> Self {
        Self::default()
    }

    /// Creates a path from a single field segment.
    pub fn from_field(name: impl Into<String>) -> Self {
        Self {
            segments: vec![PathSegment::Field(name.into())],
        }
    }

    /// Creates a path from a single index segment.
    pub fn from_index(idx: usize) -> Self {
        Self {
            segments: vec![PathSegment::Index(idx)],
        }
    }

    /// Returns a new path with a field segment appended.
    ///
    /// This method does not modify the original path; it returns a new one.
    pub fn push_field(&self, name: impl Into<String>) -> Self {
        let mut segments = self.segments.clone();
        segments.push(PathSegment::Field(name.into()));
        Self { segments }
    }

    /// Returns a new path with an index segment appended.
    ///
    /// This method does not modify the original path; it returns a new one.
    pub fn push_index(&self, index: usize) -> Self {
        let mut segments = self.segments.clone();
        segments.push(PathSegment::Index(index));
        Self { segments }
    }

    /// Returns true if this is the root path (no segments).
    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// Returns the number of segments in this path.
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Returns true if this path has no segments.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Returns an iterator over the path segments.
    pub fn segments(&self) -> impl Iterator<Item = &PathSegment> {
        self.segments.iter()
    }

    /// Returns the parent path (all segments except the last), or None if this is root.
    pub fn parent(&self) -> Option<Self> {
        if self.segments.is_empty() {
            None
        } else {
            Some(Self {
                segments: self.segments[..self.segments.len() - 1].to_vec(),
            })
        }
    }

    /// Returns the last segment, or None if this is root.
    pub fn last(&self) -> Option<&PathSegment> {
        self.segments.last()
    }
}

impl Display for JsonPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, segment) in self.segments.iter().enumerate() {
            match segment {
                PathSegment::Field(name) => {
                    if i > 0 {
                        write!(f, ".")?;
                    }
                    write!(f, "{}", name)?;
                }
                PathSegment::Index(idx) => write!(f, "[{}]", idx)?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_path_is_empty() {
        let path = JsonPath::root();
        assert!(path.is_root());
        assert!(path.is_empty());
        assert_eq!(path.len(), 0);
        assert_eq!(path.to_string(), "");
    }

    #[test]
    fn test_single_field() {
        let path = JsonPath::root().push_field("user");
        assert_eq!(path.to_string(), "user");
        assert_eq!(path.len(), 1);
    }

    #[test]
    fn test_single_index() {
        let path = JsonPath::root().push_index(0);
        assert_eq!(path.to_string(), "[0]");
    }

    #[test]
    fn test_nested_fields() {
        let path = JsonPath::root().push_field("user").push_field("email");
        assert_eq!(path.to_string(), "user.email");
    }

    #[test]
    fn test_field_with_index() {
        let path = JsonPath::root().push_field("users").push_index(0);
        assert_eq!(path.to_string(), "users[0]");
    }

    #[test]
    fn test_complex_path() {
        let path = JsonPath::root()
            .push_field("users")
            .push_index(0)
            .push_field("email");
        assert_eq!(path.to_string(), "users[0].email");
    }

    #[test]
    fn test_deeply_nested() {
        let path = JsonPath::root()
            .push_field("body")
            .push_field("data")
            .push_index(42)
            .push_field("items")
            .push_index(0)
            .push_field("name");
        assert_eq!(path.to_string(), "body.data[42].items[0].name");
    }

    #[test]
    fn test_path_immutability() {
        let base = JsonPath::root().push_field("users");
        let path_a = base.push_index(0);
        let path_b = base.push_index(1);

        assert_eq!(base.to_string(), "users");
        assert_eq!(path_a.to_string(), "users[0]");
        assert_eq!(path_b.to_string(), "users[1]");
    }

    #[test]
    fn test_parent_path() {
        let path = JsonPath::root()
            .push_field("users")
            .push_index(0)
            .push_field("email");

        let parent = path.parent().unwrap();
        assert_eq!(parent.to_string(), "users[0]");

        let grandparent = parent.parent().unwrap();
        assert_eq!(grandparent.to_string(), "users");

        let root = grandparent.parent().unwrap();
        assert!(root.is_root());

        assert!(root.parent().is_none());
    }

    #[test]
    fn test_from_constructors() {
        let field_path = JsonPath::from_field("name");
        assert_eq!(field_path.to_string(), "name");

        let index_path = JsonPath::from_index(5);
        assert_eq!(index_path.to_string(), "[5]");
    }

    #[test]
    fn test_last_segment() {
        let path = JsonPath::root().push_field("users").push_index(0);
        assert_eq!(path.last(), Some(&PathSegment::Index(0)));

        let root = JsonPath::root();
        assert_eq!(root.last(), None);
    }

    #[test]
    fn test_segments_iterator() {
        let path = JsonPath::root()
            .push_field("a")
            .push_index(1)
            .push_field("b");

        let segments: Vec<_> = path.segments().collect();
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0], &PathSegment::Field("a".to_string()));
        assert_eq!(segments[1], &PathSegment::Index(1));
        assert_eq!(segments[2], &PathSegment::Field("b".to_string()));
    }

    #[test]
    fn test_equality() {
        let path1 = JsonPath::root().push_field("a").push_index(0);
        let path2 = JsonPath::root().push_field("a").push_index(0);
        let path3 = JsonPath::root().push_field("a").push_index(1);

        assert_eq!(path1, path2);
        assert_ne!(path1, path3);
    }

    #[test]
    fn test_clone() {
        let path = JsonPath::root().push_field("test");
        let cloned = path.clone();
        assert_eq!(path, cloned);
    }
}

//! Validation context for schema reference resolution.
//!
//! This module provides the [`ValidationContext`] type that carries registry information
//! and depth tracking during validation. It enables schema references to be resolved
//! and prevents infinite loops in circular references.

use std::sync::Arc;

/// Validation context carries registry and depth tracking information.
///
/// ValidationContext is passed through the validation call chain to enable:
/// - Schema reference resolution via registry lookup
/// - Depth tracking to prevent infinite loops in circular references
/// - Thread-safe access to shared registry
///
/// The context uses Arc for the registry to avoid lifetime constraints
/// and enable flexible ownership patterns during validation.
#[derive(Clone)]
pub struct ValidationContext {
    registry: Arc<dyn RegistryAccess>,
    depth: usize,
    max_depth: usize,
}

impl ValidationContext {
    /// Creates a new validation context with a registry and max depth limit.
    pub fn new(registry: Arc<dyn RegistryAccess>, max_depth: usize) -> Self {
        Self {
            registry,
            depth: 0,
            max_depth,
        }
    }

    /// Creates a new context with incremented depth.
    ///
    /// This is called when following a schema reference to track the depth
    /// of the reference chain and prevent infinite loops.
    pub fn increment_depth(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            depth: self.depth + 1,
            max_depth: self.max_depth,
        }
    }

    /// Returns the current depth of reference traversal.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the maximum allowed depth.
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Returns a reference to the registry for schema lookups.
    pub fn registry(&self) -> &dyn RegistryAccess {
        &*self.registry
    }
}

/// Trait for accessing schemas from a registry.
///
/// This trait abstracts registry access to avoid circular dependencies
/// between the validation module and the registry module.
pub trait RegistryAccess: Send + Sync {
    /// Gets a schema by name from the registry.
    fn get_schema(
        &self,
        name: &str,
    ) -> Option<Arc<dyn crate::schema::ValueValidator>>;
}

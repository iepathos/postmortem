//! Integration tests for JsonPath.

use postmortem::{JsonPath, PathSegment};

#[test]
fn test_path_construction_and_display() {
    // Root path
    assert_eq!(JsonPath::root().to_string(), "");

    // Simple field
    assert_eq!(JsonPath::root().push_field("name").to_string(), "name");

    // Simple index
    assert_eq!(JsonPath::root().push_index(0).to_string(), "[0]");

    // Complex nested path
    let path = JsonPath::root()
        .push_field("users")
        .push_index(0)
        .push_field("address")
        .push_field("city");
    assert_eq!(path.to_string(), "users[0].address.city");
}

#[test]
fn test_path_segments_preserved() {
    let path = JsonPath::root()
        .push_field("data")
        .push_index(42)
        .push_field("value");

    let segments: Vec<&PathSegment> = path.segments().collect();
    assert_eq!(segments.len(), 3);

    match &segments[0] {
        PathSegment::Field(name) => assert_eq!(name, "data"),
        _ => panic!("Expected Field segment"),
    }

    match &segments[1] {
        PathSegment::Index(idx) => assert_eq!(*idx, 42),
        _ => panic!("Expected Index segment"),
    }

    match &segments[2] {
        PathSegment::Field(name) => assert_eq!(name, "value"),
        _ => panic!("Expected Field segment"),
    }
}

#[test]
fn test_path_is_immutable() {
    let base = JsonPath::root().push_field("items");

    let path1 = base.push_index(0);
    let path2 = base.push_index(1);
    let path3 = base.push_field("count");

    // Base path unchanged
    assert_eq!(base.to_string(), "items");

    // Each branch is independent
    assert_eq!(path1.to_string(), "items[0]");
    assert_eq!(path2.to_string(), "items[1]");
    assert_eq!(path3.to_string(), "items.count");
}

#[test]
fn test_path_equality() {
    let path1 = JsonPath::root().push_field("a").push_index(0);
    let path2 = JsonPath::root().push_field("a").push_index(0);
    let path3 = JsonPath::root().push_field("a").push_index(1);
    let path4 = JsonPath::root().push_field("b").push_index(0);

    assert_eq!(path1, path2);
    assert_ne!(path1, path3);
    assert_ne!(path1, path4);
}

#[test]
fn test_path_parent_chain() {
    let path = JsonPath::root()
        .push_field("a")
        .push_field("b")
        .push_index(0);

    let parent1 = path.parent().expect("should have parent");
    assert_eq!(parent1.to_string(), "a.b");

    let parent2 = parent1.parent().expect("should have parent");
    assert_eq!(parent2.to_string(), "a");

    let parent3 = parent2.parent().expect("should have parent");
    assert!(parent3.is_root());

    assert!(parent3.parent().is_none());
}

#[test]
fn test_consecutive_indices() {
    let path = JsonPath::root().push_index(0).push_index(1).push_index(2);
    assert_eq!(path.to_string(), "[0][1][2]");
}

#[test]
fn test_from_constructors() {
    let field = JsonPath::from_field("name");
    assert_eq!(field.to_string(), "name");
    assert_eq!(field.len(), 1);

    let index = JsonPath::from_index(5);
    assert_eq!(index.to_string(), "[5]");
    assert_eq!(index.len(), 1);
}

#[test]
fn test_path_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(JsonPath::root().push_field("a"));
    set.insert(JsonPath::root().push_field("b"));
    set.insert(JsonPath::root().push_field("a")); // duplicate

    assert_eq!(set.len(), 2);
}

#[test]
fn test_path_debug() {
    let path = JsonPath::root().push_field("test").push_index(0);
    let debug = format!("{:?}", path);
    assert!(debug.contains("JsonPath"));
    assert!(debug.contains("Field"));
    assert!(debug.contains("Index"));
}

//! Smoke test - verifies the crate compiles and basic items are reachable.

#[test]
fn version_is_set() {
    assert!(!arena_lib::VERSION.is_empty());
}
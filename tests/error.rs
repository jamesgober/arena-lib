//! Audit-grade coverage for every public [`Error`] variant.
//!
//! `Error::CapacityExceeded` and `Error::CounterOverflow` are reachable in
//! shipping code only on global-allocator failure (effectively never) or
//! after a `u32::MAX` slot exhaustion (~4 billion ops), respectively.
//! Constructing them directly and exercising their `Display` /
//! `std::error::Error` impls is the only practical way to keep them
//! covered.

use arena_lib::{Arena, Error};

#[test]
fn every_variant_has_non_empty_display() {
    for variant in [
        Error::StaleIndex,
        Error::CapacityExceeded,
        Error::CounterOverflow,
    ] {
        let rendered = format!("{variant}");
        assert!(!rendered.is_empty(), "{variant:?} has empty Display");
        assert!(
            rendered.chars().all(|c| !c.is_control() || c == ' '),
            "{variant:?} Display contains control chars: {rendered:?}"
        );
    }
}

#[test]
fn error_is_std_error() {
    let e: Box<dyn std::error::Error> = Box::new(Error::StaleIndex);
    assert!(e.source().is_none(), "Error variants have no nested source");
}

#[test]
fn error_variants_are_equatable() {
    assert_eq!(Error::StaleIndex, Error::StaleIndex);
    assert_ne!(Error::StaleIndex, Error::CapacityExceeded);
    assert_ne!(Error::CapacityExceeded, Error::CounterOverflow);
}

#[test]
fn arena_remove_returns_none_for_unknown_handle() {
    // Live exercise of the StaleIndex code path via the public API:
    // remove a handle, then attempt to use it again.
    let mut arena: Arena<u32> = Arena::new();
    let h = arena.insert(42);
    assert_eq!(arena.remove(h), Some(42));
    assert!(arena.get(h).is_none());
    assert_eq!(arena.remove(h), None);
}

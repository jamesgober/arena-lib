//! Property-based tests for the invariants the public APIs promise.
//!
//! Properties exercised:
//!
//! - **Arena**: every inserted value is retrievable via its handle; removed
//!   handles never resolve again, even after slot reuse.
//! - **Interner**: idempotent — every distinct input maps to a stable
//!   `Symbol`, and `resolve(intern(s)) == Some(s)`.
//! - **Bump**: alloc-then-read round-trips for any sequence of values;
//!   the cursor only ever advances.

use arena_lib::{Arena, Bump, Interner};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        ..ProptestConfig::default()
    })]

    /// Every freshly inserted value is reachable via the returned handle.
    #[test]
    fn arena_insert_then_get(values in prop::collection::vec(any::<u32>(), 0..200)) {
        let mut arena: Arena<u32> = Arena::with_capacity(values.len());
        let handles: Vec<_> = values.iter().map(|v| (arena.insert(*v), *v)).collect();
        for (idx, expected) in &handles {
            prop_assert_eq!(arena.get(*idx), Some(expected));
        }
        prop_assert_eq!(arena.len(), values.len());
    }

    /// After a remove, the handle is stale forever — even if the slot is
    /// reused by a later insert (which gets a fresh generation).
    #[test]
    fn arena_remove_invalidates_handle(values in prop::collection::vec(any::<u32>(), 1..50)) {
        let mut arena: Arena<u32> = Arena::new();
        let handles: Vec<_> = values.iter().map(|v| arena.insert(*v)).collect();
        for h in &handles {
            let _ = arena.remove(*h);
        }
        for h in &handles {
            prop_assert!(arena.get(*h).is_none(), "removed handle must not resolve");
        }
        // Refill with new values and confirm the old handles still stay stale.
        let new_handles: Vec<_> = values.iter().map(|v| arena.insert(*v)).collect();
        for old in &handles {
            prop_assert!(arena.get(*old).is_none(), "old handle must remain stale after slot reuse");
        }
        for n in new_handles {
            prop_assert!(arena.get(n).is_some());
        }
    }

    /// Live count never exceeds inserts minus removes; iteration sees
    /// exactly the live elements.
    #[test]
    fn arena_len_matches_iter(values in prop::collection::vec(any::<u8>(), 0..100)) {
        let mut arena: Arena<u8> = Arena::new();
        let handles: Vec<_> = values.iter().map(|v| arena.insert(*v)).collect();
        // Remove every third handle.
        let mut removed = 0;
        for (i, h) in handles.iter().enumerate() {
            if i % 3 == 0 && arena.remove(*h).is_some() {
                removed += 1;
            }
        }
        prop_assert_eq!(arena.len(), values.len() - removed);
        prop_assert_eq!(arena.iter().count(), arena.len());
    }

    /// The interner is idempotent and `resolve` is a left-inverse of `intern`.
    #[test]
    fn interner_idempotent_round_trip(inputs in prop::collection::vec("[a-z]{1,16}", 0..50)) {
        let mut interner = Interner::new();
        let mut symbols = Vec::new();
        for s in &inputs {
            let sym = interner.intern(s);
            symbols.push((sym, s.clone()));
        }
        // Re-interning every input yields the same symbol.
        for (expected_sym, s) in &symbols {
            prop_assert_eq!(interner.intern(s), *expected_sym);
        }
        // Resolve always returns the original string.
        for (sym, s) in &symbols {
            prop_assert_eq!(interner.resolve(*sym), Some(s.as_str()));
        }
        // `len` matches the count of distinct inputs.
        let distinct: std::collections::BTreeSet<&String> = inputs.iter().collect();
        prop_assert_eq!(interner.len(), distinct.len());
    }

    /// `lookup` never inserts.
    #[test]
    fn interner_lookup_does_not_mutate(queries in prop::collection::vec("[a-z]{1,8}", 0..30)) {
        let interner = Interner::new();
        let before = interner.len();
        for s in &queries {
            let result = interner.lookup(s);
            // Without prior inserts, no query can return Some.
            prop_assert!(result.is_none());
        }
        prop_assert_eq!(interner.len(), before);
    }

    /// Every value pushed into a bump arena reads back identically while
    /// the arena is alive.
    #[test]
    fn bump_alloc_round_trips(values in prop::collection::vec(any::<u64>(), 0..200)) {
        let bump = Bump::with_capacity(8);
        let refs: Vec<_> = values.iter().map(|v| {
            let ptr: *const u64 = bump.alloc(*v);
            (ptr, *v)
        }).collect();
        // We cannot hold &mut T and &mut Bump simultaneously, but the
        // raw pointer captured above remains valid for the lifetime of
        // `bump` (the chunks are not freed). SAFETY: each `ptr` came from
        // `bump.alloc`, the chunks own the memory, and no one has
        // written to those slots since.
        for (ptr, expected) in refs {
            let observed = unsafe { *ptr };
            prop_assert_eq!(observed, expected);
        }
    }

    /// The allocated-bytes counter is monotonically non-decreasing across
    /// any sequence of allocations (no double-counting, no rewinds).
    #[test]
    fn bump_allocated_bytes_monotonic(values in prop::collection::vec(any::<u32>(), 0..100)) {
        let bump = Bump::with_capacity(64);
        let mut prev = bump.allocated_bytes();
        for v in &values {
            let _ = bump.alloc(*v);
            let current = bump.allocated_bytes();
            prop_assert!(current >= prev, "allocated_bytes must be monotonic");
            prev = current;
        }
    }
}

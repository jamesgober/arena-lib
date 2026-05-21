//! End-to-end smoke test covering every public primitive together.

use arena_lib::prelude::*;

#[test]
fn version_is_set() {
    assert!(!arena_lib::VERSION.is_empty());
    assert!(arena_lib::VERSION.starts_with(char::is_numeric));
}

#[test]
fn end_to_end_workflow() {
    // 1) Build an arena of session records keyed by stable handles.
    let mut arena: Arena<Session> = Arena::with_capacity(16);

    // 2) Build an interner for the user-id strings shared across sessions.
    let mut interner = Interner::with_capacity(8);

    // 3) Build a bump arena for the per-frame scratch buffers we cite
    //    from each session record.
    let bump = Bump::with_capacity(256);

    let alice_id = interner.intern("user:alice");
    let bob_id = interner.intern("user:bob");
    let alice_again = interner.intern("user:alice");
    assert_eq!(
        alice_id, alice_again,
        "interner must deduplicate equal inputs"
    );

    let alice_buf: &mut [u8; 4] = bump.alloc([1, 2, 3, 4]);
    let bob_buf: &mut [u8; 4] = bump.alloc([5, 6, 7, 8]);
    let alice_addr = alice_buf.as_ptr() as usize;
    let bob_addr = bob_buf.as_ptr() as usize;

    // Sanity-check the bump-allocator property: two consecutive
    // 4-byte allocations land in adjacent slots of the same chunk.
    assert_eq!(
        bob_addr - alice_addr,
        core::mem::size_of_val(alice_buf),
        "bump must place sequential allocations contiguously"
    );

    let alice = arena.insert(Session {
        user: alice_id,
        scratch_start: alice_addr,
    });
    let bob = arena.insert(Session {
        user: bob_id,
        scratch_start: bob_addr,
    });

    // 4) The arena resolves both handles and yields the original session
    //    payloads.
    assert_eq!(arena.len(), 2);
    let resolved_alice = arena.get(alice).map(|s| (s.user, s.scratch_start));
    let resolved_bob = arena.get(bob).map(|s| (s.user, s.scratch_start));
    assert_eq!(resolved_alice, Some((alice_id, alice_addr)));
    assert_eq!(resolved_bob, Some((bob_id, bob_addr)));

    // 5) Removing alice invalidates her handle but leaves bob intact.
    let removed = arena
        .remove(alice)
        .unwrap_or_else(|| panic!("alice must be present"));
    assert_eq!(removed.user, alice_id);
    assert!(arena.get(alice).is_none(), "stale handle must not resolve");
    assert!(arena.get(bob).is_some(), "bob must remain reachable");

    // 6) Iteration over the arena yields only live sessions, in slot order.
    let surviving_user_ids: Vec<_> = arena.iter().map(|(_, s)| s.user).collect();
    assert_eq!(surviving_user_ids, vec![bob_id]);

    // 7) The interner round-trips the symbol back to the original bytes.
    assert_eq!(interner.resolve(alice_id), Some("user:alice"));
    assert_eq!(interner.resolve(bob_id), Some("user:bob"));

    // 8) The bump arena reports both allocations against its capacity.
    assert!(
        bump.allocated_bytes() >= 8,
        "bump must account for both 4-byte allocations"
    );
}

struct Session {
    user: Symbol,
    scratch_start: usize,
}

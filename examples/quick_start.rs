//! End-to-end tour of the four primitives in `arena-lib`.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example quick_start
//! ```
//!
//! The output is a short transcript showing each primitive in use, plus
//! a final diagnostic line summarising the arena / interner / bump state.

use arena_lib::prelude::*;

fn main() {
    // ---------------------------------------------------------------
    // 1) Generational arena — stable handles, use-after-free detection.
    // ---------------------------------------------------------------
    let mut arena: Arena<&'static str> = Arena::with_capacity(8);
    let alice = arena.insert("alice");
    let bob = arena.insert("bob");
    let charlie = arena.insert("charlie");
    println!(
        "arena: inserted alice={:?} bob={:?} charlie={:?}; len={}",
        alice,
        bob,
        charlie,
        arena.len()
    );

    // Removing alice retires her handle; subsequent lookups return None.
    let removed = arena.remove(alice).unwrap_or_else(|| {
        panic!("alice must be present");
    });
    println!(
        "arena: removed alice ({removed:?}); alice.get -> {:?}",
        arena.get(alice)
    );

    // ---------------------------------------------------------------
    // 2) String interner — O(1) equality on repeated identifiers.
    // ---------------------------------------------------------------
    let mut interner = Interner::with_capacity(8);
    let session = interner.intern("session-key");
    let session_again = interner.intern("session-key");
    let token = interner.intern("auth-token");
    println!(
        "interner: session={:?} session_again={:?} token={:?}; len={}; equal={}",
        session,
        session_again,
        token,
        interner.len(),
        session == session_again
    );

    // ---------------------------------------------------------------
    // 3) Bump arena — fast scratch, grows on demand.
    // ---------------------------------------------------------------
    let bump = Bump::with_capacity(64);
    let buf_a = bump.alloc([1_u8, 2, 3, 4]);
    let buf_b = bump.alloc([5_u8, 6, 7, 8]);
    println!(
        "bump: alloc'd two 4-byte slabs; allocated_bytes={} chunk_count={}",
        bump.allocated_bytes(),
        bump.chunk_count()
    );
    // The references coexist because each call returned a disjoint slice.
    assert_eq!(buf_a, &[1, 2, 3, 4]);
    assert_eq!(buf_b, &[5, 6, 7, 8]);

    // ---------------------------------------------------------------
    // 4) DropArena — bump-style allocation that runs destructors.
    // ---------------------------------------------------------------
    let owned = DropArena::<String>::new();
    let s = owned.alloc(String::from("freed when `owned` is dropped"));
    println!(
        "drop arena: parked String ({} bytes); arena.len={}",
        s.len(),
        owned.len()
    );

    // ---------------------------------------------------------------
    // Final state summary.
    // ---------------------------------------------------------------
    println!(
        "summary: arena.len={} interner.len={} bump.allocated_bytes={} owned.len={}",
        arena.len(),
        interner.len(),
        bump.allocated_bytes(),
        owned.len(),
    );
}

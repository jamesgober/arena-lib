//! A slot whose generation counter is exhausted must be retired, never
//! reused under a wrapped generation.
//!
//! The full cycle takes about 4 billion insert/remove pairs, so the test
//! is ignored by default. Run it in release:
//! `cargo test --release --test generation_wrap -- --ignored`.

use arena_lib::{Arena, Error};

#[test]
#[ignore = "about 4 billion operations; run with --release -- --ignored"]
fn stale_index_never_resurrects_after_generation_exhaustion() {
    let mut arena: Arena<u64> = Arena::new();
    let stale = arena.insert(0);
    assert_eq!(arena.remove(stale), Some(0));
    // The slot is reused LIFO, so every cycle bumps the same generation.
    for i in 1..=u64::from(u32::MAX) {
        let idx = arena.insert(i);
        let _ = arena.remove(idx);
    }
    let fresh = arena.insert(0xdead);
    assert_eq!(arena.get(stale), None, "stale handle must stay stale");
    assert_eq!(arena.try_get(stale), Err(Error::StaleIndex));
    assert_ne!(
        fresh.slot(),
        stale.slot(),
        "an exhausted slot must be retired"
    );
    assert_eq!(arena.get(fresh), Some(&0xdead));
}

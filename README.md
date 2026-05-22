<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <strong>arena-lib</strong>
    <br>
    <sup><sub>TYPED MEMORY ARENAS AND SLAB ALLOCATION</sub></sup>
</h1>

<p align="center">
    <a href="https://crates.io/crates/arena-lib"><img alt="crates.io" src="https://img.shields.io/crates/v/arena-lib.svg"></a>
    <a href="https://crates.io/crates/arena-lib"><img alt="downloads" src="https://img.shields.io/crates/d/arena-lib.svg?color=0099ff"></a>
    <a href="https://docs.rs/arena-lib"><img alt="docs.rs" src="https://docs.rs/arena-lib/badge.svg"></a>
    <a href="https://github.com/jamesgober/arena-lib/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/jamesgober/arena-lib/actions/workflows/ci.yml/badge.svg"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md" title="MSRV"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85%2B-blue"></a>
</p>

<p align="center">Generational indices, typed arenas, interned strings, bump allocation. Zero unsafe leakage into user code.</p>

<br>

## Why arena-lib

Allocator-aware Rust normally means juggling three or four crates: one for slab storage, one for handle stability, one for string interning, plus a bump arena for short-lived scratch. `arena-lib` collects those primitives behind a single, safe, REPS-disciplined surface so you can move fast without paying for it later.

Designed around four guarantees:

- **Typed arenas** — one backing allocation per element type, predictable layout, cache-friendly traversal.
- **Generational indices** — stable handles that catch use-after-free without reference counting.
- **String interning** — O(1) equality and compact storage for repeated identifiers.
- **Bump allocation** — short-lived scratch regions that reset in constant time.

Every public path is safe Rust. `unsafe` lives only in measured, documented internals — never in your call sites.

> **Status:** v1.0.0 — **stable**. API frozen. Within the 1.x line, only purely additive changes are permitted (new methods on existing types, new variants on the `#[non_exhaustive]` `Error` enum). Anything that would break a 1.x caller is out of scope until a hypothetical 2.0.

---

## Quick start

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
arena-lib = "1"
```

End-to-end use of every primitive:

```rust
use arena_lib::prelude::*;

fn main() {
    // Generational arena — stable handles, use-after-free detection.
    let mut arena: Arena<&'static str> = Arena::with_capacity(8);
    let alice = arena.insert("alice");
    let bob = arena.insert("bob");
    assert_eq!(arena.get(alice), Some(&"alice"));

    // String interner — O(1) equality on repeated identifiers.
    let mut interner = Interner::with_capacity(8);
    let id_a = interner.intern("user:alice");
    let id_b = interner.intern("user:alice");
    assert_eq!(id_a, id_b);

    // Bump arena — fast scratch, grows on demand, O(1) reset.
    let bump = Bump::with_capacity(64);
    let scratch = bump.alloc([0_u8; 16]);
    assert_eq!(scratch.len(), 16);

    // Drop arena — same ergonomics but runs destructors on drop.
    let owned = DropArena::<String>::new();
    let s = owned.alloc(String::from("freed when `owned` is dropped"));
    assert!(s.contains("dropped"));

    // Removing a slot invalidates its handle without touching the rest.
    assert_eq!(arena.remove(alice), Some("alice"));
    assert!(arena.get(alice).is_none());
    assert!(arena.get(bob).is_some());
}
```

See [docs/API.md](docs/API.md) for the full reference, including the planned 1.0 surface.

---

## Standards

- **REPS** governs every decision. See [REPS.md](REPS.md).
- **MSRV:** Rust 1.85.
- **Edition:** 2024.
- **Cross-platform:** Linux, macOS, Windows.

---

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.


<!-- FOOT COPYRIGHT
################################################# -->
<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>JAMES GOBER.</strong></sup>
</div>
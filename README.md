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

> **Status:** Early scaffolding (v0.1.0). The public API is being designed for the 1.0 release. Today the crate compiles, exposes `VERSION`, and is safe to depend on for tracking — full allocator surfaces land in upcoming milestones.

---

## Quick start

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
arena-lib = "0.1"
```

Verify the dependency is wired up:

```rust
use arena_lib::VERSION;

fn main() {
    println!("running arena-lib {VERSION}");
}
```

The full allocator API lands in the 0.2 milestone. See [docs/API.md](docs/API.md) for the live API reference.

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
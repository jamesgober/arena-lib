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

## What it does

Typed memory arena and slab allocator library. Generational indices, typed arenas (one allocation per type), interned strings, and bump allocation. Zero unsafe leakage into user code.

---

## Quick start

```toml
[dependencies]
arena-lib = "0.1"
```

---

## Standards

- **REPS** governs every decision. See [REPS.md](REPS.md).
- **MSRV:** Rust 1.75.
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
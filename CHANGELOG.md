# Changelog

All notable changes to `arena-lib` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

### Changed

### Fixed

### Security

---

## [0.9.0] - 2026-05-21

Pre-1.0 hardening + audit. Feature freeze. The 0.5 public surface is preserved exactly; this release closes documentation, test, and example gaps and logs the audit findings.

### Added

- `examples/quick_start.rs` — runnable end-to-end tour of all four primitives. Invoke with `cargo run --example quick_start`.
- `tests/error.rs` — audit-grade coverage of every `Error` variant: non-empty `Display` output, `std::error::Error` wiring, equality semantics, and a live `StaleIndex` round-trip via the public API.
- Property tests for `DropArena`: alloc round-trip across chunk-growth events; `Drop` runs destructors exactly once per value parked into the arena.
- Method-level rustdoc examples on every non-trivial public method (`Arena::remove`, `Arena::iter`, `Interner::resolve`, `Interner::lookup`, `Bump::try_alloc`, `Bump::reset`, `DropArena::with_chunk_capacity`).

### Changed

- `Symbol` and `Interner::resolve` docstrings tightened: clarified that symbols are *opaque handles tied to a single interner*; passing a foreign symbol to `resolve` is undefined at the API contract level (may return `None` or an unrelated string), not enforced.
- Cleaned `tests/smoke.rs`: removed `#[allow(dead_code)]` on the `scratch_start` field by adding an assertion that proves the bump-allocator's contiguous-allocation property.

### Verified

- Feature freeze — no public type, method, or error variant added or removed since 0.5.
- Code cleanliness — no `TODO` / `FIXME` / `HACK` markers in `src/`; every `#[allow(...)]` carries a `reason = "..."` justification.
- Tests — 59 passing (27 unit + 9 property + 4 error + 2 smoke + 17 doctest).
- Benches — all four benchmark binaries compile.
- Docs — `cargo doc --no-deps --all-features` and `cargo doc --no-deps` both clean with `RUSTDOCFLAGS=-D warnings`.

---

## [0.5.0] - 2026-05-21

### Added

- `DropArena<T>` — typed bump-style arena that runs destructors on drop. Multi-chunk internally, alloc-from-`&self`, `Send` when `T: Send`.
- Property-based tests under `tests/properties.rs` (proptest) covering arena handle invariants, interner idempotency, and bump round-trips.
- Criterion benchmarks under `benches/arena.rs`, `benches/intern.rs`, `benches/bump.rs` exercising insert / get / churn for the arena, unique / repeated intern + resolve for the interner, and alloc / reset for the bump.
- `Bump::chunk_count()` accessor for diagnostics.

### Changed

- **Interner** switched from `alloc::collections::BTreeMap` to `hashbrown::HashMap` for the de-duplication index. `intern`, `lookup`, and `contains` are now expected O(1). `Interner::new()` and `Interner::with_capacity` are no longer `const` (hashbrown's `HashMap::new` is not `const`).
- **Bump** is now a multi-chunk linear allocator. `alloc` and `try_alloc` allocate a new chunk on demand and are effectively infallible — `try_alloc` returns `Err(Error::CapacityExceeded)` only when the global allocator itself fails. `Bump::with_capacity(n)` pre-allocates an initial chunk; subsequent chunks default to `max(n, 4 KiB)`. After `reset`, existing chunks are retained and refilled before any new chunk is requested. `Bump::chunk_capacity()` now returns the total bytes across all chunks (previously: the single chunk's bytes).
- Added `hashbrown 0.15` (default features off, `default-hasher` + `inline-more`) as a runtime dependency.
- Added `criterion 0.5` and `proptest 1` as dev-dependencies.

---

## [0.2.0] - 2026-05-21

### Added

- `Arena<T>` generational arena with stable `Index` handles, slot recycling with generation bump, `insert` / `try_insert` / `remove` / `get` / `get_mut` / `contains` / `clear` / `iter` / `iter_mut` / `reserve` / `capacity` / `len` / `is_empty`.
- `Interner` string interner with compact `Symbol` handles, idempotent `intern` / `try_intern`, non-inserting `lookup` / `contains`, `resolve`, ordered-map backing for `no_std` compatibility, `iter` over insertion order.
- `Bump` single-chunk bump arena with O(1) `alloc` / `try_alloc` / `reset`, alignment-aware allocations, interior mutability through `UnsafeCell`, drop-policy documented (no destructors on reset).
- Single crate-wide `Error` enum (`StaleIndex`, `CapacityExceeded`, `CounterOverflow`) with `Display`, `std::error::Error` (under `std`), and a `Result<T>` alias.
- `arena_lib::prelude` re-exporting the common types.
- Cross-platform end-to-end smoke test exercising all four primitives together.
- Full `docs/API.md` reference for the 0.2 surface with nested TOC, per-type cost summaries, and worked examples.

### Changed

- CI split into separate jobs: `fmt` / `clippy` / `docs` run on `ubuntu-latest` only; `test` runs the full `[ubuntu, macos, windows] × [stable, 1.85.0]` matrix.
- `actions/cache` bumped from `v4` to `v5`; removed unused `actions/setup-node` step; opted into Node 24 via `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24`.
- README quick-start now shows the full prelude-based example for v0.2 instead of the `VERSION`-only scaffold example.

---

## [0.1.0] - 2026-05-18

### Added

- Initial scaffold and repository bootstrap.
- REPS compliance baseline.
- CI for Linux/macOS/Windows on stable and MSRV (1.85).

[Unreleased]: https://github.com/jamesgober/arena-lib/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/jamesgober/arena-lib/compare/v0.5.0...v0.9.0
[0.5.0]: https://github.com/jamesgober/arena-lib/compare/v0.2.0...v0.5.0
[0.2.0]: https://github.com/jamesgober/arena-lib/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/arena-lib/releases/tag/v0.1.0

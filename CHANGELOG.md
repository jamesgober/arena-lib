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

[Unreleased]: https://github.com/jamesgober/arena-lib/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/jamesgober/arena-lib/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/arena-lib/releases/tag/v0.1.0

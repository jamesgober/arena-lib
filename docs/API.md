<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br><b>arena-lib</b><br>
    <sub><sup>API REFERENCE</sup></sub>
</h1>
<div align="center">
    <sup>
        <a href="../README.md" title="Project Home"><b>HOME</b></a>
        <span>&nbsp;│&nbsp;</span>
        <a href="../CHANGELOG.md" title="Changelog"><b>CHANGELOG</b></a>
        <span>&nbsp;│&nbsp;</span>
        <span>API</span>
        <span>&nbsp;│&nbsp;</span>
        <a href="./release/" title="Release Notes"><b>RELEASES</b></a>
    </sup>
</div>
<br>

This document is the canonical reference for every public-facing item in the `arena-lib` crate. It tracks the source of truth in `src/` and is updated before every release.

> **Status:** `arena-lib` is in early scaffolding (v0.1.0). The full allocator surface — typed arenas, generational indices, string interning, and bump allocation — is being designed for the 1.0 release. Only the items listed under [Public APIs](#public-apis) are stable today. New sections will be added here as each milestone lands.

<br>

## Table of Contents

- **[Installation](#installation)**
- **[Quick Start](#quick-start)**
- **[Public APIs](#public-apis)**
  - [Crate-level documentation](#crate-level-documentation)
  - [Constants](#constants)
    - [`VERSION`](#version)
- **[Feature Flags](#feature-flags)**
  - [`std`](#feature-std)
- **[Compatibility](#compatibility)**
- **[Planned API Surface (1.0)](#planned-api-surface-10)**
- **[Notes](#notes)**

<br><br>

<h2 id="installation">Installation</h2>

Add `arena-lib` to your `Cargo.toml`:

```toml
[dependencies]
arena-lib = "0.1"
```

Or with `cargo`:

```bash
cargo add arena-lib
```

To pin the latest published patch:

```toml
[dependencies]
arena-lib = { version = "0.1", default-features = true }
```

To build without the standard library:

```toml
[dependencies]
arena-lib = { version = "0.1", default-features = false }
```

See [Feature Flags](#feature-flags) for the full feature matrix.

<br>

<h2 id="quick-start">Quick Start</h2>

The 0.1.0 release exposes a single constant. Wire the crate in and confirm the dependency resolves:

```rust
use arena_lib::VERSION;

fn main() {
    println!("arena-lib version: {VERSION}");
}
```

That's the entire usable surface at this milestone. Subsequent releases will expand this section with allocator construction, handle issuance, and bump-region examples. See [Planned API Surface](#planned-api-surface-10) for what's coming and on what timeline.

<br>

<h2 id="public-apis">Public APIs</h2>

Every item listed in this section is part of the published, semver-tracked surface of `arena-lib`. Items not listed here are either internal or not yet released.

<br>

<h3 id="crate-level-documentation">Crate-level documentation</h3>

The crate root carries the high-level rustdoc that appears on the [docs.rs landing page](https://docs.rs/arena-lib). It restates the project goals (typed arenas, generational indices, string interning, bump allocation), the safety posture (`unsafe` is internal and measured), and links back to the repository for the roadmap.

**Lint posture:** the crate root configures `#![deny(...)]` for the REPS-mandated lints, including `missing_docs`, `unsafe_op_in_unsafe_fn`, `unused_must_use`, `unused_results`, `clippy::unwrap_used`, `clippy::expect_used`, `clippy::todo`, `clippy::unimplemented`, `clippy::print_stdout`, `clippy::print_stderr`, `clippy::dbg_macro`, `clippy::undocumented_unsafe_blocks`, and `clippy::missing_safety_doc`. Downstream crates are unaffected — these lints apply to `arena-lib` itself.

<br>

<h3 id="constants">Constants</h3>

<br>

<h4 id="version"><code>VERSION</code></h4>

```rust
pub const VERSION: &str;
```

Crate version string, populated by Cargo at build time from `CARGO_PKG_VERSION`. Mirrors the `version` field in `Cargo.toml` exactly.

**Type:** `&'static str`

**Stability:** stable since `0.1.0`. The value changes on every release.

**When to use it:**

- Surfacing the embedded `arena-lib` version in your binary's `--version` output.
- Tagging diagnostics, traces, or metrics with the allocator version in use.
- Sanity-checking that a downstream crate picked up the version you expected.

**Examples**

Read and print the version:

```rust
use arena_lib::VERSION;

println!("arena-lib v{VERSION}");
```

Assert a non-empty, well-formed value (useful in your own smoke tests):

```rust
use arena_lib::VERSION;

assert!(!VERSION.is_empty());
assert!(VERSION.chars().next().is_some_and(|c| c.is_ascii_digit()));
```

Embed the allocator version in a structured log line:

```rust
use arena_lib::VERSION;

fn startup_banner(service: &str) -> String {
    format!("{service} | arena-lib={VERSION}")
}

let line = startup_banner("ingest");
assert!(line.contains("arena-lib="));
```

<br><br>

<h2 id="feature-flags">Feature Flags</h2>

`arena-lib` is `no_std`-compatible by design. Cargo features control which capabilities are compiled in.

| Feature | Default | Description |
| ------- | :-----: | ----------- |
| [`std`](#feature-std) |   yes   | Enables use of `std`. Disable for `no_std` consumers. |

<br>

<h3 id="feature-std"><code>std</code></h3>

Enables the standard library. On by default. Disable it when targeting `no_std` environments:

```toml
[dependencies]
arena-lib = { version = "0.1", default-features = false }
```

Re-enable explicitly:

```toml
[dependencies]
arena-lib = { version = "0.1", default-features = false, features = ["std"] }
```

When `std` is disabled, the crate compiles under `#![no_std]`. Today the public surface is identical in both modes; this may change as the allocator API lands and certain conveniences require `alloc` or `std`.

<br><br>

<h2 id="compatibility">Compatibility</h2>

| Concern        | Value                                  |
| -------------- | -------------------------------------- |
| **MSRV**       | Rust `1.85`                            |
| **Edition**    | `2024`                                 |
| **Platforms**  | Linux, macOS, Windows (Tier-1 targets) |
| **`no_std`**   | Supported via `default-features = false` |
| **Unsafe**     | Internal only; never leaks to user code |
| **License**    | `Apache-2.0 OR MIT`                    |

The crate runs identically on all Tier-1 targets. Platform-specific behavior — if any is ever introduced — will be feature-gated and documented at the call site.

<br><br>

<h2 id="planned-api-surface-10">Planned API Surface (1.0)</h2>

The items below are **not yet implemented**. They define the contract `arena-lib` is being built toward. This list will be replaced by full API documentation as each milestone lands.

| Area                    | Coming in | Purpose |
| ----------------------- | :-------: | ------- |
| Typed arenas            | `0.2.0`   | One backing allocation per element type; predictable layout; cache-friendly traversal. |
| Generational indices    | `0.2.0`   | Stable handles that detect use-after-free without reference counting. |
| String interning        | `0.5.0`   | O(1) equality and compact storage for repeated identifiers. |
| Bump allocation         | `0.5.0`   | Short-lived scratch regions resettable in constant time. |
| Error type              | `0.2.0`   | Single public error enum covering allocation, exhaustion, and index validation failures. |
| Benchmarks              | `0.5.0`   | Hot-path microbenchmarks committed under `benches/`. |
| Hardening + audit       | `0.9.0`   | Feature freeze, REPS audit, doc completeness, cross-platform CI green on stable + MSRV. |
| Stable 1.0              | `1.0.0`   | Final API freeze, published to crates.io. |

Until the items above land, this document tracks only what ships today.

<br><br>

<h2 id="notes">Notes</h2>

- **Source of truth.** This document mirrors `src/lib.rs`. If you find a divergence, the source wins — and the divergence is a doc bug worth filing.
- **Stability.** Every item under [Public APIs](#public-apis) is part of the semver contract from the listed version onward. The crate is pre-1.0; minor releases may still introduce additions, but documented items will not silently change shape.
- **Safety.** `arena-lib` never exposes `unsafe` in its public surface. Internal `unsafe` blocks carry `// SAFETY:` documentation and are measured against alternatives before being kept.
- **Reporting issues.** File documentation bugs, missing examples, or API gaps at the [project repository](https://github.com/jamesgober/arena-lib/issues).

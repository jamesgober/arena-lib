//! # arena-lib
//!
//! TYPED MEMORY ARENAS AND SLAB ALLOCATION
//!
//! Generational indices, typed arenas, interned strings, bump allocation. Zero unsafe leakage into user code.
//!
//! # Status
//!
//! Early scaffolding (v0.1.0). The public API is being designed for the 1.0 release.
//! The crate currently compiles and exposes [`VERSION`] only. See
//! [the repository](https://github.com/jamesgober/arena-lib) for the milestone plan.
//!
//! # License
//!
//! Dual-licensed under Apache-2.0 OR MIT.

#![doc(html_root_url = "https://docs.rs/arena-lib")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unused_must_use)]
#![deny(unused_results)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::missing_safety_doc)]

/// Crate version string, populated by Cargo at build time.
///
/// Matches the `version` field in `Cargo.toml` exactly. Useful for diagnostics,
/// telemetry, and `--version` output in tools that embed `arena-lib`.
///
/// # Examples
///
/// ```
/// use arena_lib::VERSION;
///
/// assert!(!VERSION.is_empty());
/// assert!(VERSION.chars().next().is_some_and(|c| c.is_ascii_digit()));
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

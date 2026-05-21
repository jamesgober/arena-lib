//! # arena-lib
//!
//! TYPED MEMORY ARENAS AND SLAB ALLOCATION
//!
//! `arena-lib` collects four allocator primitives behind a single, safe Rust
//! surface: a generational [`Arena`], a string [`Interner`], a [`Bump`]
//! arena, and the supporting [`Index`] / [`Symbol`] / [`Error`] types. Every
//! public path is safe Rust; `unsafe` is internal-only, measured, and
//! documented at the call site.
//!
//! # Quick tour
//!
//! ```
//! use arena_lib::{Arena, Bump, Interner};
//!
//! // Generational arena — stable handles, use-after-free detection.
//! let mut arena = Arena::new();
//! let alice = arena.insert("alice");
//! let bob = arena.insert("bob");
//! assert_eq!(arena.get(alice), Some(&"alice"));
//! let _ = arena.remove(alice);
//! assert_eq!(arena.get(alice), None); // stale handle, safely rejected
//!
//! // Interner — O(1) equality on repeated identifiers.
//! let mut interner = Interner::new();
//! let id_a = interner.intern("user:42");
//! let id_b = interner.intern("user:42");
//! assert_eq!(id_a, id_b);
//!
//! // Bump arena — fast scratch, reset in O(1).
//! let mut bump = Bump::with_capacity(64);
//! let n = bump.alloc(7_u32);
//! assert_eq!(*n, 7);
//! bump.reset();
//!
//! let _ = bob;
//! ```
//!
//! # Modules
//!
//! - [`arena`] — generational arena and [`Index`] handle.
//! - [`intern`] — string interner and [`Symbol`] handle.
//! - [`bump`] — multi-chunk bump arena for short-lived scratch (no drop).
//! - [`drop_arena`] — typed bump-style arena that runs destructors.
//! - [`error`] — single public [`Error`] type and [`Result`] alias.
//! - [`prelude`] — convenience re-exports for downstream crates.
//!
//! # `no_std`
//!
//! Disable default features (`std`) to compile under `#![no_std]`. The crate
//! still requires `alloc` — it is pulled in automatically.
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

extern crate alloc;

pub mod arena;
pub mod bump;
pub mod drop_arena;
pub mod error;
pub mod intern;
pub mod prelude;

pub use crate::arena::{Arena, Index};
pub use crate::bump::Bump;
pub use crate::drop_arena::DropArena;
pub use crate::error::{Error, Result};
pub use crate::intern::{Interner, Symbol};

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

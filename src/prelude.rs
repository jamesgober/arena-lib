//! Convenience re-exports.
//!
//! Glob-import this module to bring the most commonly used types into scope:
//!
//! ```
//! use arena_lib::prelude::*;
//!
//! let mut arena: Arena<&'static str> = Arena::new();
//! let mut interner: Interner = Interner::new();
//! let bump: Bump = Bump::with_capacity(64);
//!
//! let _ = arena.insert("alice");
//! let _ = interner.intern("session-key");
//! let _ = bump.alloc(7_u32);
//! ```

pub use crate::arena::{Arena, Index};
pub use crate::bump::Bump;
pub use crate::drop_arena::DropArena;
pub use crate::error::{Error, Result};
pub use crate::intern::{Interner, Symbol};

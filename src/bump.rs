//! Bump arena for short-lived scratch allocations.
//!
//! [`Bump`] is a single-chunk linear allocator. Allocations are O(1) (bump a
//! pointer, write the value), and the entire arena clears in O(1) via
//! [`Bump::reset`]. Multiple references handed out from a shared `&self`
//! borrow coexist safely because each allocation hands out a disjoint slice
//! of the chunk.
//!
//! # Cost summary
//!
//! - `alloc` / `try_alloc`: O(1) (pointer bump + write).
//! - `reset`: O(1) (resets the offset; does **not** drop values).
//! - `allocated_bytes` / `chunk_capacity`: O(1).
//!
//! # Drop behavior
//!
//! `Bump` does not run destructors when reset or dropped. Allocate types
//! that do not require `Drop` (anything that is `Copy`, or owns only
//! arena-internal memory). The 0.5 milestone introduces a typed drop-arena
//! variant for cases where this restriction is too tight.
//!
//! # Growth policy (current and planned)
//!
//! 0.2 ships a **single-chunk** allocator: capacity is fixed at
//! construction time and [`Bump::try_alloc`] returns
//! [`Error::CapacityExceeded`] when full. The 0.5 milestone replaces the
//! internal chunk with a linked list of chunks so that `alloc` becomes
//! effectively infallible.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::ptr::NonNull;

use crate::error::{Error, Result};

/// Single-chunk bump arena. See the [module-level docs](self).
///
/// # Examples
///
/// ```
/// use arena_lib::Bump;
///
/// let mut bump = Bump::with_capacity(64);
/// let a = bump.alloc(7_u32);
/// let b = bump.alloc(42_u32);
/// assert_eq!(*a, 7);
/// assert_eq!(*b, 42);
///
/// // Resetting clears the offset; capacity is retained.
/// bump.reset();
/// assert_eq!(bump.allocated_bytes(), 0);
/// ```
pub struct Bump {
    chunk: Box<[u8]>,
    state: UnsafeCell<State>,
}

struct State {
    offset: usize,
}

impl Bump {
    /// Creates an empty bump arena that can satisfy only zero-sized
    /// allocations.
    ///
    /// Use [`Bump::with_capacity`] for any non-trivial use.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            chunk: Vec::<u8>::new().into_boxed_slice(),
            state: UnsafeCell::new(State { offset: 0 }),
        }
    }

    /// Creates a bump arena backed by a fixed-size chunk of `capacity` bytes.
    ///
    /// `capacity` is the upper bound on the *byte* footprint of every
    /// allocation made against this arena before [`Bump::reset`] is called.
    /// Account for alignment padding in addition to raw value size.
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let buf: Vec<u8> = vec![0; capacity];
        Self {
            chunk: buf.into_boxed_slice(),
            state: UnsafeCell::new(State { offset: 0 }),
        }
    }

    /// Total capacity of the underlying chunk, in bytes.
    #[inline]
    #[must_use]
    pub fn chunk_capacity(&self) -> usize {
        self.chunk.len()
    }

    /// Bytes consumed since the most recent [`Bump::reset`] (or since
    /// construction).
    ///
    /// Includes alignment padding.
    #[inline]
    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        // SAFETY: `&self` read of `state.offset` is sound because the only
        // writers are `try_alloc_layout` (which advances the offset
        // monotonically through `&self`) and `reset` (which requires
        // `&mut self`, excluding any concurrent `&self` access). `Bump` is
        // not `Sync`, so cross-thread `&self` access is rejected at
        // compile time.
        unsafe { (*self.state.get()).offset }
    }

    /// Allocates `value` and returns a unique reference to it.
    ///
    /// Panics if the chunk does not have room for `value` (after alignment
    /// padding). Use [`Bump::try_alloc`] for an explicit fallible variant.
    ///
    /// # Examples
    ///
    /// ```
    /// use arena_lib::Bump;
    ///
    /// let bump = Bump::with_capacity(16);
    /// let n = bump.alloc(123_u32);
    /// assert_eq!(*n, 123);
    /// ```
    #[inline]
    #[allow(
        clippy::mut_from_ref,
        reason = "interior mutability via UnsafeCell; each call returns a disjoint region of the chunk"
    )]
    pub fn alloc<T>(&self, value: T) -> &mut T {
        match self.try_alloc(value) {
            Ok(reference) => reference,
            Err(_) => panic!("bump arena exhausted; allocate with `with_capacity` accordingly"),
        }
    }

    /// Allocates `value`, returning `Ok(&mut T)` on success or
    /// [`Error::CapacityExceeded`] if the chunk is full.
    ///
    /// The returned `&mut T` borrows from `&self` because the [`Bump`]
    /// owns the underlying chunk and hands out non-overlapping regions
    /// per call — the same pattern used by `bumpalo::Bump::alloc`.
    #[allow(
        clippy::mut_from_ref,
        reason = "interior mutability via UnsafeCell; each call returns a disjoint region of the chunk"
    )]
    pub fn try_alloc<T>(&self, value: T) -> Result<&mut T> {
        let layout = Layout::new::<T>();
        let raw = self.try_alloc_layout(layout)?;
        let typed = raw.cast::<T>();
        // SAFETY: `typed` is non-null, aligned for `T`, and points into a
        // freshly reserved disjoint region of the chunk. Writing `value`
        // initialises the slot before any read.
        unsafe { core::ptr::write(typed.as_ptr(), value) };
        // SAFETY: lifetime is tied to `&self`. The chunk is a `Box<[u8]>`
        // owned by `self`, so its address is stable. `try_alloc_layout`
        // guarantees no future allocation reuses this region until
        // `reset` is called (which requires `&mut self`, invalidating all
        // outstanding `&self` borrows including this returned reference).
        Ok(unsafe { &mut *typed.as_ptr() })
    }

    /// Resets the arena, marking every prior allocation as discarded.
    ///
    /// Capacity is retained. Destructors of previously allocated values are
    /// **not** run (see the [module-level docs](self) for the Drop policy).
    /// Taking `&mut self` ensures no outstanding references survive across
    /// the call.
    #[inline]
    pub fn reset(&mut self) {
        // SAFETY: `&mut self` excludes any outstanding `&T` / `&mut T`
        // produced by `try_alloc`, so resetting the offset cannot dangle
        // any live reference.
        unsafe { (*self.state.get()).offset = 0 };
    }

    fn try_alloc_layout(&self, layout: Layout) -> Result<NonNull<u8>> {
        // SAFETY: see `allocated_bytes` — `&self` access to `state` is
        // sound because `Bump` is not `Sync` and the only `&mut self`
        // operation (`reset`) excludes concurrent `&self` callers.
        let state = unsafe { &mut *self.state.get() };
        let base = self.chunk.as_ptr() as usize;
        let cursor = match base.checked_add(state.offset) {
            Some(c) => c,
            None => return Err(Error::CapacityExceeded),
        };
        let align_mask = layout.align().wrapping_sub(1);
        let aligned = match cursor.checked_add(align_mask) {
            Some(a) => a & !align_mask,
            None => return Err(Error::CapacityExceeded),
        };
        let end = match aligned.checked_add(layout.size()) {
            Some(e) => e,
            None => return Err(Error::CapacityExceeded),
        };
        let limit = base.saturating_add(self.chunk.len());
        if end > limit {
            return Err(Error::CapacityExceeded);
        }

        state.offset = end - base;

        if layout.size() == 0 {
            return Ok(dangling_aligned(layout.align()));
        }

        // SAFETY: `aligned` is in-bounds for the chunk (verified by the
        // `end > limit` check above) and properly aligned for `layout`.
        // The pointer is non-null because the chunk has non-zero length
        // (otherwise `end > limit` would have fired for any non-ZST).
        let ptr = aligned as *mut u8;
        NonNull::new(ptr).ok_or(Error::CapacityExceeded)
    }
}

impl Default for Bump {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for Bump {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Bump")
            .field("allocated_bytes", &self.allocated_bytes())
            .field("chunk_capacity", &self.chunk_capacity())
            .finish()
    }
}

fn dangling_aligned(align: usize) -> NonNull<u8> {
    // SAFETY: `align` is a non-zero power of two (invariant of `Layout`),
    // so casting it to a pointer yields a non-null, properly aligned
    // dangling pointer suitable for zero-sized accesses only.
    unsafe { NonNull::new_unchecked(align as *mut u8) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_returns_unique_reference() {
        let bump = Bump::with_capacity(32);
        let a = bump.alloc(1_u32);
        let b = bump.alloc(2_u32);
        assert_eq!(*a, 1);
        assert_eq!(*b, 2);
        assert!(bump.allocated_bytes() >= 8);
    }

    #[test]
    fn try_alloc_fails_when_full() {
        let bump = Bump::with_capacity(4);
        let _ = bump.alloc(1_u32);
        assert!(bump.try_alloc(2_u32).is_err());
    }

    #[test]
    fn reset_clears_offset() {
        let mut bump = Bump::with_capacity(16);
        let _ = bump.alloc(42_u64);
        assert!(bump.allocated_bytes() > 0);
        bump.reset();
        assert_eq!(bump.allocated_bytes(), 0);
        let _ = bump.alloc(7_u64);
        assert!(bump.allocated_bytes() > 0);
    }

    #[test]
    fn alignment_is_respected() {
        let bump = Bump::with_capacity(64);
        let _ = bump.alloc(1_u8);
        let aligned = bump.alloc(0xdead_beef_u32);
        let addr = aligned as *const u32 as usize;
        assert_eq!(addr % core::mem::align_of::<u32>(), 0);
    }

    #[test]
    fn zst_alloc_does_not_advance_offset() {
        let bump = Bump::with_capacity(8);
        let before = bump.allocated_bytes();
        let _: &mut () = bump.alloc(());
        assert_eq!(bump.allocated_bytes(), before);
    }

    #[test]
    fn empty_bump_only_satisfies_zsts() {
        let bump = Bump::new();
        assert_eq!(bump.chunk_capacity(), 0);
        let _: &mut () = bump.alloc(());
        assert!(bump.try_alloc(1_u8).is_err());
    }
}

//! Bump arena for short-lived scratch allocations.
//!
//! [`Bump`] is a linear allocator backed by a linked list of chunks.
//! Allocations are O(1) (bump a pointer, write the value); the entire
//! arena clears in O(1) via [`Bump::reset`]. Multiple references handed
//! out from a shared `&self` borrow coexist safely because each allocation
//! hands out a disjoint slice of the chunk.
//!
//! When a chunk is full the arena allocates a new chunk and continues —
//! [`Bump::alloc`] is effectively infallible. The original chunk is
//! retained so subsequent allocations after a [`Bump::reset`] refill the
//! existing memory before any new chunk is requested from the global
//! allocator.
//!
//! # Cost summary
//!
//! - `alloc` / `try_alloc`: O(1) (pointer bump + write; amortised over
//!   chunk growth).
//! - `reset`: O(1) (resets the chunk cursor; does **not** drop values).
//! - `allocated_bytes` / `chunk_capacity` / `chunk_count`: O(n) in the
//!   number of chunks (typically tiny).
//!
//! # Drop behavior
//!
//! `Bump` does not run destructors when reset or dropped. Allocate types
//! that do not require `Drop` (anything that is `Copy`, or owns only
//! arena-internal memory). For payloads that own resources (heap
//! allocations, file handles, etc.) use [`DropArena`](crate::drop_arena::DropArena)
//! instead — it has the same alloc-from-`&self` ergonomics but runs
//! destructors on drop.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::ptr::NonNull;

use crate::error::{Error, Result};

/// Default minimum chunk size, used when [`Bump::new`] is called or when
/// the user-supplied capacity is smaller than this value.
const DEFAULT_CHUNK_SIZE: usize = 4096;

/// Multi-chunk bump arena. See the [module-level docs](self).
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
/// // Resetting clears the cursor; chunks are retained for reuse.
/// bump.reset();
/// assert_eq!(bump.allocated_bytes(), 0);
/// assert!(bump.chunk_capacity() >= 64);
/// ```
pub struct Bump {
    state: UnsafeCell<State>,
    next_chunk_size: usize,
}

struct State {
    chunks: Vec<Box<[u8]>>,
    current_chunk: usize,
    offset: usize,
}

impl Bump {
    /// Creates an empty bump arena. The first allocation triggers the
    /// allocation of an initial chunk (default size 4 KiB).
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: UnsafeCell::new(State {
                chunks: Vec::new(),
                current_chunk: 0,
                offset: 0,
            }),
            next_chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }

    /// Creates a bump arena that pre-allocates an initial chunk of
    /// `capacity` bytes.
    ///
    /// Subsequent chunks (if needed) are sized to at least `capacity`
    /// bytes, with a floor of 4 KiB.
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let chunk_size = core::cmp::max(capacity, 1);
        let buf: Vec<u8> = vec![0; chunk_size];
        Self {
            state: UnsafeCell::new(State {
                chunks: alloc::vec![buf.into_boxed_slice()],
                current_chunk: 0,
                offset: 0,
            }),
            next_chunk_size: core::cmp::max(capacity, DEFAULT_CHUNK_SIZE),
        }
    }

    /// Total bytes reserved across every chunk currently held by the arena.
    #[inline]
    #[must_use]
    pub fn chunk_capacity(&self) -> usize {
        // SAFETY: `&self` read of `state` is sound — the only writers are
        // allocation paths (advancing `offset` / pushing a chunk through
        // `&self`) and `reset` (which takes `&mut self`, excluding any
        // concurrent `&self` access). `Bump` is not `Sync`, so cross-thread
        // `&self` access is rejected at compile time.
        let state = unsafe { &*self.state.get() };
        state.chunks.iter().map(|c| c.len()).sum()
    }

    /// Number of chunks currently held.
    ///
    /// Starts at 0 for a [`Bump::new`] arena, and grows by one each time
    /// an allocation forces a new chunk to be requested from the global
    /// allocator. [`Bump::reset`] does not reduce this count.
    #[inline]
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        // SAFETY: see `chunk_capacity`.
        let state = unsafe { &*self.state.get() };
        state.chunks.len()
    }

    /// Bytes consumed since the most recent [`Bump::reset`] (or since
    /// construction).
    ///
    /// Counts the fully-used capacity of every chunk before
    /// `current_chunk` plus the cursor within `current_chunk`. Alignment
    /// padding is included.
    #[inline]
    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        // SAFETY: see `chunk_capacity`.
        let state = unsafe { &*self.state.get() };
        if state.chunks.is_empty() {
            return 0;
        }
        let mut total = 0;
        for i in 0..state.current_chunk {
            if let Some(c) = state.chunks.get(i) {
                total += c.len();
            }
        }
        total + state.offset
    }

    /// Allocates `value` and returns a unique reference to it.
    ///
    /// Allocates a new chunk if the current chunk does not have enough
    /// space. Panics only if the global allocator itself fails (same
    /// failure model as `Vec::push` / `Box::new`).
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
            Err(_) => panic!("bump arena failed to grow (global allocator out of memory)"),
        }
    }

    /// Allocates `value`, returning `Ok(&mut T)` on success or
    /// [`Error::CapacityExceeded`] only if a new chunk could not be
    /// allocated (effectively never, on systems with a working global
    /// allocator).
    ///
    /// The returned `&mut T` borrows from `&self` because the [`Bump`]
    /// owns the underlying chunks and hands out non-overlapping regions
    /// per call — the same pattern used by `bumpalo::Bump::alloc`.
    ///
    /// # Examples
    ///
    /// ```
    /// use arena_lib::Bump;
    ///
    /// let bump = Bump::with_capacity(16);
    /// let r = bump.try_alloc(0xfeed_u32).expect("global allocator should be live");
    /// assert_eq!(*r, 0xfeed);
    /// ```
    #[allow(
        clippy::mut_from_ref,
        reason = "interior mutability via UnsafeCell; each call returns a disjoint region of the chunk"
    )]
    pub fn try_alloc<T>(&self, value: T) -> Result<&mut T> {
        let layout = Layout::new::<T>();
        let raw = self.try_alloc_layout(layout)?;
        let typed = raw.cast::<T>();
        // SAFETY: `typed` is non-null, aligned for `T`, and points into a
        // freshly reserved disjoint region of a chunk. Writing `value`
        // initialises the slot before any read.
        unsafe { core::ptr::write(typed.as_ptr(), value) };
        // SAFETY: lifetime is tied to `&self`. Chunks are `Box<[u8]>`
        // values owned by `self`; their addresses are stable for the
        // lifetime of `&self`. `try_alloc_layout` guarantees no future
        // allocation reuses this region until `reset` is called (which
        // requires `&mut self`, invalidating all outstanding `&self`
        // borrows including this returned reference).
        Ok(unsafe { &mut *typed.as_ptr() })
    }

    /// Resets the arena, marking every prior allocation as discarded.
    ///
    /// Every chunk is retained — subsequent allocations refill chunk 0
    /// first, then chunk 1, etc., before any new chunk is requested from
    /// the global allocator. Destructors are **not** run (see the
    /// [module-level docs](self) for the Drop policy). Taking `&mut self`
    /// ensures no outstanding references survive across the call.
    ///
    /// # Examples
    ///
    /// ```
    /// use arena_lib::Bump;
    ///
    /// let mut bump = Bump::with_capacity(64);
    /// let _ = bump.alloc([0_u8; 32]);
    /// let chunks_before = bump.chunk_count();
    ///
    /// bump.reset();
    /// assert_eq!(bump.allocated_bytes(), 0);
    /// assert_eq!(bump.chunk_count(), chunks_before, "reset retains chunks");
    /// ```
    #[inline]
    pub fn reset(&mut self) {
        // SAFETY: `&mut self` excludes any outstanding `&T` / `&mut T`
        // produced by `try_alloc`, so resetting the cursor cannot dangle
        // any live reference.
        let state = unsafe { &mut *self.state.get() };
        state.current_chunk = 0;
        state.offset = 0;
    }

    fn try_alloc_layout(&self, layout: Layout) -> Result<NonNull<u8>> {
        // SAFETY: see `chunk_capacity` — `&self` access to `state` is
        // sound because `Bump` is not `Sync` and the only `&mut self`
        // operation (`reset`) excludes concurrent `&self` callers.
        let state = unsafe { &mut *self.state.get() };

        // Try the current chunk first.
        if let Some(ptr) = try_in_chunk(state, layout) {
            return Ok(ptr);
        }

        // Walk forward through any retained chunks (typical after reset).
        while state.current_chunk + 1 < state.chunks.len() {
            state.current_chunk += 1;
            state.offset = 0;
            if let Some(ptr) = try_in_chunk(state, layout) {
                return Ok(ptr);
            }
        }

        // No retained chunk fits — allocate a new one.
        let min_for_layout = layout
            .size()
            .checked_add(layout.align())
            .ok_or(Error::CapacityExceeded)?;
        let new_size = core::cmp::max(self.next_chunk_size, min_for_layout);
        let buf: Vec<u8> = vec![0; new_size];
        state.chunks.push(buf.into_boxed_slice());
        state.current_chunk = state.chunks.len().saturating_sub(1);
        state.offset = 0;

        try_in_chunk(state, layout).ok_or(Error::CapacityExceeded)
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
            .field("chunk_count", &self.chunk_count())
            .finish()
    }
}

fn try_in_chunk(state: &mut State, layout: Layout) -> Option<NonNull<u8>> {
    let chunk = state.chunks.get(state.current_chunk)?;
    let base = chunk.as_ptr() as usize;
    let cursor = base.checked_add(state.offset)?;
    let align_mask = layout.align().wrapping_sub(1);
    let aligned = cursor.checked_add(align_mask).map(|a| a & !align_mask)?;
    let end = aligned.checked_add(layout.size())?;
    let limit = base.saturating_add(chunk.len());
    if end > limit {
        return None;
    }
    state.offset = end - base;

    if layout.size() == 0 {
        return Some(dangling_aligned(layout.align()));
    }

    NonNull::new(aligned as *mut u8)
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
    fn alloc_grows_to_new_chunk_when_current_is_full() {
        let bump = Bump::with_capacity(8);
        // Fill the initial chunk.
        let _ = bump.alloc([0_u8; 8]);
        let chunks_before = bump.chunk_count();
        // This allocation forces a new chunk.
        let n = bump.alloc(0xfeed_u32);
        let chunks_after = bump.chunk_count();
        assert_eq!(*n, 0xfeed);
        assert!(
            chunks_after > chunks_before,
            "should have grown to a new chunk"
        );
        assert!(bump.chunk_capacity() > 8);
    }

    #[test]
    fn try_alloc_succeeds_via_growth() {
        let bump = Bump::with_capacity(4);
        let _ = bump.alloc(1_u32); // fills chunk 0
        // try_alloc would have failed in 0.2; with multi-chunk it grows.
        assert!(bump.try_alloc(2_u32).is_ok());
    }

    #[test]
    fn reset_clears_cursor_and_retains_chunks() {
        let mut bump = Bump::with_capacity(8);
        let _ = bump.alloc([0_u8; 8]);
        let _ = bump.alloc(42_u64); // grows
        let chunks_before_reset = bump.chunk_count();
        assert!(bump.allocated_bytes() > 0);

        bump.reset();
        assert_eq!(bump.allocated_bytes(), 0);
        assert_eq!(
            bump.chunk_count(),
            chunks_before_reset,
            "reset must retain chunks for reuse"
        );

        // New allocations refill existing chunks rather than allocating fresh.
        let _ = bump.alloc(7_u64);
        assert_eq!(bump.chunk_count(), chunks_before_reset);
    }

    #[test]
    fn alignment_is_respected_across_chunks() {
        let bump = Bump::with_capacity(64);
        let _ = bump.alloc(1_u8);
        let aligned = bump.alloc(0xdead_beef_u32);
        let addr = aligned as *const u32 as usize;
        assert_eq!(addr % core::mem::align_of::<u32>(), 0);

        // Force a chunk transition and verify alignment again.
        let _ = bump.alloc([0_u8; 200]);
        let aligned2 = bump.alloc(0xcafe_u64);
        let addr2 = aligned2 as *const u64 as usize;
        assert_eq!(addr2 % core::mem::align_of::<u64>(), 0);
    }

    #[test]
    fn zst_alloc_does_not_advance_offset() {
        let bump = Bump::with_capacity(8);
        let before = bump.allocated_bytes();
        let _: &mut () = bump.alloc(());
        assert_eq!(bump.allocated_bytes(), before);
    }

    #[test]
    fn empty_bump_grows_on_first_real_alloc() {
        let bump = Bump::new();
        assert_eq!(bump.chunk_capacity(), 0);
        let n = bump.alloc(99_u32);
        assert_eq!(*n, 99);
        assert!(bump.chunk_capacity() >= 4096);
    }

    #[test]
    fn very_large_single_alloc_triggers_oversized_chunk() {
        let bump = Bump::with_capacity(16);
        // Force a chunk larger than the default.
        let big: &mut [u8; 8192] = bump.alloc([0_u8; 8192]);
        assert_eq!(big.len(), 8192);
        assert!(bump.chunk_capacity() >= 8192);
    }
}

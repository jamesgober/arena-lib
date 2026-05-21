//! Typed arena that *does* run destructors.
//!
//! [`DropArena<T>`] is the drop-honouring sibling of [`Bump`](crate::bump::Bump).
//! Both hand out `&mut T` from a shared `&self` borrow at O(1) cost, but
//! `DropArena` parks every value inside a `Vec<T>` chunk so that `T`'s
//! destructor runs when the arena (or the chunk holding the value) is
//! dropped. Reach for `DropArena` when payloads own resources — boxed
//! data, file handles, mutexes — and you cannot leak on reset.
//!
//! Like `Bump`, the arena is single-threaded: it is `Send` when `T: Send`
//! but never `Sync`.
//!
//! # Cost summary
//!
//! - `alloc`: amortised O(1) (chunk grows by allocating a new chunk).
//! - `len` / `is_empty` / `chunk_count`: O(chunk_count) (typically tiny).
//!
//! # Examples
//!
//! ```
//! use arena_lib::DropArena;
//!
//! let arena = DropArena::<String>::new();
//! let s1 = arena.alloc(String::from("alpha"));
//! let s2 = arena.alloc(String::from("bravo"));
//! assert_eq!(s1, "alpha");
//! assert_eq!(s2, "bravo");
//! assert_eq!(arena.len(), 2);
//! // When `arena` is dropped, both Strings free their heap buffers.
//! ```

use alloc::vec::Vec;
use core::cell::UnsafeCell;

/// Default capacity (in `T` slots) of the first chunk and any
/// subsequently-grown chunk.
const DEFAULT_CHUNK_CAPACITY: usize = 16;

/// Typed drop-honouring arena. See the [module-level docs](self).
pub struct DropArena<T> {
    chunks: UnsafeCell<Vec<Vec<T>>>,
    chunk_capacity: usize,
}

impl<T> DropArena<T> {
    /// Creates an empty arena. The first allocation triggers the
    /// allocation of an initial chunk holding 16 `T` slots.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            chunks: UnsafeCell::new(Vec::new()),
            chunk_capacity: DEFAULT_CHUNK_CAPACITY,
        }
    }

    /// Creates an empty arena whose chunks each hold `chunk_capacity`
    /// `T` slots.
    ///
    /// A `chunk_capacity` of zero is silently clamped to 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use arena_lib::DropArena;
    ///
    /// // Tightly-sized chunks — every alloc beyond the 4th opens a new chunk.
    /// let arena = DropArena::<u32>::with_chunk_capacity(4);
    /// for i in 0..4 {
    ///     let _ = arena.alloc(i);
    /// }
    /// assert_eq!(arena.chunk_count(), 1);
    /// let _ = arena.alloc(99);
    /// assert_eq!(arena.chunk_count(), 2);
    /// ```
    #[inline]
    #[must_use]
    pub fn with_chunk_capacity(chunk_capacity: usize) -> Self {
        Self {
            chunks: UnsafeCell::new(Vec::new()),
            chunk_capacity: core::cmp::max(chunk_capacity, 1),
        }
    }

    /// Total number of `T` values currently held across every chunk.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        // SAFETY: `&self` read of `chunks` is sound because `DropArena`
        // is not `Sync` and the only `&mut self` operations exclude
        // concurrent `&self` access.
        let chunks = unsafe { &*self.chunks.get() };
        chunks.iter().map(Vec::len).sum()
    }

    /// Returns `true` when the arena holds no values.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of chunks currently held.
    #[inline]
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        // SAFETY: see `len`.
        let chunks = unsafe { &*self.chunks.get() };
        chunks.len()
    }

    /// Allocates `value` and returns a unique reference to it.
    ///
    /// The value is moved into a chunk; its destructor will run when the
    /// arena is dropped. Panics only if the global allocator itself fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use arena_lib::DropArena;
    ///
    /// let arena = DropArena::<String>::new();
    /// let s = arena.alloc(String::from("owned"));
    /// assert_eq!(s.as_str(), "owned");
    /// ```
    #[allow(
        clippy::mut_from_ref,
        reason = "interior mutability via UnsafeCell; each call returns a disjoint slot in a chunk"
    )]
    pub fn alloc(&self, value: T) -> &mut T {
        // SAFETY: `&self` access to `chunks` is sound because `DropArena`
        // is not `Sync` and `&mut self` operations exclude concurrent
        // `&self` access. We use the borrow only to push a new chunk if
        // needed and to write into the current chunk's spare capacity.
        let chunks = unsafe { &mut *self.chunks.get() };

        let needs_new_chunk = match chunks.last() {
            None => true,
            Some(c) => c.len() == c.capacity(),
        };
        if needs_new_chunk {
            chunks.push(Vec::with_capacity(self.chunk_capacity));
        }

        let current = match chunks.last_mut() {
            Some(c) => c,
            None => panic!("drop arena chunk invariant violated"),
        };

        let len = current.len();
        debug_assert!(len < current.capacity());

        // SAFETY: `len < capacity` was just ensured, so adding `len` to
        // the chunk's base pointer yields an in-bounds pointer into the
        // reserved (but uninitialised) tail of the chunk's buffer.
        let slot_ptr: *mut T = unsafe { current.as_mut_ptr().add(len) };
        // SAFETY: `slot_ptr` is properly aligned for `T` (the chunk is a
        // `Vec<T>`), points into reserved capacity, and is exclusive.
        unsafe { core::ptr::write(slot_ptr, value) };
        // SAFETY: we just initialised the slot at index `len`; growing
        // `len` by one is sound (the chunk now logically owns that slot).
        unsafe { current.set_len(len + 1) };

        // SAFETY: `slot_ptr` is valid for `&mut T` for the lifetime of
        // `&self`:
        //   - The chunk's `Vec<T>` buffer is heap-allocated and its
        //     address is stable until the chunk itself is dropped.
        //   - The outer `Vec<Vec<T>>` may reallocate when chunks are
        //     pushed, but moving a `Vec<T>` does not move its underlying
        //     buffer; the slot's address is preserved.
        //   - We never grow an existing chunk (we always push a fresh
        //     one), so the chunk's buffer is never reallocated, only the
        //     `len` advances.
        //   - The borrow is tied to `&self`; `&mut self` operations
        //     (`reset` is not provided; the arena clears on `Drop` only)
        //     would invalidate it via the borrow checker.
        unsafe { &mut *slot_ptr }
    }
}

impl<T> Default for DropArena<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for DropArena<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DropArena")
            .field("len", &self.len())
            .field("chunk_count", &self.chunk_count())
            .field("chunk_capacity", &self.chunk_capacity)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::sync::Arc;

    #[test]
    fn alloc_returns_unique_references() {
        let arena = DropArena::<u32>::new();
        let a = arena.alloc(1);
        let b = arena.alloc(2);
        assert_eq!(*a, 1);
        assert_eq!(*b, 2);
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn grows_to_new_chunk_when_current_is_full() {
        let arena = DropArena::<u32>::with_chunk_capacity(4);
        for i in 0..4 {
            let _ = arena.alloc(i);
        }
        assert_eq!(arena.chunk_count(), 1);
        let _ = arena.alloc(99); // forces a new chunk
        assert_eq!(arena.chunk_count(), 2);
        assert_eq!(arena.len(), 5);
    }

    #[test]
    fn destructors_run_on_drop() {
        let shared = Arc::new(0_u32);
        {
            let arena = DropArena::<Arc<u32>>::new();
            let _ = arena.alloc(Arc::clone(&shared));
            let _ = arena.alloc(Arc::clone(&shared));
            assert_eq!(Arc::strong_count(&shared), 3);
        }
        assert_eq!(Arc::strong_count(&shared), 1);
    }

    #[test]
    fn mutating_returned_reference_is_visible() {
        let arena = DropArena::<String>::new();
        let s = arena.alloc(String::from("hello"));
        s.push_str(", world");
        assert_eq!(s, "hello, world");
    }

    #[test]
    fn references_remain_valid_across_chunk_growth() {
        let arena = DropArena::<u32>::with_chunk_capacity(2);
        let first = arena.alloc(100);
        let _ = arena.alloc(200);
        // Force chunk growth — first should still be valid.
        let _ = arena.alloc(300);
        let _ = arena.alloc(400);
        let _ = arena.alloc(500);
        assert_eq!(*first, 100);
    }

    #[test]
    fn with_chunk_capacity_clamps_zero_to_one() {
        let arena = DropArena::<u8>::with_chunk_capacity(0);
        let _ = arena.alloc(1);
        let _ = arena.alloc(2);
        assert_eq!(arena.len(), 2);
        // Should have grown to at least one chunk per value because cap=1.
        assert_eq!(arena.chunk_count(), 2);
    }
}

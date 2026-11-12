//! Bump arena for short-lived scratch allocations.
//!
//! [`Bump`] is a linear allocator backed by a list of raw chunks.
//! Allocations are O(1) (bump a pointer, write the value); the entire
//! arena clears in O(1) via [`Bump::reset`]. Multiple references handed
//! out from a shared `&self` borrow coexist safely because each allocation
//! hands out a disjoint slice of the chunk.
//!
//! When a chunk is full the arena allocates a new chunk, twice the size of
//! the previous one up to a cap, and continues. [`Bump::alloc`] panics only
//! if that chunk cannot be allocated; [`Bump::try_alloc`] returns an error
//! instead. Zero-sized values never touch a chunk. The original chunk is
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

use alloc::alloc::{alloc, dealloc};
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::ptr::NonNull;

use crate::error::{Error, Result};

/// Default minimum chunk size, used when [`Bump::new`] is called or when
/// the user-supplied capacity is smaller than this value.
const DEFAULT_CHUNK_SIZE: usize = 4096;

/// Largest size the automatic chunk growth doubles up to. Chunks grow
/// geometrically from the starting size until they reach this value (or the
/// starting size, if that is larger); a single oversized request still gets
/// a chunk big enough to hold it.
const MAX_GROWN_CHUNK_SIZE: usize = 1 << 20;

/// Alignment of every chunk's base address. Requests with a larger
/// alignment are satisfied by padding inside the chunk.
const CHUNK_ALIGN: usize = 16;

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
}

// SAFETY: a `Bump` owns its chunks outright (each `Chunk` is a unique
// allocation freed only in its `Drop`), and moving the arena to another
// thread requires that no borrow of it is outstanding, so no reference
// into a chunk can be used from two threads. `Bump` stays `!Sync` through
// the `UnsafeCell`.
unsafe impl Send for Bump {}

struct State {
    chunks: Vec<Chunk>,
    current_chunk: usize,
    offset: usize,
    next_chunk_size: usize,
    growth_cap: usize,
}

/// One raw chunk of arena memory.
///
/// The chunk is held as a raw pointer from the global allocator, never as a
/// `Box<[u8]>` or slice, so every pointer handed out by the arena carries
/// the allocation's full write provenance, and no shared or unique borrow
/// of the whole chunk is ever created while results are outstanding.
struct Chunk {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl Chunk {
    /// Allocates a chunk of at least one byte. Returns
    /// [`Error::CapacityExceeded`] when the size is not a valid layout or
    /// the global allocator reports failure.
    fn try_new(size: usize) -> Result<Self> {
        let size = core::cmp::max(size, 1);
        let layout =
            Layout::from_size_align(size, CHUNK_ALIGN).map_err(|_| Error::CapacityExceeded)?;
        // SAFETY: `layout` has a non-zero size.
        let raw = unsafe { alloc(layout) };
        let ptr = NonNull::new(raw).ok_or(Error::CapacityExceeded)?;
        Ok(Self { ptr, layout })
    }

    #[inline]
    fn len(&self) -> usize {
        self.layout.size()
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        // SAFETY: `ptr` was returned by `alloc` with exactly `layout` and is
        // freed only here, once.
        unsafe { dealloc(self.ptr.as_ptr(), self.layout) };
    }
}

impl Bump {
    /// Creates an empty bump arena. The first allocation triggers the
    /// allocation of an initial chunk (default size 4 KiB).
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::from_parts(Vec::new(), DEFAULT_CHUNK_SIZE)
    }

    /// Creates a bump arena that pre-allocates an initial chunk of
    /// `capacity` bytes.
    ///
    /// Subsequent chunks (if needed) are sized to at least `capacity`
    /// bytes, with a floor of 4 KiB, and double in size as the arena grows
    /// (up to 1 MiB per chunk, or `capacity` if that is larger).
    ///
    /// # Panics
    ///
    /// Panics if the initial chunk cannot be allocated. Use
    /// [`Bump::try_with_capacity`] to handle that case.
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        match Self::try_with_capacity(capacity) {
            Ok(bump) => bump,
            Err(_) => panic!("bump arena could not allocate its initial chunk"),
        }
    }

    /// Fallible form of [`Bump::with_capacity`]: returns
    /// [`Error::CapacityExceeded`] instead of panicking when the initial
    /// chunk cannot be allocated (a `capacity` larger than `isize::MAX`, or
    /// a global allocator that reports failure).
    ///
    /// # Examples
    ///
    /// ```
    /// use arena_lib::{Bump, Error};
    ///
    /// assert!(Bump::try_with_capacity(64).is_ok());
    /// assert_eq!(
    ///     Bump::try_with_capacity(usize::MAX).err(),
    ///     Some(Error::CapacityExceeded)
    /// );
    /// ```
    pub fn try_with_capacity(capacity: usize) -> Result<Self> {
        let mut chunks = Vec::new();
        chunks
            .try_reserve_exact(1)
            .map_err(|_| Error::CapacityExceeded)?;
        chunks.push(Chunk::try_new(capacity)?);
        Ok(Self::from_parts(
            chunks,
            core::cmp::max(capacity, DEFAULT_CHUNK_SIZE),
        ))
    }

    #[inline]
    fn from_parts(chunks: Vec<Chunk>, base_chunk_size: usize) -> Self {
        Self {
            state: UnsafeCell::new(State {
                chunks,
                current_chunk: 0,
                offset: 0,
                next_chunk_size: base_chunk_size,
                growth_cap: core::cmp::max(base_chunk_size, MAX_GROWN_CHUNK_SIZE),
            }),
        }
    }

    /// Total bytes reserved across every chunk currently held by the arena.
    #[inline]
    #[must_use]
    pub fn chunk_capacity(&self) -> usize {
        // SAFETY: `&self` read of `state` is sound. The only writers are
        // allocation paths (advancing `offset` / pushing a chunk through
        // `&self`) and `reset` (which takes `&mut self`, excluding any
        // concurrent `&self` access). `Bump` is not `Sync`, so cross-thread
        // `&self` access is rejected at compile time. `State` holds chunk
        // pointers, not references to chunk bytes, so this borrow does not
        // touch memory handed out to callers.
        let state = unsafe { &*self.state.get() };
        state.chunks.iter().map(Chunk::len).sum()
    }

    /// Number of chunks currently held.
    ///
    /// Starts at 0 for a [`Bump::new`] arena, and grows by one each time
    /// an allocation forces a new chunk to be requested from the global
    /// allocator. [`Bump::reset`] does not reduce this count. Zero-sized
    /// allocations never allocate a chunk.
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
    /// space.
    ///
    /// # Panics
    ///
    /// Panics if a new chunk is needed and cannot be allocated. Use
    /// [`Bump::try_alloc`] to handle that case.
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
    /// [`Error::CapacityExceeded`] if a new chunk was needed and could not
    /// be allocated (the size does not fit a valid layout, or the global
    /// allocator reports failure). On error, `value` is dropped and the
    /// arena keeps every earlier allocation.
    ///
    /// The returned `&mut T` borrows from `&self` because the [`Bump`]
    /// owns the underlying chunks and hands out non-overlapping regions
    /// per call, the same pattern used by `bumpalo::Bump::alloc`.
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
        // SAFETY: `typed` is non-null and aligned for `T`. For a sized `T`
        // it points into a freshly reserved region of a chunk that no other
        // live reference covers, and it is derived from the chunk's
        // allocation pointer, so it carries write provenance. For a
        // zero-sized `T` it is an aligned dangling pointer, valid for
        // zero-sized writes. Writing `value` initialises the slot before
        // any read.
        unsafe { core::ptr::write(typed.as_ptr(), value) };
        // SAFETY: lifetime is tied to `&self`. Chunk allocations are owned
        // by `self` and never move or shrink while `self` is alive (pushing
        // to `chunks` moves only the `Chunk` handles, not the memory).
        // `try_alloc_layout` guarantees no future allocation reuses this
        // region until `reset` is called (which requires `&mut self`,
        // invalidating all outstanding `&self` borrows including this
        // returned reference).
        Ok(unsafe { &mut *typed.as_ptr() })
    }

    /// Resets the arena, marking every prior allocation as discarded.
    ///
    /// Every chunk is retained: subsequent allocations refill chunk 0
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
        let state = self.state.get_mut();
        state.current_chunk = 0;
        state.offset = 0;
    }

    fn try_alloc_layout(&self, layout: Layout) -> Result<NonNull<u8>> {
        // Zero-sized requests never touch a chunk.
        if layout.size() == 0 {
            return Ok(dangling_aligned(layout));
        }

        // SAFETY: see `chunk_capacity`. `&self` access to `state` is sound
        // because `Bump` is not `Sync`, the only `&mut self` operation
        // (`reset`) excludes concurrent `&self` callers, and no other
        // borrow of `state` is live during this call.
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

        // No retained chunk fits, so allocate a new one. `size + align` is
        // always enough room, whatever the padding at the chunk start.
        let min_for_layout = layout
            .size()
            .checked_add(layout.align())
            .ok_or(Error::CapacityExceeded)?;
        let new_size = core::cmp::max(state.next_chunk_size, min_for_layout);
        state
            .chunks
            .try_reserve(1)
            .map_err(|_| Error::CapacityExceeded)?;
        let chunk = Chunk::try_new(new_size)?;
        state.chunks.push(chunk);
        state.current_chunk = state.chunks.len() - 1;
        state.offset = 0;
        state.next_chunk_size =
            core::cmp::min(state.next_chunk_size.saturating_mul(2), state.growth_cap);

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

/// Reserves `layout` (non-zero size) in the current chunk and returns a
/// pointer derived from the chunk's allocation pointer.
///
/// Alignment padding is computed from the address alone (`addr()` exposes
/// nothing), and the result is produced with `add` on the chunk pointer, so
/// it keeps the allocation's provenance.
fn try_in_chunk(state: &mut State, layout: Layout) -> Option<NonNull<u8>> {
    let chunk = state.chunks.get(state.current_chunk)?;
    let base_addr = chunk.ptr.as_ptr().addr();
    let cursor = base_addr.checked_add(state.offset)?;
    let align_mask = layout.align().wrapping_sub(1);
    let aligned = cursor.checked_add(align_mask)? & !align_mask;
    let start = aligned - base_addr;
    let end = start.checked_add(layout.size())?;
    if end > chunk.len() {
        return None;
    }
    state.offset = end;
    // SAFETY: `start + layout.size() <= chunk.len()`, so `start` is within
    // the chunk allocation and the resulting pointer stays in bounds.
    Some(unsafe { chunk.ptr.add(start) })
}

/// A non-null pointer aligned for `layout`, valid for zero-sized accesses
/// only. It carries no provenance and is never dereferenced for a sized
/// value.
fn dangling_aligned(layout: Layout) -> NonNull<u8> {
    let align =
        core::num::NonZeroUsize::new(layout.align()).unwrap_or(core::num::NonZeroUsize::MIN);
    NonNull::<u8>::dangling().with_addr(align)
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
    fn zst_alloc_allocates_no_chunk() {
        #[repr(align(4096))]
        struct Z;
        let bump = Bump::new();
        let z: &mut Z = bump.alloc(Z);
        assert_eq!((z as *mut Z as usize) % 4096, 0);
        let _: &mut () = bump.alloc(());
        // 1.0.0 allocated a 4 KiB chunk for the aligned ZST.
        assert_eq!(bump.chunk_count(), 0);
        assert_eq!(bump.chunk_capacity(), 0);
    }

    #[test]
    fn new_chunks_double_up_to_the_cap() {
        let bump = Bump::new();
        let mut sizes = Vec::new();
        for _ in 0..12 {
            // Each request fills more than half of any chunk up to 4 KiB,
            // so every call below lands in a new chunk until they grow.
            let _ = bump.alloc([0_u8; 3000]);
        }
        // SAFETY: test-only read of the chunk sizes; no allocation is live
        // through this borrow.
        let state = unsafe { &*bump.state.get() };
        for c in &state.chunks {
            sizes.push(c.len());
        }
        assert_eq!(sizes[0], 4096);
        assert_eq!(sizes[1], 8192);
        assert!(sizes.windows(2).all(|w| w[1] >= w[0]));
        assert!(sizes.iter().all(|&s| s <= MAX_GROWN_CHUNK_SIZE));
        // 36 KB in 12 requests fits in far fewer than 12 chunks.
        assert!(bump.chunk_count() < 6, "chunks: {sizes:?}");
    }

    #[test]
    fn try_with_capacity_reports_impossible_size() {
        assert_eq!(
            Bump::try_with_capacity(usize::MAX).err(),
            Some(Error::CapacityExceeded)
        );
        assert_eq!(
            Bump::try_with_capacity(isize::MAX as usize + 1).err(),
            Some(Error::CapacityExceeded)
        );
        match Bump::try_with_capacity(128) {
            Ok(bump) => assert_eq!(bump.chunk_capacity(), 128),
            Err(e) => panic!("small chunk failed: {e}"),
        }
    }

    #[test]
    fn bump_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Bump>();
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

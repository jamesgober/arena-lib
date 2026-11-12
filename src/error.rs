//! Error type and result alias used across the crate.
//!
//! `arena-lib` exposes a single [`Error`] enum so callers can match on every
//! failure mode without juggling per-module error types. New variants may be
//! introduced in minor releases — match using `_ =>` to stay forward-compatible.

use core::fmt;

/// Failure modes returned by the public APIs of `arena-lib`.
///
/// The enum is `#[non_exhaustive]`: callers must include a wildcard arm when
/// matching so future variants can be added in minor releases without
/// breaking source compatibility.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The provided index does not point to a live element.
    ///
    /// Returned by [`Arena::try_get`](crate::arena::Arena::try_get),
    /// [`Arena::try_get_mut`](crate::arena::Arena::try_get_mut) and
    /// [`Arena::try_remove`](crate::arena::Arena::try_remove) when an
    /// [`Index`](crate::Index) is used after its slot has been removed and
    /// either left vacant, re-issued under a new generation, or retired, or
    /// when the slot was never issued. An `Index` is not tied to the arena
    /// that issued it, so a handle from a different arena is reported as
    /// stale only when nothing in this arena matches its slot and generation.
    StaleIndex,

    /// An allocation request could not be satisfied.
    ///
    /// Returned by [`Bump::try_alloc`](crate::bump::Bump::try_alloc) and
    /// [`Bump::try_with_capacity`](crate::bump::Bump::try_with_capacity)
    /// when a chunk large enough for the request does not fit a valid
    /// layout (more than `isize::MAX` bytes) or the global allocator
    /// reports failure.
    CapacityExceeded,

    /// A monotonic counter would have wrapped past its representable range.
    ///
    /// Returned by [`Arena::try_insert`](crate::arena::Arena::try_insert)
    /// when the arena already holds `u32::MAX` slots, and by the interner
    /// when its symbol counter reaches its upper bound. Treat this as a
    /// permanent failure: drop the affected container and build a new one.
    /// (Exhausted per-slot generation counters do not produce this error;
    /// the slot is retired and the arena keeps working.)
    CounterOverflow,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleIndex => f.write_str("stale index: handle does not refer to a live element"),
            Self::CapacityExceeded => f.write_str("capacity exceeded: allocator could not grow"),
            Self::CounterOverflow => {
                f.write_str("counter overflow: generation or symbol counter exhausted")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/// Result alias that uses [`Error`] as the failure type.
///
/// # Examples
///
/// ```
/// use arena_lib::{Error, Result};
///
/// fn checked() -> Result<u32> {
///     Err(Error::StaleIndex)
/// }
///
/// assert!(checked().is_err());
/// ```
pub type Result<T> = core::result::Result<T, Error>;

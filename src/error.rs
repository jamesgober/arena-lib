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
    /// Returned when an [`Index`](crate::Index) is used after its slot has
    /// been removed and either left vacant or re-issued under a new
    /// generation, or when the index was never live in this arena.
    StaleIndex,

    /// An allocation request exceeded the allocator's available capacity.
    ///
    /// Returned by [`Bump::try_alloc`](crate::bump::Bump::try_alloc) and
    /// related fallible allocation entry points when the underlying buffer
    /// could not be grown to satisfy the request.
    CapacityExceeded,

    /// A monotonic counter would have wrapped past its representable range.
    ///
    /// Returned by [`Arena::insert`](crate::arena::Arena::insert) and the
    /// interner when generation or symbol counters reach their upper bound.
    /// Treat this as a permanent failure: drop the affected container and
    /// build a new one.
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

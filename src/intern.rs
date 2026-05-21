//! String interner that hands out compact [`Symbol`] handles.
//!
//! [`Interner`] deduplicates owned [`String`] storage:
//! every call to [`Interner::intern`] returns the same [`Symbol`] for the
//! same input. Equality and hashing operate on the four-byte symbol rather
//! than the underlying bytes, which is the win when the same identifier
//! appears thousands of times across a workload.
//!
//! # Cost summary
//!
//! - `intern`: expected O(1) on first sight; expected O(1) on repeated sight.
//! - `resolve`: O(1).
//! - `lookup` / `contains`: expected O(1).
//! - `len` / `is_empty`: O(1).
//!
//! The implementation uses [`hashbrown::HashMap`] for the de-duplication
//! index, keeping `intern` / `lookup` at expected O(1) while still
//! compiling under `no_std`.

use alloc::string::String;
use alloc::vec::Vec;

use hashbrown::HashMap;

use crate::error::{Error, Result};

/// Compact handle returned by [`Interner::intern`].
///
/// A `Symbol` is a 4-byte `Copy` value. Two symbols compare equal if and
/// only if the strings they refer to were interned by the **same**
/// [`Interner`] from byte-identical inputs. Symbols are not transferable
/// across interners.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(u32);

impl Symbol {
    /// Returns the underlying numeric identifier.
    ///
    /// Useful for diagnostics or for serializing alongside an interner
    /// snapshot. Do not synthesize `Symbol` values; obtain them from
    /// [`Interner::intern`].
    #[inline]
    pub const fn id(self) -> u32 {
        self.0
    }
}

/// String interner. See the [module-level docs](self).
///
/// # Examples
///
/// ```
/// use arena_lib::Interner;
///
/// let mut interner = Interner::new();
/// let a = interner.intern("user:42");
/// let b = interner.intern("user:42");
/// let c = interner.intern("user:7");
///
/// assert_eq!(a, b);
/// assert_ne!(a, c);
/// assert_eq!(interner.resolve(a), Some("user:42"));
/// assert_eq!(interner.len(), 2);
/// ```
pub struct Interner {
    storage: Vec<String>,
    lookup: HashMap<String, u32>,
}

impl Interner {
    /// Creates an empty interner that performs no allocation up front.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Creates an empty interner with storage pre-reserved for `capacity`
    /// distinct strings.
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            storage: Vec::with_capacity(capacity),
            lookup: HashMap::with_capacity(capacity),
        }
    }

    /// Number of distinct strings currently interned.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Returns `true` if the interner holds no strings.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// Interns `s` and returns its [`Symbol`].
    ///
    /// Idempotent: repeated calls with the same input return the same
    /// symbol. Panics if the symbol counter would overflow `u32::MAX`
    /// — use [`Interner::try_intern`] for the explicit fallible variant.
    pub fn intern(&mut self, s: &str) -> Symbol {
        match self.try_intern(s) {
            Ok(sym) => sym,
            Err(_) => panic!("interner symbol counter overflow (u32::MAX symbols)"),
        }
    }

    /// Interns `s`, returning a [`Symbol`] on success or
    /// [`Error::CounterOverflow`] if the interner cannot represent more
    /// distinct strings.
    pub fn try_intern(&mut self, s: &str) -> Result<Symbol> {
        if let Some(&id) = self.lookup.get(s) {
            return Ok(Symbol(id));
        }
        let id_usize = self.storage.len();
        if id_usize > u32::MAX as usize {
            return Err(Error::CounterOverflow);
        }
        let id = id_usize as u32;
        let owned = String::from(s);
        self.storage.push(owned.clone());
        let _ = self.lookup.insert(owned, id);
        Ok(Symbol(id))
    }

    /// Returns the original string for `symbol`, or `None` if the symbol
    /// did not come from this interner.
    #[inline]
    #[must_use]
    pub fn resolve(&self, symbol: Symbol) -> Option<&str> {
        self.storage.get(symbol.0 as usize).map(String::as_str)
    }

    /// Returns `true` if `s` has already been interned.
    ///
    /// Equivalent to [`Interner::lookup`] returning `Some`.
    #[inline]
    #[must_use]
    pub fn contains(&self, s: &str) -> bool {
        self.lookup.contains_key(s)
    }

    /// Returns the symbol previously assigned to `s`, without inserting
    /// a new entry if the string is unknown.
    #[inline]
    #[must_use]
    pub fn lookup(&self, s: &str) -> Option<Symbol> {
        self.lookup.get(s).copied().map(Symbol)
    }

    /// Iterator over `(Symbol, &str)` pairs for every interned string,
    /// in insertion order.
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            inner: self.storage.iter().enumerate(),
        }
    }
}

impl Default for Interner {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for Interner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Interner")
            .field("len", &self.storage.len())
            .finish()
    }
}

/// Iterator over `(Symbol, &str)` pairs returned by [`Interner::iter`].
pub struct Iter<'a> {
    inner: core::iter::Enumerate<core::slice::Iter<'a, String>>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (Symbol, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let (i, s) = self.inner.next()?;
        Some((Symbol(i as u32), s.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_string_returns_same_symbol() {
        let mut i = Interner::new();
        let a = i.intern("hello");
        let b = i.intern("hello");
        assert_eq!(a, b);
        assert_eq!(i.len(), 1);
    }

    #[test]
    fn distinct_strings_return_distinct_symbols() {
        let mut i = Interner::new();
        let a = i.intern("alpha");
        let b = i.intern("bravo");
        assert_ne!(a, b);
    }

    #[test]
    fn resolve_round_trips() {
        let mut i = Interner::new();
        let s = i.intern("round-trip");
        assert_eq!(i.resolve(s), Some("round-trip"));
    }

    #[test]
    fn lookup_does_not_insert() {
        let mut i = Interner::new();
        let _ = i.intern("first");
        assert!(i.lookup("second").is_none());
        assert_eq!(i.len(), 1);
    }

    #[test]
    fn contains_reflects_state() {
        let mut i = Interner::new();
        assert!(!i.contains("x"));
        let _ = i.intern("x");
        assert!(i.contains("x"));
    }

    #[test]
    fn iter_yields_insertion_order() {
        let mut i = Interner::new();
        let _ = i.intern("a");
        let _ = i.intern("b");
        let _ = i.intern("c");
        let collected: Vec<&str> = i.iter().map(|(_, s)| s).collect();
        assert_eq!(collected, vec!["a", "b", "c"]);
    }
}

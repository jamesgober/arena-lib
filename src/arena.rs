//! Generational arena and its [`Index`] handle.
//!
//! [`Arena<T>`] is a slab-style container that hands out stable [`Index`]
//! handles. When a slot is freed, its generation counter advances, so any
//! still-held handle pointing at that slot is detected and rejected — no
//! use-after-free, no dangling references, all in safe Rust.
//!
//! Generations are 32-bit. A slot whose generation counter is exhausted
//! (after about 4 billion reuses of that one slot) is retired instead of
//! wrapping: it is never handed out again, so an old handle can never
//! resolve to a newer element. A retired slot keeps its storage until the
//! arena is dropped.
//!
//! An [`Index`] is not tied to the arena that issued it. Using a handle
//! from one arena on another resolves whatever the second arena holds at
//! that slot and generation, if anything; keep each handle with its arena.
//!
//! # Cost summary
//!
//! - `insert`: amortized O(1).
//! - `remove`: O(1).
//! - `get` / `get_mut` / `contains`: O(1).
//! - `iter` / `iter_mut`: O(capacity) — skips vacant slots.

use alloc::vec::Vec;

use crate::error::{Error, Result};

/// Stable handle into an [`Arena`].
///
/// An `Index` is `Copy`, cheap to pass by value, and remains valid until the
/// element it points at is removed. Once removed, the handle becomes stale
/// and all lookups against the arena return `None` (or [`Error::StaleIndex`]
/// from the fallible variants such as [`Arena::try_get`]). Reusing the
/// underlying slot under a new generation does **not** revive the original
/// handle, and a slot whose generation counter is exhausted is retired
/// rather than wrapped.
///
/// An `Index` is not scoped to the arena that issued it; see the
/// [module-level docs](self).
///
/// # Layout
///
/// `Index` is 8 bytes (two `u32`s) and packs trivially into structs.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Index {
    generation: u32,
    slot: u32,
}

impl Index {
    /// Returns the slot number this handle points at.
    ///
    /// Useful for diagnostics; do not use the slot number as a substitute for
    /// the handle itself — slots are reused under new generations.
    #[inline]
    pub const fn slot(self) -> u32 {
        self.slot
    }

    /// Returns the generation counter recorded when this handle was issued.
    #[inline]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

enum Occupant<T> {
    Occupied(T),
    Vacant { next_free: Option<u32> },
}

struct Slot<T> {
    /// Generation of the current (or next) occupant. Starts at 1 and only
    /// grows. [`RETIRED`] marks a vacant slot whose counter is exhausted;
    /// such a slot is never linked into the free list again.
    generation: u32,
    occupant: Occupant<T>,
}

/// Generation value of a retired slot. No handle ever carries it, because
/// live generations start at 1 and never wrap.
const RETIRED: u32 = 0;

/// Generational arena. See the [module-level docs](self).
///
/// # Examples
///
/// ```
/// use arena_lib::Arena;
///
/// let mut arena = Arena::new();
/// let a = arena.insert("alpha");
/// let b = arena.insert("bravo");
///
/// assert_eq!(arena.len(), 2);
/// assert_eq!(arena.get(a), Some(&"alpha"));
///
/// let removed = arena.remove(a);
/// assert_eq!(removed, Some("alpha"));
/// assert!(arena.get(a).is_none()); // stale handle
/// assert_eq!(arena.get(b), Some(&"bravo"));
/// ```
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free_head: Option<u32>,
    len: usize,
}

impl<T> Arena<T> {
    /// Creates an empty arena that performs no allocation until first insert.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_head: None,
            len: 0,
        }
    }

    /// Creates an empty arena with space pre-reserved for `capacity` elements.
    ///
    /// The arena will still grow on demand; this is a hint, not a hard cap.
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            free_head: None,
            len: 0,
        }
    }

    /// Reserves capacity for at least `additional` more elements.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.slots.reserve(additional);
    }

    /// Number of live elements currently in the arena.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the arena holds no live elements.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of slots the arena can hold before reallocating.
    ///
    /// This counts both occupied and vacant slots.
    #[inline]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.slots.capacity()
    }

    /// Returns `true` if the index refers to a live element.
    #[inline]
    #[must_use]
    pub fn contains(&self, idx: Index) -> bool {
        self.get(idx).is_some()
    }

    /// Returns a shared reference to the element behind `idx`, or `None`
    /// if the handle is stale.
    #[inline]
    #[must_use]
    pub fn get(&self, idx: Index) -> Option<&T> {
        let slot = self.slots.get(idx.slot as usize)?;
        if slot.generation != idx.generation {
            return None;
        }
        match &slot.occupant {
            Occupant::Occupied(value) => Some(value),
            Occupant::Vacant { .. } => None,
        }
    }

    /// Returns a unique reference to the element behind `idx`, or `None`
    /// if the handle is stale.
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, idx: Index) -> Option<&mut T> {
        let slot = self.slots.get_mut(idx.slot as usize)?;
        if slot.generation != idx.generation {
            return None;
        }
        match &mut slot.occupant {
            Occupant::Occupied(value) => Some(value),
            Occupant::Vacant { .. } => None,
        }
    }

    /// Returns a shared reference to the element behind `idx`, or
    /// [`Error::StaleIndex`] if the handle does not refer to a live element.
    ///
    /// # Examples
    ///
    /// ```
    /// use arena_lib::{Arena, Error};
    ///
    /// let mut arena = Arena::new();
    /// let h = arena.insert(5_u32);
    /// assert_eq!(arena.try_get(h), Ok(&5));
    /// arena.remove(h);
    /// assert_eq!(arena.try_get(h), Err(Error::StaleIndex));
    /// ```
    #[inline]
    pub fn try_get(&self, idx: Index) -> Result<&T> {
        self.get(idx).ok_or(Error::StaleIndex)
    }

    /// Returns a unique reference to the element behind `idx`, or
    /// [`Error::StaleIndex`] if the handle does not refer to a live element.
    #[inline]
    pub fn try_get_mut(&mut self, idx: Index) -> Result<&mut T> {
        self.get_mut(idx).ok_or(Error::StaleIndex)
    }

    /// Removes the element behind `idx` and returns it, or
    /// [`Error::StaleIndex`] if the handle does not refer to a live element.
    ///
    /// # Examples
    ///
    /// ```
    /// use arena_lib::{Arena, Error};
    ///
    /// let mut arena = Arena::new();
    /// let h = arena.insert("payload");
    /// assert_eq!(arena.try_remove(h), Ok("payload"));
    /// assert_eq!(arena.try_remove(h), Err(Error::StaleIndex));
    /// ```
    #[inline]
    pub fn try_remove(&mut self, idx: Index) -> Result<T> {
        self.remove(idx).ok_or(Error::StaleIndex)
    }

    /// Inserts `value` and returns a fresh [`Index`].
    ///
    /// Panics on the catastrophic case where the slot counter would overflow
    /// `u32::MAX`. Use [`Arena::try_insert`] for an explicit fallible variant.
    #[inline]
    pub fn insert(&mut self, value: T) -> Index {
        match self.try_insert(value) {
            Ok(idx) => idx,
            Err(_) => panic!("arena slot counter overflow (u32::MAX slots)"),
        }
    }

    /// Inserts `value`, returning an [`Index`] on success or
    /// [`Error::CounterOverflow`] if the arena cannot represent more slots.
    pub fn try_insert(&mut self, value: T) -> Result<Index> {
        if let Some(slot_id) = self.free_head {
            let slot = match self.slots.get_mut(slot_id as usize) {
                Some(s) => s,
                None => return Err(Error::CounterOverflow),
            };
            let next_free = match &slot.occupant {
                Occupant::Vacant { next_free } => *next_free,
                Occupant::Occupied(_) => return Err(Error::CounterOverflow),
            };
            self.free_head = next_free;
            slot.occupant = Occupant::Occupied(value);
            self.len += 1;
            Ok(Index {
                generation: slot.generation,
                slot: slot_id,
            })
        } else {
            let slot_idx = self.slots.len();
            if slot_idx > u32::MAX as usize {
                return Err(Error::CounterOverflow);
            }
            self.slots.push(Slot {
                generation: 1,
                occupant: Occupant::Occupied(value),
            });
            self.len += 1;
            Ok(Index {
                generation: 1,
                slot: slot_idx as u32,
            })
        }
    }

    /// Removes the element behind `idx` and returns it, or `None` if the
    /// handle is stale.
    ///
    /// The slot's generation counter advances so the consumed handle —
    /// and any other handle previously issued for the same slot — never
    /// resolves again, even if the slot is reused by a later `insert`. If
    /// the counter is exhausted, the slot is retired and never reused.
    ///
    /// # Examples
    ///
    /// ```
    /// use arena_lib::Arena;
    ///
    /// let mut arena = Arena::new();
    /// let h = arena.insert("payload");
    ///
    /// assert_eq!(arena.remove(h), Some("payload"));
    /// assert_eq!(arena.remove(h), None); // stale on every subsequent call
    /// ```
    pub fn remove(&mut self, idx: Index) -> Option<T> {
        let slot = self.slots.get_mut(idx.slot as usize)?;
        if slot.generation != idx.generation {
            return None;
        }
        if matches!(slot.occupant, Occupant::Vacant { .. }) {
            return None;
        }

        let prior = if let Some(next) = slot.generation.checked_add(1) {
            slot.generation = next;
            let vacated = Occupant::Vacant {
                next_free: self.free_head,
            };
            self.free_head = Some(idx.slot);
            core::mem::replace(&mut slot.occupant, vacated)
        } else {
            // Generation exhausted: retire the slot instead of wrapping, so
            // no handle issued for it can ever resolve again.
            slot.generation = RETIRED;
            core::mem::replace(&mut slot.occupant, Occupant::Vacant { next_free: None })
        };
        self.len -= 1;

        match prior {
            Occupant::Occupied(value) => Some(value),
            Occupant::Vacant { .. } => None,
        }
    }

    /// Removes every element and resets the free list.
    ///
    /// Underlying capacity is retained. Generation counters are preserved,
    /// so handles issued before the clear remain stale afterwards. Retired
    /// slots stay retired.
    pub fn clear(&mut self) {
        // Rebuild the free list back to front so it hands out the lowest
        // slot first, skipping retired slots.
        let mut next_free = None;
        for (i, slot) in self.slots.iter_mut().enumerate().rev() {
            if matches!(slot.occupant, Occupant::Occupied(_)) {
                slot.generation = slot.generation.checked_add(1).unwrap_or(RETIRED);
            }
            if slot.generation == RETIRED {
                slot.occupant = Occupant::Vacant { next_free: None };
            } else {
                slot.occupant = Occupant::Vacant { next_free };
                next_free = Some(i as u32);
            }
        }
        self.free_head = next_free;
        self.len = 0;
    }

    /// Returns an iterator over `(Index, &T)` for every live element.
    ///
    /// Vacant slots are skipped; iteration order follows slot indices,
    /// which is *not* the insertion order if any removals have happened.
    ///
    /// # Examples
    ///
    /// ```
    /// use arena_lib::Arena;
    ///
    /// let mut arena = Arena::new();
    /// let _a = arena.insert("alpha");
    /// let killed = arena.insert("dead");
    /// let _b = arena.insert("bravo");
    /// arena.remove(killed);
    ///
    /// let live: Vec<&&str> = arena.iter().map(|(_idx, value)| value).collect();
    /// assert_eq!(live, vec![&"alpha", &"bravo"]);
    /// ```
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            slots: self.slots.iter().enumerate(),
        }
    }

    /// Returns an iterator over `(Index, &mut T)` for every live element.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            slots: self.slots.iter_mut().enumerate(),
        }
    }
}

impl<T> Default for Arena<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for Arena<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Arena")
            .field("len", &self.len)
            .field("capacity", &self.slots.capacity())
            .finish()
    }
}

/// Iterator over `(Index, &T)` pairs returned by [`Arena::iter`].
pub struct Iter<'a, T> {
    slots: core::iter::Enumerate<core::slice::Iter<'a, Slot<T>>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (Index, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        for (i, slot) in self.slots.by_ref() {
            if let Occupant::Occupied(value) = &slot.occupant {
                return Some((
                    Index {
                        generation: slot.generation,
                        slot: i as u32,
                    },
                    value,
                ));
            }
        }
        None
    }
}

/// Iterator over `(Index, &mut T)` pairs returned by [`Arena::iter_mut`].
pub struct IterMut<'a, T> {
    slots: core::iter::Enumerate<core::slice::IterMut<'a, Slot<T>>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (Index, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        for (i, slot) in self.slots.by_ref() {
            let generation = slot.generation;
            if let Occupant::Occupied(value) = &mut slot.occupant {
                return Some((
                    Index {
                        generation,
                        slot: i as u32,
                    },
                    value,
                ));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    #[test]
    fn insert_and_get() {
        let mut a = Arena::new();
        let i = a.insert(42);
        assert_eq!(a.get(i), Some(&42));
        assert_eq!(a.len(), 1);
        assert!(!a.is_empty());
    }

    #[test]
    fn remove_invalidates_handle() {
        let mut a = Arena::new();
        let i = a.insert("hello");
        assert_eq!(a.remove(i), Some("hello"));
        assert!(a.get(i).is_none());
        assert!(a.remove(i).is_none());
    }

    #[test]
    fn slot_reuse_bumps_generation() {
        let mut a = Arena::new();
        let i1 = a.insert(1);
        let _ = a.remove(i1);
        let i2 = a.insert(2);
        assert_eq!(i1.slot(), i2.slot());
        assert_ne!(i1.generation(), i2.generation());
        assert!(a.get(i1).is_none());
        assert_eq!(a.get(i2), Some(&2));
    }

    #[test]
    fn iter_yields_only_live() {
        let mut a = Arena::new();
        let i1 = a.insert("a");
        let i2 = a.insert("b");
        let i3 = a.insert("c");
        let _ = a.remove(i2);
        let values: Vec<_> = a.iter().map(|(_, v)| *v).collect();
        assert_eq!(values, vec!["a", "c"]);
        let _ = (i1, i3);
    }

    #[test]
    fn clear_resets_len_and_invalidates_handles() {
        let mut a = Arena::new();
        let i = a.insert(7);
        a.clear();
        assert_eq!(a.len(), 0);
        assert!(a.get(i).is_none());
    }

    #[test]
    fn get_mut_mutates() {
        let mut a = Arena::new();
        let i = a.insert(10);
        if let Some(v) = a.get_mut(i) {
            *v = 99;
        }
        assert_eq!(a.get(i), Some(&99));
    }

    /// Forces the slot behind `idx` to the last generation, as if it had
    /// been reused about 4 billion times.
    fn exhaust_generation(a: &mut Arena<u32>, idx: Index) -> Index {
        a.slots[idx.slot as usize].generation = u32::MAX;
        Index {
            generation: u32::MAX,
            slot: idx.slot,
        }
    }

    #[test]
    fn exhausted_slot_is_retired_on_remove() {
        let mut a = Arena::new();
        let first = a.insert(1_u32);
        let last = exhaust_generation(&mut a, first);
        assert_eq!(a.get(last), Some(&1));
        assert_eq!(a.remove(last), Some(1));

        // Under 1.0.0 the generation wrapped to 0 and the slot was reused,
        // so one more cycle made `first` (generation 1) resolve again.
        let fresh = a.insert(2);
        let again = a.insert(3);
        assert_ne!(
            fresh.slot(),
            first.slot(),
            "retired slot must not be reused"
        );
        assert_ne!(again.slot(), first.slot());
        assert_eq!(a.get(first), None);
        assert_eq!(a.get(last), None);
        assert_eq!(a.try_get(first), Err(Error::StaleIndex));
        assert_eq!(a.len(), 2);
        assert_eq!(a.iter().count(), 2);
    }

    #[test]
    fn exhausted_slot_is_retired_on_clear() {
        let mut a = Arena::new();
        let first = a.insert(1_u32);
        let _second = a.insert(2);
        let last = exhaust_generation(&mut a, first);
        a.clear();
        assert_eq!(a.get(last), None);

        // Only slot 1 is free; slot 0 is retired.
        let x = a.insert(10);
        let y = a.insert(11);
        assert_eq!(x.slot(), 1);
        assert_eq!(y.slot(), 2);
        assert_eq!(a.get(first), None);

        // A second clear keeps slot 0 retired.
        a.clear();
        assert_ne!(a.insert(12).slot(), 0);
    }

    #[test]
    fn clear_hands_out_lowest_slot_first() {
        let mut a = Arena::new();
        for i in 0..4_u32 {
            let _ = a.insert(i);
        }
        a.clear();
        let slots: Vec<u32> = (0..4).map(|i| a.insert(i).slot()).collect();
        assert_eq!(slots, vec![0, 1, 2, 3]);
    }

    #[test]
    fn fallible_lookups_return_stale_index() {
        let mut a = Arena::new();
        let h = a.insert(7_u32);
        assert_eq!(a.try_get(h), Ok(&7));
        if let Ok(v) = a.try_get_mut(h) {
            *v = 8;
        }
        assert_eq!(a.try_remove(h), Ok(8));
        assert_eq!(a.try_get(h), Err(Error::StaleIndex));
        assert_eq!(a.try_get_mut(h), Err(Error::StaleIndex));
        assert_eq!(a.try_remove(h), Err(Error::StaleIndex));
    }

    #[test]
    fn contains_reflects_liveness() {
        let mut a = Arena::new();
        let i = a.insert(0_u8);
        assert!(a.contains(i));
        let _ = a.remove(i);
        assert!(!a.contains(i));
    }
}

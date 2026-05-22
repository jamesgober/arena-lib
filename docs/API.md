<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br><b>arena-lib</b><br>
    <sub><sup>API REFERENCE</sup></sub>
</h1>
<div align="center">
    <sup>
        <a href="../README.md" title="Project Home"><b>HOME</b></a>
        <span>&nbsp;│&nbsp;</span>
        <a href="../CHANGELOG.md" title="Changelog"><b>CHANGELOG</b></a>
        <span>&nbsp;│&nbsp;</span>
        <span>API</span>
        <span>&nbsp;│&nbsp;</span>
        <a href="./release/" title="Release Notes"><b>RELEASES</b></a>
    </sup>
</div>
<br>

This document is the canonical reference for every public-facing item in the `arena-lib` crate. It tracks the source of truth in `src/` and is updated before every release.

> **Status:** `arena-lib` `1.0.0` — **stable**. The API documented below is frozen for the entire 1.x line. Pre-1.0 hardening notes live in [docs/release/v0.9.0.md](./release/v0.9.0.md) (audit findings A1–A4); the 1.0 release note is in [docs/release/v1.0.0.md](./release/v1.0.0.md).

<br>

## Table of Contents

- **[Installation](#installation)**
- **[Quick Start](#quick-start)**
- **[Public APIs](#public-apis)**
  - [Constants](#constants)
    - [`VERSION`](#version)
  - [Error handling](#error-handling)
    - [`Error`](#error)
    - [`Result<T>`](#result)
  - [Generational Arena](#generational-arena)
    - [`Arena<T>`](#arena)
    - [`Index`](#index)
    - [`Iter` / `IterMut`](#arena-iter)
  - [String Interner](#string-interner)
    - [`Interner`](#interner)
    - [`Symbol`](#symbol)
  - [Bump Arena](#bump-arena)
    - [`Bump`](#bump)
  - [Drop Arena](#drop-arena)
    - [`DropArena<T>`](#droparena)
  - [Prelude](#prelude)
- **[Feature Flags](#feature-flags)**
  - [`std`](#feature-std)
- **[Compatibility](#compatibility)**
- **[API Stability (1.x)](#api-stability-1x)**
- **[Notes](#notes)**

<br><br>

<h2 id="installation">Installation</h2>

Add `arena-lib` to your `Cargo.toml`:

```toml
[dependencies]
arena-lib = "1"
```

Or with `cargo`:

```bash
cargo add arena-lib
```

Build under `no_std`:

```toml
[dependencies]
arena-lib = { version = "0.2", default-features = false }
```

See [Feature Flags](#feature-flags) for the full matrix.

<br>

<h2 id="quick-start">Quick Start</h2>

The 0.2 release ships four collaborating primitives. The example below wires them together end-to-end:

```rust
use arena_lib::prelude::*;

// Stable handles into a session table.
let mut arena: Arena<&'static str> = Arena::with_capacity(8);
let alice = arena.insert("alice");
let bob = arena.insert("bob");
assert_eq!(arena.get(alice), Some(&"alice"));

// Compact identifiers for repeated strings.
let mut interner = Interner::with_capacity(8);
let alice_id = interner.intern("user:alice");
let alice_again = interner.intern("user:alice");
assert_eq!(alice_id, alice_again);

// Fast scratch with O(1) reset.
let bump = Bump::with_capacity(64);
let scratch = bump.alloc([0_u8; 16]);
assert_eq!(scratch.len(), 16);

// Removing a slot invalidates its handle without affecting the rest.
assert_eq!(arena.remove(alice), Some("alice"));
assert!(arena.get(alice).is_none());
assert!(arena.get(bob).is_some());
```

<br>

<h2 id="public-apis">Public APIs</h2>

Every item listed in this section is part of the published, semver-tracked surface of `arena-lib`. Items not listed here are either internal or not yet released.

<br>

<h3 id="constants">Constants</h3>

<br>

<h4 id="version"><code>VERSION</code></h4>

```rust
pub const VERSION: &str;
```

Crate version string, populated by Cargo at build time from `CARGO_PKG_VERSION`. Mirrors the `version` field in `Cargo.toml` exactly.

**Type:** `&'static str`
**Stability:** stable since `0.1.0`. The value changes on every release.

**Examples**

```rust
use arena_lib::VERSION;

println!("arena-lib v{VERSION}");
assert!(!VERSION.is_empty());
```

<br><br>

<h3 id="error-handling">Error handling</h3>

`arena-lib` exposes a single [`Error`](#error) enum so callers can match on every failure mode without juggling per-module error types. New variants may be introduced in minor releases — match using `_ =>` to stay forward-compatible.

<br>

<h4 id="error"><code>Error</code></h4>

```rust
#[non_exhaustive]
pub enum Error {
    StaleIndex,
    CapacityExceeded,
    CounterOverflow,
}
```

The crate-wide error type.

**Variants**

| Variant | When it is returned |
| ------- | ------------------- |
| `StaleIndex` | An [`Index`](#index) was used after its slot was removed (or it never belonged to the arena). |
| `CapacityExceeded` | A [`Bump::try_alloc`](#bump) request exceeded the arena's remaining chunk space. |
| `CounterOverflow` | The arena's slot counter or the interner's symbol counter would have wrapped past `u32::MAX`. |

**Trait impls:** `Debug`, `Clone`, `PartialEq`, `Eq`, `Display`, and `std::error::Error` (under the `std` feature).

**Examples**

```rust
use arena_lib::{Arena, Error};

let arena: Arena<u32> = Arena::new();
// A handle synthesised against a different arena is rejected.
let bogus = arena_lib::Arena::<u32>::new();
// (no Index can be synthesised by the user — handles come only from `insert`)
let _ = arena.len();
let _ = Error::StaleIndex; // pattern-match variants directly
```

<br>

<h4 id="result"><code>Result&lt;T&gt;</code></h4>

```rust
pub type Result<T> = core::result::Result<T, Error>;
```

Result alias that uses [`Error`](#error) as the failure type. Used by every fallible entry point in the crate (`Arena::try_insert`, `Interner::try_intern`, `Bump::try_alloc`).

<br><br>

<h3 id="generational-arena">Generational Arena</h3>

The arena module ships a slab-style container that hands out stable [`Index`](#index) handles. When a slot is freed, its generation counter advances, so any still-held handle pointing at that slot is detected and rejected — no use-after-free, no dangling references, all in safe Rust.

**Cost summary**

| Operation | Cost |
| --------- | :--: |
| `insert` | amortized O(1) |
| `remove` | O(1) |
| `get` / `get_mut` / `contains` | O(1) |
| `iter` / `iter_mut` | O(capacity) |
| `clear` | O(capacity) |

<br>

<h4 id="arena"><code>Arena&lt;T&gt;</code></h4>

```rust
pub struct Arena<T>;

impl<T> Arena<T> {
    // Construction
    pub const fn new() -> Self;
    pub fn with_capacity(capacity: usize) -> Self;
    pub fn reserve(&mut self, additional: usize);

    // Inspection
    pub const fn len(&self) -> usize;
    pub const fn is_empty(&self) -> bool;
    pub fn capacity(&self) -> usize;
    pub fn contains(&self, idx: Index) -> bool;

    // Access
    pub fn get(&self, idx: Index) -> Option<&T>;
    pub fn get_mut(&mut self, idx: Index) -> Option<&mut T>;

    // Insertion
    pub fn insert(&mut self, value: T) -> Index;
    pub fn try_insert(&mut self, value: T) -> Result<Index>;

    // Removal
    pub fn remove(&mut self, idx: Index) -> Option<T>;
    pub fn clear(&mut self);

    // Iteration
    pub fn iter(&self) -> Iter<'_, T>;
    pub fn iter_mut(&mut self) -> IterMut<'_, T>;
}
```

The generational arena. `Arena::new` is `const` and allocates lazily; use `Arena::with_capacity(n)` when you already know how many slots you need.

**Constructors**

- `new` — empty arena, no allocation until first insert.
- `with_capacity(n)` — pre-reserves room for `n` slots; arena still grows on demand.
- `reserve(additional)` — reserves at least `additional` more slots.

**Inspection**

- `len` returns the count of live elements (not the slot count).
- `is_empty` is `len() == 0`.
- `capacity` returns the number of slots that can be held without reallocation (occupied + vacant).
- `contains(idx)` returns `true` iff `idx` refers to a live element.

**Access**

- `get(idx)` returns `Some(&T)` only if the handle is still live; stale handles return `None`.
- `get_mut(idx)` is the unique-borrow counterpart.

**Insertion**

- `insert(value)` returns a fresh [`Index`](#index). Panics only on the catastrophic case where the slot counter would overflow `u32::MAX`.
- `try_insert(value)` returns `Ok(Index)` or `Err(Error::CounterOverflow)` — same semantics, explicit failure.

**Removal**

- `remove(idx)` returns the element if the handle is live (and increments the slot's generation counter, invalidating any other handle to that slot). Stale handles return `None`.
- `clear()` removes every element, bumps every occupied generation, and retains capacity.

**Iteration**

- `iter()` yields `(Index, &T)` pairs for every live element in slot order.
- `iter_mut()` yields `(Index, &mut T)` pairs.

**Trait impls:** `Default`, `Debug` (where `T: Debug`).

**Examples**

End-to-end use:

```rust
use arena_lib::Arena;

let mut arena = Arena::with_capacity(4);
let a = arena.insert("alpha");
let b = arena.insert("bravo");

assert_eq!(arena.len(), 2);
assert_eq!(arena.get(a), Some(&"alpha"));

let removed = arena.remove(a);
assert_eq!(removed, Some("alpha"));
assert!(arena.get(a).is_none()); // stale
assert_eq!(arena.get(b), Some(&"bravo"));
```

Slot reuse with generation bump:

```rust
use arena_lib::Arena;

let mut arena = Arena::new();
let first = arena.insert(1);
let _ = arena.remove(first);
let second = arena.insert(2);

assert_eq!(first.slot(), second.slot()); // same slot reused
assert_ne!(first.generation(), second.generation()); // different generation
assert!(arena.get(first).is_none());        // old handle rejected
assert_eq!(arena.get(second), Some(&2));    // new handle accepted
```

Mutating in place:

```rust
use arena_lib::Arena;

let mut arena = Arena::new();
let h = arena.insert(10_u32);
if let Some(v) = arena.get_mut(h) {
    *v = 42;
}
assert_eq!(arena.get(h), Some(&42));
```

Iteration:

```rust
use arena_lib::Arena;

let mut arena = Arena::new();
let _ = arena.insert("a");
let killed = arena.insert("dead");
let _ = arena.insert("b");
let _ = arena.remove(killed);

let live: Vec<_> = arena.iter().map(|(_, v)| *v).collect();
assert_eq!(live, vec!["a", "b"]);
```

<br>

<h4 id="index"><code>Index</code></h4>

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Index { /* private fields */ }

impl Index {
    pub const fn slot(self) -> u32;
    pub const fn generation(self) -> u32;
}
```

Stable handle into an [`Arena`](#arena). An `Index` is 8 bytes (two `u32`s), `Copy`, and packs trivially into structs. The handle stays valid until the underlying element is removed; once stale, every arena lookup against it returns `None`.

**Notes**

- `slot()` is useful for diagnostics. Do **not** use it in place of the handle itself — slots are reused under new generations.
- Index values are not portable across arenas. Using an `Index` issued by one `Arena<T>` against a different `Arena<T>` returns `None`.

<br>

<h4 id="arena-iter"><code>Iter</code> / <code>IterMut</code></h4>

```rust
pub struct Iter<'a, T>;
pub struct IterMut<'a, T>;

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (Index, &'a T);
}
impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (Index, &'a mut T);
}
```

Iterators returned by [`Arena::iter`](#arena) and [`Arena::iter_mut`](#arena). Both skip vacant slots and yield `(Index, …)` pairs in slot order. Iteration is O(capacity) in the worst case (every slot vacant).

<br><br>

<h3 id="string-interner">String Interner</h3>

A string interner deduplicates owned string storage: repeated calls with the same input return the same compact [`Symbol`](#symbol). Equality and hashing operate on the four-byte symbol rather than the underlying bytes — the win when the same identifier appears thousands of times across a workload.

**Cost summary**

| Operation | Cost |
| --------- | :--: |
| `intern` (first sight) | expected O(1) |
| `intern` (repeat sight) | expected O(1) |
| `resolve` | O(1) |
| `lookup` / `contains` | expected O(1) |
| `len` / `is_empty` | O(1) |

The 0.5 implementation uses [`hashbrown::HashMap`](https://docs.rs/hashbrown) for the de-duplication index, keeping the crate `no_std`-compatible while delivering O(1) intern and lookup. The `default-hasher` feature of `hashbrown` is enabled, so `Interner::new()` and `Interner::with_capacity(n)` need no caller-supplied hasher.

<br>

<h4 id="interner"><code>Interner</code></h4>

```rust
pub struct Interner;

impl Interner {
    pub fn new() -> Self;
    pub fn with_capacity(capacity: usize) -> Self;

    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;

    pub fn intern(&mut self, s: &str) -> Symbol;
    pub fn try_intern(&mut self, s: &str) -> Result<Symbol>;

    pub fn resolve(&self, symbol: Symbol) -> Option<&str>;
    pub fn lookup(&self, s: &str) -> Option<Symbol>;
    pub fn contains(&self, s: &str) -> bool;

    pub fn iter(&self) -> Iter<'_>;
}
```

The string interner.

**Constructors**

- `new` — empty interner; the underlying hash table allocates on first insert.
- `with_capacity(n)` — pre-reserves both the storage vector and the hash lookup for `n` distinct strings.

**Interning**

- `intern(s)` — idempotent. Returns the existing [`Symbol`](#symbol) if `s` was seen before; otherwise allocates a fresh symbol. Panics only on `u32::MAX` overflow.
- `try_intern(s)` — explicit fallible variant returning `Result<Symbol>`.

**Lookup**

- `resolve(symbol)` — the inverse of `intern`. Returns `None` for symbols that did not originate from this interner.
- `lookup(s)` — non-inserting query: returns the existing symbol if any, else `None`.
- `contains(s)` — boolean form of `lookup`.

**Iteration**

- `iter()` yields `(Symbol, &str)` pairs for every interned string, in insertion order.

**Trait impls:** `Default`, `Debug`.

**Examples**

Deduplication and round-trip:

```rust
use arena_lib::Interner;

let mut interner = Interner::with_capacity(8);
let a = interner.intern("user:42");
let b = interner.intern("user:42");
let c = interner.intern("user:7");

assert_eq!(a, b);
assert_ne!(a, c);
assert_eq!(interner.resolve(a), Some("user:42"));
assert_eq!(interner.len(), 2);
```

Non-inserting lookup:

```rust
use arena_lib::Interner;

let mut interner = Interner::new();
let _ = interner.intern("known");

assert!(interner.contains("known"));
assert!(interner.lookup("unknown").is_none());
assert_eq!(interner.len(), 1); // lookup did not insert
```

<br>

<h4 id="symbol"><code>Symbol</code></h4>

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(/* private */);

impl Symbol {
    pub const fn id(self) -> u32;
}
```

Compact 4-byte handle returned by [`Interner::intern`](#interner). **Within a single interner**, two symbols compare equal if and only if the strings they refer to were interned from byte-identical inputs.

Symbols are scoped to the [`Interner`](#interner) that issued them: id values collide across separate interners (both start at 0, etc.). Passing a foreign symbol to [`Interner::resolve`](#interner) is undefined at the API contract level — the call may return `None`, or it may return an unrelated string. Treat `Symbol` values as opaque handles tied to a single interner.

<br><br>

<h3 id="bump-arena">Bump Arena</h3>

A linear allocator for short-lived scratch data, backed by a linked list of chunks. Allocations are O(1) (pointer bump + write); the entire arena clears in O(1) via [`Bump::reset`](#bump). Multiple references handed out from a shared `&self` borrow coexist safely because each allocation hands out a disjoint slice of a chunk.

When a chunk fills, the arena allocates a new one and continues — `alloc` is effectively infallible (it only fails if the global allocator itself fails, just like `Vec::push` or `Box::new`). After `reset`, subsequent allocations refill the existing chunks before any new chunk is requested.

**Cost summary**

| Operation | Cost |
| --------- | :--: |
| `alloc` / `try_alloc` | O(1) (amortised over chunk growth) |
| `reset` | O(1) |
| `allocated_bytes` / `chunk_capacity` / `chunk_count` | O(chunk_count) (typically tiny) |

> **Drop policy.** `Bump` does **not** run destructors when reset or dropped. Allocate types that do not require `Drop` (anything that is `Copy`, or owns only arena-internal memory). For payloads that own resources, use [`DropArena<T>`](#droparena).

<br>

<h4 id="bump"><code>Bump</code></h4>

```rust
pub struct Bump;

impl Bump {
    pub fn new() -> Self;
    pub fn with_capacity(capacity: usize) -> Self;

    pub fn chunk_capacity(&self) -> usize;
    pub fn chunk_count(&self) -> usize;
    pub fn allocated_bytes(&self) -> usize;

    pub fn alloc<T>(&self, value: T) -> &mut T;
    pub fn try_alloc<T>(&self, value: T) -> Result<&mut T>;

    pub fn reset(&mut self);
}
```

Multi-chunk bump arena.

**Constructors**

- `new` — empty arena; the first allocation triggers the allocation of an initial chunk (default size 4 KiB).
- `with_capacity(n)` — pre-allocates an initial chunk of `n` bytes. Subsequent chunks (if needed) are at least `n` bytes with a floor of 4 KiB.

**Allocation**

- `alloc(value)` — pointer bump + write, returning `&mut T` tied to `&self`. Allocates a new chunk if needed. Panics only if the global allocator fails.
- `try_alloc(value)` — explicit fallible variant returning `Result<&mut T>`.

The returned `&mut T` borrows from `&self`: [`Bump`](#bump) owns the underlying chunks and hands out disjoint regions per call, so multiple `&mut T` from the same `&Bump` coexist without aliasing. This matches `bumpalo::Bump::alloc`.

**Inspection**

- `chunk_capacity()` — total bytes reserved across every chunk currently held.
- `chunk_count()` — number of chunks currently held. Grows when allocation forces a new chunk; `reset` does **not** reduce it.
- `allocated_bytes()` — bytes consumed since the most recent `reset` (or since construction). Includes alignment padding and fully-used capacity of any chunks before the current cursor.

**Reset**

- `reset()` takes `&mut self`, statically guaranteeing no outstanding allocation reference can survive across the call. Capacity is retained.

**Threading**

`Bump` is `Send` (it owns its chunks) but **not** `Sync` — concurrent `&self` allocation across threads is rejected at compile time. Use one `Bump` per thread, or wrap in `Arc<Mutex<Bump>>` if you must share.

**Trait impls:** `Default`, `Debug`.

**Examples**

Basic allocation and reset:

```rust
use arena_lib::Bump;

let mut bump = Bump::with_capacity(64);
let a = bump.alloc(7_u32);
let b = bump.alloc(42_u32);
assert_eq!(*a, 7);
assert_eq!(*b, 42);

bump.reset();
assert_eq!(bump.allocated_bytes(), 0);
```

Alignment is handled automatically:

```rust
use arena_lib::Bump;

let bump = Bump::with_capacity(64);
let _ = bump.alloc(1_u8);                // 1 byte
let n = bump.alloc(0xdead_beef_u32);     // padded for u32 alignment
assert_eq!(*n, 0xdead_beef);
```

Growth across chunks (no explicit error to handle, but observable through `chunk_count`):

```rust
use arena_lib::Bump;

let bump = Bump::with_capacity(8);
let _ = bump.alloc([0_u8; 8]);          // fills the initial chunk
let chunks_before = bump.chunk_count();
let n = bump.alloc(42_u32);              // forces a new chunk
assert_eq!(*n, 42);
assert!(bump.chunk_count() > chunks_before);
```

<br><br>

<h3 id="drop-arena">Drop Arena</h3>

The drop-honouring sibling of [`Bump`](#bump). Both hand out `&mut T` from a shared `&self` borrow at O(1) cost, but `DropArena<T>` parks every value inside a `Vec<T>` chunk so that `T`'s destructor runs when the arena (or the chunk holding the value) is dropped. Reach for `DropArena` when payloads own resources — boxed data, file handles, mutexes — and you cannot leak on reset.

**Cost summary**

| Operation | Cost |
| --------- | :--: |
| `alloc` | amortised O(1) |
| `len` / `is_empty` / `chunk_count` | O(chunk_count) (typically tiny) |

<br>

<h4 id="droparena"><code>DropArena&lt;T&gt;</code></h4>

```rust
pub struct DropArena<T>;

impl<T> DropArena<T> {
    pub fn new() -> Self;
    pub fn with_chunk_capacity(chunk_capacity: usize) -> Self;

    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn chunk_count(&self) -> usize;

    pub fn alloc(&self, value: T) -> &mut T;
}
```

Typed drop-honouring arena.

**Constructors**

- `new` — empty arena; the first allocation triggers an initial chunk holding 16 `T` slots.
- `with_chunk_capacity(n)` — empty arena whose chunks each hold `n` `T` slots. A zero is silently clamped to 1.

**Allocation**

- `alloc(value)` — moves `value` into the current chunk's spare capacity. Allocates a new chunk if needed. Panics only if the global allocator fails. Returns `&mut T` tied to `&self`.

The chunk layout — a `Vec<Vec<T>>` where each inner `Vec` is filled by direct writes into its spare capacity — means inner `Vec` buffers are never reallocated once issued, so handed-out references remain valid for the lifetime of `&self`. Outer `Vec` reallocations move inner `Vec` headers but not their heap buffers.

**Threading**

`DropArena<T>` is `Send` when `T: Send`, never `Sync`. Use one arena per thread.

**No `reset`.** Destructors are honoured only via `Drop`. Dropping the arena (or letting it go out of scope) runs every `T`'s destructor exactly once. There is no in-place reset because that would require dropping every live value while outstanding `&mut T` borrows are still in scope, which the borrow checker cannot model safely.

**Trait impls:** `Default`, `Debug` (where `T: Debug`).

**Examples**

Owned `String` payloads — drop frees the heap buffers when the arena dies:

```rust
use arena_lib::DropArena;

let arena = DropArena::<String>::new();
let s1 = arena.alloc(String::from("alpha"));
let s2 = arena.alloc(String::from("bravo"));
assert_eq!(s1, "alpha");
assert_eq!(s2, "bravo");
assert_eq!(arena.len(), 2);
// `arena` drops here; both String heaps are freed.
```

Shared ownership — confirm destructors actually run:

```rust
use arena_lib::DropArena;
use std::sync::Arc;

let shared = Arc::new(0_u32);
{
    let arena = DropArena::<Arc<u32>>::new();
    let _ = arena.alloc(Arc::clone(&shared));
    let _ = arena.alloc(Arc::clone(&shared));
    assert_eq!(Arc::strong_count(&shared), 3);
}
assert_eq!(Arc::strong_count(&shared), 1); // arena dropped; clones gone.
```

<br><br>

<h3 id="prelude">Prelude</h3>

```rust
pub mod prelude {
    pub use crate::arena::{Arena, Index};
    pub use crate::bump::Bump;
    pub use crate::drop_arena::DropArena;
    pub use crate::error::{Error, Result};
    pub use crate::intern::{Interner, Symbol};
}
```

Glob-import this module to bring the most commonly used types into scope:

```rust
use arena_lib::prelude::*;

let mut arena: Arena<&'static str> = Arena::new();
let mut interner = Interner::new();
let bump = Bump::with_capacity(64);

let _ = arena.insert("alice");
let _ = interner.intern("session-key");
let _ = bump.alloc(7_u32);
```

<br><br>

<h2 id="feature-flags">Feature Flags</h2>

`arena-lib` is `no_std`-compatible by design. Cargo features control which standard-library conveniences are compiled in.

| Feature | Default | Description |
| ------- | :-----: | ----------- |
| [`std`](#feature-std) |   yes   | Enables `std`. Disable for `no_std` consumers. |

<br>

<h3 id="feature-std"><code>std</code></h3>

Enables the standard library. On by default. The crate always requires `alloc` (pulled in automatically) regardless of `std`.

```toml
# no_std consumers:
[dependencies]
arena-lib = { version = "0.2", default-features = false }
```

```toml
# explicit re-enable:
[dependencies]
arena-lib = { version = "0.2", default-features = false, features = ["std"] }
```

The public surface is identical under both modes today. With `std` enabled, [`Error`](#error) also implements `std::error::Error`.

<br><br>

<h2 id="compatibility">Compatibility</h2>

| Concern        | Value                                  |
| -------------- | -------------------------------------- |
| **MSRV**       | Rust `1.85`                            |
| **Edition**    | `2024`                                 |
| **Platforms**  | Linux, macOS, Windows (Tier-1 targets) |
| **`no_std`**   | Supported via `default-features = false` |
| **Unsafe**     | Internal only; never leaks to user code |
| **License**    | `Apache-2.0 OR MIT`                    |

The crate runs identically on all Tier-1 targets. Platform-specific behavior — if any is ever introduced — will be feature-gated and documented at the call site.

<br><br>

<h2 id="api-stability-1x">API Stability (1.x)</h2>

The surface documented above is the **frozen 1.0 contract**. Any further evolution within the 1.x line is **purely additive**:

- New methods on existing types.
- New variants on the `#[non_exhaustive]` `Error` enum.
- New types in new modules.

Anything that would break a 1.x caller — removed items, renamed methods, signature changes, reshaped error variants — is out of scope until a hypothetical 2.0. Per-release notes will continue to live under [`docs/release/`](./release/).

<br><br>

<h2 id="notes">Notes</h2>

- **Source of truth.** This document mirrors `src/`. If you find a divergence, the source wins — and the divergence is a doc bug worth filing.
- **Stability.** Every item under [Public APIs](#public-apis) is part of the semver contract from the listed version onward. The crate is pre-1.0; minor releases may still introduce additions, but documented items will not silently change shape.
- **Safety.** `arena-lib` never exposes `unsafe` in its public surface. The internal `unsafe` blocks (concentrated in the bump arena) carry `// SAFETY:` documentation and are measured against alternatives before being kept.
- **Reporting issues.** File documentation bugs, missing examples, or API gaps at the [project repository](https://github.com/jamesgober/arena-lib/issues).

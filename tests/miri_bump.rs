//! Aliasing regression tests for `Bump` and `DropArena`.
//!
//! These pass natively on every version. Their value is under Miri:
//! `cargo +nightly miri test --test miri_bump` must report no undefined
//! behaviour under both Stacked Borrows (the default) and Tree Borrows
//! (`MIRIFLAGS=-Zmiri-tree-borrows`). arena-lib 1.0.0 failed all three
//! `bump_*` write tests under both models.

use arena_lib::{Bump, DropArena};

#[test]
fn bump_single_alloc_write() {
    let bump = Bump::new();
    let a = bump.alloc(1_u64);
    *a = 5;
    assert_eq!(*a, 5);
}

#[test]
fn bump_two_live_refs_then_write_first() {
    let bump = Bump::new();
    let a = bump.alloc(1_u64);
    let b = bump.alloc(2_u64);
    *a = 3;
    *b = 4;
    assert_eq!(*a + *b, 7);
}

#[test]
fn bump_ref_survives_new_chunk() {
    let bump = Bump::new();
    let a = bump.alloc([0_u8; 4000]);
    let b = bump.alloc([1_u8; 4000]); // forces a second chunk
    a[0] = 9;
    assert_eq!(a[0] + b[0], 10);
}

#[test]
fn bump_mixed_alignments_stay_writable() {
    let bump = Bump::with_capacity(64);
    let a = bump.alloc(1_u8);
    let b = bump.alloc(2_u64);
    let c = bump.alloc(3_u16);
    let d = bump.alloc([4_u32; 32]); // spills into a new chunk
    *a += 1;
    *b += 1;
    *c += 1;
    d[31] += 1;
    assert_eq!((*a, *b, *c, d[31]), (2, 3, 4, 5));
}

#[test]
fn bump_reuse_after_reset() {
    let mut bump = Bump::with_capacity(32);
    {
        let a = bump.alloc(7_u64);
        *a += 1;
        assert_eq!(*a, 8);
    }
    bump.reset();
    let b = bump.alloc(9_u64);
    *b += 1;
    assert_eq!(*b, 10);
}

#[test]
fn bump_zst_huge_align() {
    #[repr(align(4096))]
    struct Z;
    let bump = Bump::new();
    let z: &mut Z = bump.alloc(Z);
    assert_eq!((z as *mut Z as usize) % 4096, 0);
}

#[test]
fn drop_arena_two_live_refs() {
    let arena = DropArena::with_chunk_capacity(2);
    let a = arena.alloc(String::from("a"));
    let b = arena.alloc(String::from("b"));
    let c = arena.alloc(String::from("c")); // new chunk
    a.push('x');
    b.push('y');
    c.push('z');
    assert_eq!(a.as_str(), "ax");
    assert_eq!(b.as_str(), "by");
    assert_eq!(c.as_str(), "cz");
}

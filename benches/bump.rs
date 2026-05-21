//! Microbenchmarks for [`arena_lib::Bump`] and [`arena_lib::DropArena`].

use arena_lib::{Bump, DropArena};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn bench_bump_alloc_u64(c: &mut Criterion) {
    let mut group = c.benchmark_group("Bump::alloc<u64>");
    for n in [16_usize, 1024, 65536] {
        group.bench_function(format!("n={n}"), |b| {
            b.iter(|| {
                let bump = Bump::with_capacity(8 * n);
                for i in 0..n as u64 {
                    let _ = black_box(bump.alloc(i));
                }
                bump
            });
        });
    }
    group.finish();
}

fn bench_bump_alloc_array(c: &mut Criterion) {
    c.bench_function("Bump::alloc<[u8; 64]> (1000 allocations)", |b| {
        b.iter(|| {
            let bump = Bump::with_capacity(64 * 1024);
            for i in 0..1000_u32 {
                let _ = black_box(bump.alloc([(i & 0xff) as u8; 64]));
            }
            bump
        });
    });
}

fn bench_bump_reset_reuse(c: &mut Criterion) {
    c.bench_function(
        "Bump: 1000 allocs + reset + 1000 allocs (chunk reuse)",
        |b| {
            b.iter(|| {
                let mut bump = Bump::with_capacity(8 * 1024);
                for i in 0..1000_u64 {
                    let _ = black_box(bump.alloc(i));
                }
                bump.reset();
                for i in 0..1000_u64 {
                    let _ = black_box(bump.alloc(i));
                }
                bump
            });
        },
    );
}

fn bench_drop_arena_alloc_string(c: &mut Criterion) {
    c.bench_function("DropArena::alloc<String> (1000 allocations)", |b| {
        b.iter(|| {
            let arena: DropArena<String> = DropArena::with_chunk_capacity(256);
            for i in 0..1000_u32 {
                let _ = black_box(arena.alloc(format!("payload-{i}")));
            }
            arena
        });
    });
}

criterion_group!(
    bump_benches,
    bench_bump_alloc_u64,
    bench_bump_alloc_array,
    bench_bump_reset_reuse,
    bench_drop_arena_alloc_string
);
criterion_main!(bump_benches);

//! Microbenchmarks for [`arena_lib::Arena`].

use arena_lib::Arena;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn bench_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("Arena::insert");
    for size in [16, 256, 4096] {
        group.bench_function(format!("n={size}"), |b| {
            b.iter(|| {
                let mut arena: Arena<u64> = Arena::with_capacity(size);
                for i in 0..size {
                    let _ = black_box(arena.insert(i as u64));
                }
                arena
            });
        });
    }
    group.finish();
}

fn bench_get_after_insert(c: &mut Criterion) {
    c.bench_function("Arena::get (live handle, n=1000)", |b| {
        b.iter_batched(
            || {
                let mut arena: Arena<u64> = Arena::with_capacity(1000);
                let handles: Vec<_> = (0..1000_u64).map(|i| arena.insert(i)).collect();
                (arena, handles)
            },
            |(arena, handles)| {
                let mut sum: u64 = 0;
                for h in &handles {
                    if let Some(v) = arena.get(*h) {
                        sum = sum.wrapping_add(*v);
                    }
                }
                black_box(sum)
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_remove_insert_churn(c: &mut Criterion) {
    c.bench_function("Arena: 1000-element insert+remove churn", |b| {
        b.iter(|| {
            let mut arena: Arena<u64> = Arena::with_capacity(1000);
            let handles: Vec<_> = (0..1000_u64).map(|i| arena.insert(i)).collect();
            for h in &handles {
                let _ = black_box(arena.remove(*h));
            }
            for i in 1000..2000_u64 {
                let _ = black_box(arena.insert(i));
            }
            arena
        });
    });
}

criterion_group!(
    arena_benches,
    bench_insert,
    bench_get_after_insert,
    bench_remove_insert_churn
);
criterion_main!(arena_benches);

//! Microbenchmarks for [`arena_lib::Interner`].

use arena_lib::Interner;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn bench_intern_unique(c: &mut Criterion) {
    c.bench_function("Interner::intern (1000 unique strings)", |b| {
        b.iter_batched(
            || {
                (0..1000_u32)
                    .map(|i| format!("user:{i}"))
                    .collect::<Vec<_>>()
            },
            |strings| {
                let mut interner = Interner::with_capacity(1000);
                for s in &strings {
                    let _ = black_box(interner.intern(s));
                }
                interner
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_intern_repeated(c: &mut Criterion) {
    c.bench_function(
        "Interner::intern (1000 lookups of 10 distinct strings)",
        |b| {
            b.iter_batched(
                || {
                    let strings: Vec<String> = (0..10).map(|i| format!("kind:{i}")).collect();
                    let mut interner = Interner::with_capacity(10);
                    for s in &strings {
                        let _ = interner.intern(s);
                    }
                    (interner, strings)
                },
                |(mut interner, strings)| {
                    let mut sum: u32 = 0;
                    for i in 0..1000 {
                        let s = &strings[i % strings.len()];
                        sum = sum.wrapping_add(interner.intern(s).id());
                    }
                    black_box(sum)
                },
                BatchSize::SmallInput,
            );
        },
    );
}

fn bench_resolve(c: &mut Criterion) {
    c.bench_function("Interner::resolve (1000 hits)", |b| {
        b.iter_batched(
            || {
                let mut interner = Interner::with_capacity(1000);
                let symbols: Vec<_> = (0..1000_u32)
                    .map(|i| interner.intern(&format!("sym:{i}")))
                    .collect();
                (interner, symbols)
            },
            |(interner, symbols)| {
                let mut total: usize = 0;
                for s in &symbols {
                    if let Some(text) = interner.resolve(*s) {
                        total = total.wrapping_add(text.len());
                    }
                }
                black_box(total)
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    intern_benches,
    bench_intern_unique,
    bench_intern_repeated,
    bench_resolve
);
criterion_main!(intern_benches);

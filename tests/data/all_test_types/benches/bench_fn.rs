use all_test_types::only_bench::*;
use criterion::{criterion_group, criterion_main, Criterion};

fn check_speed(c: &mut Criterion) {
    c.bench_function("some_fn", |b| {
        b.iter(|| only_ran_in_benches(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]))
    });
}

criterion_group!(benches, check_speed);
criterion_main!(benches);

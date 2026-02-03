use criterion::{Criterion, criterion_group, criterion_main};
use pascal_rust::interprete;

fn criternion_benchmark(c: &mut Criterion) {
    c.bench_function("main 20", |b| b.iter(|| interprete()));
}

criterion_group!(benches, criternion_benchmark);
criterion_main!(benches);

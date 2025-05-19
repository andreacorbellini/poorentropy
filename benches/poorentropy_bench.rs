use criterion::criterion_group;
use criterion::criterion_main;
use criterion::Criterion;
use std::hint::black_box;

pub fn get(c: &mut Criterion) {
    c.bench_function("get", |b| b.iter(|| black_box(poorentropy::get())));
}

pub fn fill(c: &mut Criterion) {
    fn bench_fill<const N: usize>(c: &mut Criterion) {
        c.bench_function(&format!("fill {N} bytes"), |b| {
            b.iter(move || {
                let mut bytes = [0u8; N];
                poorentropy::fill(&mut bytes);
                black_box(bytes);
            })
        });
    }

    bench_fill::<8>(c);
    bench_fill::<16>(c);
    bench_fill::<32>(c);
    bench_fill::<64>(c);
    bench_fill::<1024>(c);
    bench_fill::<2048>(c);
    bench_fill::<4096>(c);
}

criterion_group!(benches, get, fill);
criterion_main!(benches);

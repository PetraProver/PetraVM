use std::collections::HashSet;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use petravm_asm::memory::vrom::ValueRom;
use rand::{rngs::StdRng, Rng, SeedableRng};

const VROM_SIZE: usize = 1 << 16; // 64K slots
const ACCESS_COUNTS: [usize; 3] = [100, 1_000, 10_000];

fn bench_vrom_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("VROM Reads");

    for &n in &ACCESS_COUNTS {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || {
                    // Init VROM with random values
                    let mut rng = StdRng::seed_from_u64(42);
                    let init_vals = (0..VROM_SIZE)
                        .map(|_| rng.random_range(0..=u32::MAX))
                        .collect::<Vec<_>>();
                    let vrom = ValueRom::new_with_init_vals(&init_vals);
                    let indices = (0..n)
                        .map(|_| rng.random_range(0..VROM_SIZE as u32))
                        .collect::<Vec<_>>();
                    (vrom, indices)
                },
                |(vrom, indices)| {
                    for &i in &indices {
                        let _ = vrom.read::<u32>(i).unwrap();
                    }
                },
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

fn bench_vrom_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("VROM Writes");

    for &n in &ACCESS_COUNTS {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || {
                    let mut rng = StdRng::seed_from_u64(123);
                    let values = (0..n)
                        .map(|_| {
                            (
                                rng.random_range(0..VROM_SIZE as u32),
                                rng.random_range(0..=u32::MAX),
                            )
                        })
                        .collect::<HashSet<_>>();

                    let mut vrom = ValueRom::default();
                    let _ = vrom.write((VROM_SIZE + 1) as u32, 0u32, false); // To ensure enough capacity
                    (vrom, values)
                },
                |(mut vrom, values)| {
                    for &(i, val) in &values {
                        let _ = vrom.write::<u32>(i, val, false);
                    }
                },
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

criterion_group!(vrom, bench_vrom_reads, bench_vrom_writes);
criterion_main!(vrom);

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use petravm_asm::isa::GenericISA;
use petravm_prover::model::Trace;
use petravm_prover::prover::{verify_proof, Prover};
use petravm_prover::test_utils::generate_trace;
use rand::Rng;

fn generate_shift_trace(n: usize) -> Result<Trace, anyhow::Error> {
    let mut rng = rand::rng();

    let mut asm_lines = vec![
        format!("#[framesize(0x{:x})]", n * 3 + 2),
        "_start:".to_string(),
    ];

    // We only use right logical and arithmetic shifts in this benchmark
    let shift_opcodes = ["SRLI", "SRL", "SRAI", "SRA"];
    let num_opcodes = shift_opcodes.len();

    for i in 0..n {
        let src_pos = 2 + i * 3;
        let shift_amount_pos = src_pos + 1;
        let dst_pos = shift_amount_pos + 1;

        let opcode_index = i % num_opcodes;
        let opcode = shift_opcodes[opcode_index];

        let src_val = rng.random::<u32>();
        let shift_amount = rng.random_range(0..32);
        let shift_imm = shift_amount as u16;

        asm_lines.push(format!("LDI.W @{src_pos}, #{src_val}"));
        asm_lines.push(format!("LDI.W @{shift_amount_pos}, #{shift_amount}"));

        let line = match opcode {
            "SRLI" => format!("SRLI @{dst_pos}, @{src_pos}, #{shift_imm}"),
            "SRL" => format!("SRL  @{dst_pos}, @{src_pos}, @{shift_amount_pos}"),
            "SRAI" => format!("SRAI @{dst_pos}, @{src_pos}, #{shift_imm}"),
            "SRA" => format!("SRA  @{dst_pos}, @{src_pos}, @{shift_amount_pos}"),
            _ => unreachable!(),
        };
        asm_lines.push(line);
    }

    asm_lines.push("RET".to_string());
    let asm_code = asm_lines.join("\n");

    generate_trace(asm_code, None, None)
}

fn bench_shifts(c: &mut Criterion) {
    let mut group = c.benchmark_group("Shift Operations");

    let sizes = [2048, 8192, 16384];
    let sample_sizes = [30, 15, 10];

    for (&n, &sample_size) in sizes.iter().zip(sample_sizes.iter()) {
        let trace = generate_shift_trace(n).expect("Failed to generate shift trace");
        let prover = Prover::new(Box::new(GenericISA));

        group.sample_size(sample_size);

        group.bench_with_input(BenchmarkId::new("Prove", n), &n, |b, _n_val| {
            b.iter(|| {
                let (_proof, _statement, _compiled_cs) = prover.prove(&trace).unwrap();
            });
        });

        let (proof, statement, compiled_cs) = prover.prove(&trace).unwrap();

        group.bench_with_input(BenchmarkId::new("Verify", n), &n, |b, _n_val| {
            b.iter(|| {
                verify_proof(&statement, &compiled_cs, proof.clone()).unwrap();
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_shifts);
criterion_main!(benches);

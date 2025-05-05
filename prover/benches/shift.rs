use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use zcrayvm_assembly::isa::GenericISA;
use zcrayvm_prover::model::Trace;
use zcrayvm_prover::prover::{verify_proof, Prover};
use zcrayvm_prover::test_utils::generate_trace;

fn generate_shift_trace(n: usize) -> Result<Trace, anyhow::Error> {
    let initial_val: u32 = 0xdeadbeef;
    let shift_amount: u32 = 5;
    let shift_imm = shift_amount as u16;

    let mut asm_lines = vec![
        format!("#[framesize(0x{:x})]", n + 4),
        "_start:".to_string(),
        format!("LDI.W @2, #{}", initial_val),
        format!("LDI.W @3, #{}", shift_amount),
    ];

    let shift_opcodes = ["SRLI", "SRL", "SLLI", "SLL", "SRAI", "SRA"];
    let num_opcodes = shift_opcodes.len();

    for i in 0..n {
        let dest_reg = 4 + i;
        let opcode_index = i % num_opcodes;
        let opcode = shift_opcodes[opcode_index];

        let line = match opcode {
            "SRLI" => format!("SRLI @{}, @2, #{}", dest_reg, shift_imm),
            "SRL" => format!("SRL  @{}, @2, @3", dest_reg),
            "SLLI" => format!("SLLI @{}, @2, #{}", dest_reg, shift_imm),
            "SLL" => format!("SLL  @{}, @2, @3", dest_reg),
            "SRAI" => format!("SRAI @{}, @2, #{}", dest_reg, shift_imm),
            "SRA" => format!("SRA  @{}, @2, @3", dest_reg),
            _ => unreachable!(),
        };
        asm_lines.push(line);
    }

    asm_lines.push("RET".to_string());
    let asm_code = asm_lines.join("\n");

    let init_values = Some(vec![0, 0, initial_val, shift_amount]);
    generate_trace(asm_code, init_values, None)
}

fn bench_shifts(c: &mut Criterion) {
    let mut group = c.benchmark_group("Shift Operations");

    for n in [48, 192, 768].iter() {
        let trace = generate_shift_trace(*n).expect("Failed to generate shift trace");
        let prover = Prover::new(Box::new(GenericISA));

        group.bench_with_input(BenchmarkId::new("Prove", n), n, |b, _n_val| {
            b.iter(|| {
                let (_proof, _statement, _compiled_cs) = prover.prove(&trace).unwrap();
            });
        });

        let (proof, statement, compiled_cs) = prover.prove(&trace).unwrap();

        group.bench_with_input(BenchmarkId::new("Verify", n), n, |b, _n_val| {
            b.iter(|| {
                verify_proof(&statement, &compiled_cs, proof.clone()).unwrap();
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_shifts);
criterion_main!(benches);

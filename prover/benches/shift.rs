use criterion::{criterion_group, criterion_main, Criterion};
use zcrayvm_assembly::isa::GenericISA;
use zcrayvm_prover::model::Trace;
use zcrayvm_prover::prover::{verify_proof, Prover};
use zcrayvm_prover::test_utils::generate_trace;

fn generate_shift_trace(n: usize) -> Result<Trace, anyhow::Error> {
    let initial_val: u32 = 0xdeadbeef;
    let shift_amount: u32 = 5;

    let mut asm_lines = vec![
        "#[framesize(0x10)]".to_string(),
        "_start:".to_string(),
        format!("LDI.W @2, #{}", initial_val),
    ];

    for i in 0..n {
        let dest_reg = 4 + (i % 10);
        let shift_imm = shift_amount as u16;
        asm_lines.push(format!("SRLI @{}, @2, #{}", dest_reg, shift_imm));
    }

    asm_lines.push("RET".to_string());
    let asm_code = asm_lines.join(
        "
",
    );

    let init_values = Some(vec![0, 0, initial_val]);
    generate_trace(asm_code, init_values, None)
}

fn bench_shift_prove(c: &mut Criterion) {
    let n = 10;
    let trace = generate_shift_trace(n).expect("Failed to generate shift trace");
    let prover = Prover::new(Box::new(GenericISA));
    c.bench_function(&format!("shift_prove_{}", n), |b| {
        b.iter(|| {
            let (_proof, _statement, _compiled_cs) = prover.prove(&trace).unwrap();
        })
    });
}

fn bench_shift_verify(c: &mut Criterion) {
    let n = 10;
    let trace = generate_shift_trace(n).expect("Failed to generate shift trace");
    let prover = Prover::new(Box::new(GenericISA));
    let (proof, statement, compiled_cs) = prover.prove(&trace).unwrap();
    c.bench_function(&format!("shift_verify_{}", n), |b| {
        b.iter(|| {
            verify_proof(&statement, &compiled_cs, proof.clone()).unwrap();
        })
    });
}

criterion_group!(benches, bench_shift_prove, bench_shift_verify);
criterion_main!(benches);

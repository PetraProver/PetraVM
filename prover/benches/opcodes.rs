
use criterion::{criterion_group, criterion_main, Criterion};
use petravm_asm::isa::GenericISA;
use petravm_asm::opcodes::Opcode;
use petravm_prover::model::Trace;
use petravm_prover::prover::Prover;
use petravm_prover::test_utils::generate_trace;
use rand::Rng;

// ========== Trace Generation Functions ==========

/// Generate a trace for a specific opcode
fn generate_opcode_trace(opcode: Opcode, n: usize) -> Result<Trace, anyhow::Error> {
    let mut rng = rand::rng();
    let src_val = rng.random::<u32>();
    let src2_val = rng.random::<u32>();
    let imm_val = rng.random::<u16>();

    let mut asm_lines = vec![
        "#[framesize(0x10)]".to_string(),
        "_start:".to_string(),
        format!("LDI.W @4, #{}", src_val),
        format!("LDI.W @8, #{}", src2_val),
    ];

    for _ in 0..n {
        asm_lines.push(generate_instruction(opcode, 12, 4, 8, imm_val));
    }

    asm_lines.push("RET".to_string());
    let asm_code = asm_lines.join("\n");

    generate_trace(asm_code, None, None)
}

/// Generates the assembly instruction for an opcode
fn generate_instruction(opcode: Opcode, dst: usize, src1: usize, src2: usize, imm: u16) -> String {
    match opcode {
        // Shift right operations
        Opcode::Srli => format!("SRLI @{dst}, @{src1}, #{}", imm % 32),
        Opcode::Srl => format!("SRL  @{dst}, @{src1}, @{src2}"),
        Opcode::Srai => format!("SRAI @{dst}, @{src1}, #{}", imm % 32),
        Opcode::Sra => format!("SRA  @{dst}, @{src1}, @{src2}"),

        // Shift left operations
        Opcode::Slli => format!("SLLI @{dst}, @{src1}, #{}", imm % 32),
        Opcode::Sll => format!("SLL  @{dst}, @{src1}, @{src2}"),

        // Arithmetic operations
        Opcode::Add => format!("ADD  @{dst}, @{src1}, @{src2}"),
        Opcode::Addi => format!("ADDI @{dst}, @{src1}, #{imm}"),
        _ => panic!("Unsupported opcode for benchmarking: {opcode:?}"),
    }
}

/// Returns all opcodes to benchmark
fn opcodes_to_benchmark() -> Vec<Opcode> {
    vec![
        // Shift operations
        Opcode::Srli,
        Opcode::Srl,
        Opcode::Srai,
        Opcode::Sra,
        Opcode::Slli,
        Opcode::Sll,
        // Arithmetic operations
        Opcode::Add,
        Opcode::Addi,
    ]
}

// ========== Benchmark Function ==========

/// Benchmark a specific opcode
fn bench_opcode(c: &mut Criterion, opcode: Opcode) {
    // Use a smaller n for benchmarking to keep run times reasonable
    let n = 300;
    let trace = generate_opcode_trace(opcode, n)
        .unwrap_or_else(|_| panic!("Failed to generate {opcode:?} trace"));
    let prover = Prover::new(Box::new(GenericISA));

    let mut group = c.benchmark_group(format!("{opcode:?}"));
    group.sample_size(10);

    // Only benchmark proving, not verification as per requirements
    group.bench_function("prove", |b| b.iter(|| prover.prove(&trace)));

    group.finish();
}

/// Add all opcodes to the benchmark
fn bench_all_opcodes(c: &mut Criterion) {
    for opcode in opcodes_to_benchmark() {
        bench_opcode(c, opcode);
    }
}

// ========== Criterion Configuration ==========

criterion_group!(benches, bench_all_opcodes);
criterion_main!(benches);

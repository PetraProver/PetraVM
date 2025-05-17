use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use petravm_asm::isa::GenericISA;
use petravm_asm::opcodes::Opcode;
use petravm_prover::model::Trace;
use petravm_prover::prover::Prover;
use petravm_prover::test_utils::generate_trace;
use rand::{rng, Rng};

const TRACE_LEN: usize = 1 << 20; // 1M instructions per trace
const SAMPLE_SIZE: usize = 20; // Number of benchmark runs per opcode

/// Generate a random VROM trace for the given opcode
fn generate_trace_for(opcode: Opcode, length: usize) -> Trace {
    let mut rng = rng();
    let imm = rng.random::<u16>();
    let mut asm = Vec::with_capacity(length + 12);

    asm.push("#[framesize(0x10)]".to_owned());
    asm.push("_start:".to_owned());

    // Initialize registers @4 through @11 with random values
    for reg in 4..=11 {
        let val = rng.random::<u32>();
        asm.push(format!("LDI.W @{reg}, #{val}"));
    }

    // Emit `length` instructions of the target opcode
    for _ in 0..length {
        asm.push(format_instruction(opcode, 12, 4, 8, imm));
    }

    asm.push("RET".to_owned());
    let program = asm.join("\n");
    generate_trace(program, None, None).expect("Trace generation failed")
}

/// Format a single instruction for benchmarking
fn format_instruction(opcode: Opcode, dst: usize, src1: usize, src2: usize, imm: u16) -> String {
    use Opcode::*;
    match opcode {
        // Binary field operations
        Xor => format!("XOR    @{dst}, @{src1}, @{src2}"),
        Xori => format!("XORI   @{dst}, @{src1}, #{imm}"),
        B32Mul => format!("B32_MUL @{dst}, @{src1}, @{src2}"),
        B32Muli => format!("B32_MULI @{dst}, @{src1}, #{imm}G"),
        B128Add => format!("B128_ADD @{dst}, @{src1}, @{src2}"),
        B128Mul => format!("B128_MUL @{dst}, @{src1}, @{src2}"),

        // Integer arithmetic
        Add => format!("ADD    @{dst}, @{src1}, @{src2}"),
        Addi => format!("ADDI   @{dst}, @{src1}, #{imm}"),
        Sub => format!("SUB    @{dst}, @{src1}, @{src2}"),

        // Bitwise logic
        And => format!("AND    @{dst}, @{src1}, @{src2}"),
        Andi => format!("ANDI   @{dst}, @{src1}, #{imm}"),
        Or => format!("OR     @{dst}, @{src1}, @{src2}"),
        Ori => format!("ORI    @{dst}, @{src1}, #{imm}"),

        // Shift operations
        Sll => format!("SLL    @{dst}, @{src1}, @{src2}"),
        Slli => format!("SLLI   @{dst}, @{src1}, #{imm}"),
        Srl => format!("SRL    @{dst}, @{src1}, @{src2}"),
        Srli => format!("SRLI   @{dst}, @{src1}, #{imm}"),
        Sra => format!("SRA    @{dst}, @{src1}, @{src2}"),
        Srai => format!("SRAI   @{dst}, @{src1}, #{imm}"),

        // Multiplication
        Mul => format!("MUL    @{dst}, @{src1}, @{src2}"),
        Muli => format!("MULI   @{dst}, @{src1}, #{imm}"),
        Mulu => format!("MULU   @{dst}, @{src1}, @{src2}"),
        Mulsu => format!("MULSU  @{dst}, @{src1}, @{src2}"),

        // Comparisons
        Slt => format!("SLT    @{dst}, @{src1}, @{src2}"),
        Slti => format!("SLTI   @{dst}, @{src1}, #{imm}"),
        Sltu => format!("SLTU   @{dst}, @{src1}, @{src2}"),
        Sltiu => format!("SLTIU  @{dst}, @{src1}, #{imm}"),
        Sle => format!("SLE    @{dst}, @{src1}, @{src2}"),
        Slei => format!("SLEI   @{dst}, @{src1}, #{imm}"),
        Sleu => format!("SLEU   @{dst}, @{src1}, @{src2}"),
        Sleiu => format!("SLEIU  @{dst}, @{src1}, #{imm}"),

        _ => panic!("Unhandled opcode: {opcode:?}"),
    }
}

/// List of all opcodes to benchmark
fn all_opcodes() -> &'static [Opcode] {
    &[
        Opcode::Xor,
        Opcode::Xori,
        Opcode::B32Mul,
        Opcode::B32Muli,
        Opcode::B128Add,
        Opcode::B128Mul,
        Opcode::Add,
        Opcode::Addi,
        Opcode::Sub,
        Opcode::And,
        Opcode::Andi,
        Opcode::Or,
        Opcode::Ori,
        Opcode::Sll,
        Opcode::Slli,
        Opcode::Srl,
        Opcode::Srli,
        Opcode::Sra,
        Opcode::Srai,
        Opcode::Mul,
        Opcode::Muli,
        Opcode::Mulu,
        Opcode::Mulsu,
        Opcode::Slt,
        Opcode::Slti,
        Opcode::Sltu,
        Opcode::Sltiu,
        Opcode::Sle,
        Opcode::Slei,
        Opcode::Sleu,
        Opcode::Sleiu,
    ]
}

/// Benchmark all opcodes' proving performance
fn bench_all(c: &mut Criterion) {
    let prover = Prover::new(Box::new(GenericISA));
    let mut group = c.benchmark_group("opcode_proving");
    group.sample_size(SAMPLE_SIZE);

    for &opc in all_opcodes() {
        let trace = generate_trace_for(opc, TRACE_LEN);
        group.bench_with_input(BenchmarkId::new("prove", opc), &trace, |b, t| {
            b.iter(|| prover.prove(t))
        });
    }

    group.finish();
}

criterion_group!(benches, bench_all);
criterion_main!(benches);

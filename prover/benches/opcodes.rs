use std::array::from_fn;

use binius_field::{BinaryField, Field};
use binius_m3::builder::B32;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use petravm_asm::opcodes::Opcode;
use petravm_asm::{isa::GenericISA, Assembler};
use petravm_asm::{
    AssembledProgram, Instruction, InterpreterInstruction, Memory, PetraTrace, ValueRom,
};
use petravm_prover::model::Trace;
use petravm_prover::prover::Prover;
use petravm_prover::test_utils::generate_trace;
use rand::{rng, Rng};

const TRACE_LEN: usize = 8_000; // 8K instructions per trace
const SAMPLE_SIZE: usize = 20; // Number of benchmark runs per opcode

fn generate_trace_for_opcode(opcode: Opcode, length: usize) -> Trace {
    let mut rng = rng();
    let imm = rng.random::<u16>();

    let mut asm = Vec::new();

    // ——— Boot: load counter & tail‐call into helper ———
    asm.push("#[framesize(0x4)]".to_owned());
    asm.push("bench:".to_owned());
    asm.push("ALLOCI! @3, #32".to_owned());
    asm.push(format!("LDI.W @2, #{length}G")); // 1
    asm.push("MVV.W @3[2], @2".to_owned()); // 2
    asm.push("TAILI bench_helper, @3".to_owned()); // 3

    // ——— Helper: test counter ———
    asm.push("\n#[framesize(0x20)]".to_owned());
    asm.push("bench_helper:".to_owned());
    asm.push("LDI.W @3, #0G".to_owned()); // 4
    asm.push("XOR   @16, @2, @3".to_owned()); // 5
    asm.push("BNZ   bench_body, @16".to_owned()); // 6
    asm.push("RET".to_owned()); // 7

    // ——— Body: one opcode + loop back ———
    asm.push("bench_body:".to_owned());
    asm.push("B32_MULI @17, @2, #-1G".to_owned()); // 8  (decrement)
                                                   // 8 × LDI.W(@4–@11)
    for reg in 4..=11 {
        let val = rng.random::<u32>();
        asm.push(format!("LDI.W @{reg}, #{val}")); // 9–16
    }
    asm.push(format_instruction(opcode, 12, 4, 8, imm)); // 17 (your opcode)
    asm.push("ALLOCI! @18, #32".to_owned());
    asm.push("MVV.W @18[2], @17".to_owned()); // 18 (re‐package counter)
    asm.push("TAILI bench_helper, @18".to_owned()); // 19 (loop back)

    // Exact exec‐count:
    //     3 + (3 + 12) × (TRACE_LEN − 1) + 4 = 15 × TRACE_LEN − 8 = 120000 − 8 =
    // 119992

    // ——— Emit the trace ———
    let program = asm.join("\n");
    generate_trace(program, None, None).expect("Trace generation failed")
}

fn generate_simple_trace_for_opcode(opcode: Opcode, length: usize) -> (Memory, AssembledProgram) {
    let mut rng = rng();
    let imm = rng.random::<u16>();
    let vals: [u32; 8] = from_fn(|_| rng.random::<u32>());

    let mut asm = Vec::new();

    // ——— Boot: load counter & tail‐call into helper ———
    asm.push("#[framesize(0x10)]".to_owned());
    asm.push("bench:".to_owned());
    for i in 4..12 {
        asm.push(format!("LDI.W @{i}, #{}", vals[i - 4]));
    }

    for _ in 0..length {
        asm.push(format_instruction(opcode, 12, 4, 8, imm)); // 17 (your opcode)
    }
    asm.push("RET".to_owned()); // 18 (re‐package counter)

    // Exact exec‐count:
    //     3 + (3 + 12) × (TRACE_LEN − 1) + 4 = 15 × TRACE_LEN − 8 = 120000 − 8 =
    // 119992

    // ——— Emit the trace ———
    let program = asm.join("\n");

    // Compile the assembly code
    let compiled_program = Assembler::from_code(&program).unwrap();

    // Keep a copy of the program for later
    let mut program = compiled_program.prom.clone();

    // TODO: pad program to 128 instructions required by lookup gadget
    let prom_size = program.len().next_power_of_two().max(128);
    let mut max_pc = program.last().map_or(B32::ZERO, |instr| instr.field_pc);

    for _ in program.len()..prom_size {
        max_pc *= B32::MULTIPLICATIVE_GENERATOR;
        program.push(InterpreterInstruction::new(
            Instruction::default(),
            max_pc,
            None,
            false,
        ));
    }

    // Initialize memory with return PC = 0, return FP = 0 if not provided
    let vrom = ValueRom::new_with_init_vals(&vec![0, 0]);
    let memory = Memory::new(compiled_program.prom.clone(), vrom);

    (memory, compiled_program)

    // generate_trace(program, None, None).expect("Trace generation failed")
}

/// Format a single instruction for benchmarking
fn format_instruction(opcode: Opcode, dst: usize, src1: usize, src2: usize, imm: u16) -> String {
    use Opcode::*;
    match opcode {
        // Binary field operations
        Xor => format!("XOR    @{dst}, @{src1}, @{src2}"),
        Xori => format!("XORI   @{dst}, @{src1}, #{imm}"),
        B32Mul => format!("B32_MUL @{dst}, @{src1}, @{src2}"),
        B32Muli => format!("B32_MULI @{dst}, @{src1}, #{imm}"),
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

        // FP
        Fp => format!("FP   @{dst}, #{imm}"),

        _ => panic!("Unhandled opcode: {opcode:?}"),
    }
}

/// List of all opcodes to benchmark
fn all_opcodes() -> &'static [Opcode] {
    &[
        Opcode::Fp,
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
    group.measurement_time(std::time::Duration::from_secs(20));

    for &opc in all_opcodes() {
        let trace = generate_trace_for_opcode(opc, TRACE_LEN);
        group.bench_with_input(BenchmarkId::new("prove", opc), &trace, |b, t| {
            b.iter(|| prover.prove(t))
        });
    }

    group.finish();
}

/// Benchmark all opcodes' proving performance
fn bench_all_trace(c: &mut Criterion) {
    // let prover = Prover::new(Box::new(GenericISA));
    let mut group = c.benchmark_group("opcode_generation");
    group.sample_size(SAMPLE_SIZE);
    group.measurement_time(std::time::Duration::from_secs(20));

    for &opc in all_opcodes() {
        let (memory, compiled_program) = generate_simple_trace_for_opcode(opc, TRACE_LEN);
        group.bench_with_input(BenchmarkId::new("generate", opc), &(), |b, _| {
            b.iter(|| {
                PetraTrace::generate(
                    Box::new(GenericISA),
                    memory.clone(),
                    compiled_program.frame_sizes.clone(),
                    compiled_program.pc_field_to_index_pc.clone(),
                )
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench_all, bench_all_trace);
criterion_main!(benches);

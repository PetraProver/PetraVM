pub mod common;

use std::array::from_fn;

use binius_field::{BinaryField, Field};
use binius_m3::builder::B32;
use petravm_asm::{
    init_logger,
    isa::GenericISA,
    stats::{all_opcodes, STAT_OP_COUNT},
    Assembler, Instruction, InterpreterInstruction, Memory, Opcode, PetraTrace, ValueRom,
};
use rand::{rng, Rng};
use tracing::info;

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

fn generate_simple_trace_for_opcode(opcode: Opcode, length: usize) -> Vec<(Opcode, f64)> {
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

    let mut cycles = (0..STAT_OP_COUNT)
        .map(|opcode| (all_opcodes()[opcode], 0f64))
        .collect::<Vec<_>>();
    for _ in 0..20 {
        let (_, _, cur_cycles) = PetraTrace::generate_with_cycles(
            Box::new(GenericISA),
            memory.clone(),
            compiled_program.frame_sizes.clone(),
            compiled_program.pc_field_to_index_pc.clone(),
        )
        .unwrap();
        cycles
            .iter_mut()
            .zip(cur_cycles.iter())
            .for_each(|((_, c), &v)| {
                *c += v.1;
            });
    }

    cycles.iter_mut().for_each(|(_, c)| {
        *c /= 20.0; // Average over 20 runs
    });

    cycles
}

#[test]
fn test_multiple_runs() {
    init_logger();

    for index in 0..STAT_OP_COUNT {
        let opcode = all_opcodes()[index];
        let cycles = generate_simple_trace_for_opcode(opcode, 10);

        // Check if the cycles are within a reasonable range
        for (op, cycle_count) in cycles {
            if cycle_count != 0.0 {
                info!("Opcode {:?} executed in {:.2} cycles", op, cycle_count);
            }
        }
    }
}

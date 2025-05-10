use anyhow::Result;
use binius_field::{BinaryField, Field};
use binius_m3::builder::B32;
use log::trace;
use petravm_asm::{
    isa::GenericISA, Assembler, Instruction, InterpreterInstruction, Memory, PetraTrace, ValueRom,
};
use tracing::instrument;

use crate::model::Trace;

pub fn fibonacci(n: u32) -> u32 {
    if n <= 1 {
        return n;
    }
    let (mut a, mut b) = (0u32, 1u32);
    for _ in 0..n {
        let temp = b;
        b = a.wrapping_add(b);
        a = temp;
    }
    a
}

/// Creates an execution trace for a Fibonacci program.
///
/// # Arguments
/// * `n` - The Fibonacci number to calculate.
/// * `res` - The result of the Fibonacci number.
///
/// # Returns
/// * A trace containing the Fibonacci program execution
#[instrument(level = "info", skip(res))]
pub fn generate_fibonacci_trace(n: u32, res: u32) -> Result<Trace> {
    // Read the Fibonacci assembly code from examples directory
    let asm_path = format!("{}/../examples/fib.asm", env!("CARGO_MANIFEST_DIR"));
    let asm_code = std::fs::read_to_string(asm_path)
        .map_err(|e| anyhow::anyhow!("Failed to read fib.asm: {}", e))?;

    let n = B32::MULTIPLICATIVE_GENERATOR.pow([n as u64]).val();
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: n
    // Slot 3: Arg: Result
    let init_values = vec![0, 0, n, res];

    generate_trace(asm_code, Some(init_values), None)
}

pub const fn collatz(mut n: u32) -> usize {
    let mut count = 0;
    while n != 1 {
        if n % 2 == 0 {
            n /= 2;
        } else {
            n = 3 * n + 1;
        }
        count += 1;
    }

    count
}

/// Creates an execution trace for a Collatz program.
///
/// # Arguments
/// * `n` - The number to start the Collatz sequence from.
/// * `res` - The result of the Fibonacci number.
///
/// # Returns
/// * A trace containing the Fibonacci program execution
#[instrument(level = "info", skip_all)]
pub fn generate_collatz_trace(n: u32) -> Result<Trace> {
    // Read the Fibonacci assembly code from examples directory
    let asm_path = format!("{}/../examples/collatz.asm", env!("CARGO_MANIFEST_DIR"));
    let asm_code = std::fs::read_to_string(asm_path)
        .map_err(|e| anyhow::anyhow!("Failed to read collatz.asm: {}", e))?;

    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: n
    let init_values = vec![0, 0, n];

    generate_trace(asm_code, Some(init_values), None)
}

/// Creates an execution trace for the instructions in `asm_code`.
///
/// # Arguments
/// * `asm_code` - The assembly code.
/// * `init_values` - The initial values for the VROM.
/// * `vrom_writes` - The VROM writes to be added to the trace.
///
/// # Returns
/// * A Trace containing executed instructions
pub fn generate_trace(
    asm_code: String,
    init_values: Option<Vec<u32>>,
    vrom_writes: Option<Vec<(u32, u32, u32)>>,
) -> Result<Trace> {
    // Compile the assembly code
    let compiled_program = Assembler::from_code(&asm_code)?;
    trace!("compiled program = {:?}", compiled_program);

    // Keep a copy of the program for later
    let mut program = compiled_program.prom.clone();

    // TODO: pad program to 128 instructions required by lookup gadget
    let prom_size = program.len().next_power_of_two().max(128);
    let mut max_pc = program.last().map_or(B32::ZERO, |instr| instr.field_pc);

    for _ in program.len()..prom_size {
        max_pc *= B32::MULTIPLICATIVE_GENERATOR;
        program.push(InterpreterInstruction::new(Instruction::default(), max_pc));
    }

    // Initialize memory with return PC = 0, return FP = 0 if not provided
    let vrom = ValueRom::new_with_init_vals(&init_values.unwrap_or_else(|| vec![0, 0]));
    let memory = Memory::new(compiled_program.prom, vrom);

    // Generate the trace from the compiled program
    let (petra_trace, _) = PetraTrace::generate(
        Box::new(GenericISA),
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_int,
    )
    .map_err(|e| anyhow::anyhow!("Failed to generate trace: {:?}", e))?;

    // Convert to Trace format for the prover
    let mut zkvm_trace = Trace::from_petra_trace(program, petra_trace);
    let actual_vrom_writes = zkvm_trace.trace.vrom().sorted_access_counts();

    // Validate that manually specified multiplicities match the actual ones if
    // provided.
    if let Some(vrom_writes) = vrom_writes {
        assert_eq!(actual_vrom_writes, vrom_writes);
    }

    // Add other VROM writes
    let mut max_dst = 0;
    // TODO: the lookup gadget requires a minimum of 128 entries
    let vrom_write_size = actual_vrom_writes.len().next_power_of_two().max(128);
    for (dst, val, multiplicity) in actual_vrom_writes {
        zkvm_trace.add_vrom_write(dst, val, multiplicity);
        max_dst = max_dst.max(dst);
    }

    // TODO: we have to add a zero multiplicity entry at the end and pad to 128 due
    // to the bug in the lookup gadget
    for _ in zkvm_trace.vrom_writes.len()..vrom_write_size {
        max_dst += 1;
        zkvm_trace.add_vrom_write(max_dst, 0, 0);
    }

    zkvm_trace.max_vrom_addr = max_dst as usize;
    Ok(zkvm_trace)
}

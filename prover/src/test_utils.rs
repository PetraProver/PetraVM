use anyhow::Result;
use binius_field::{BinaryField, Field};
use binius_m3::builder::B32;
use log::trace;
use petravm_asm::{
    execution::FramePointer, isa::GenericISA, Assembler, Instruction, InterpreterInstruction,
    Memory, PetraTrace, ValueRom,
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

/// Creates an execution trace for the instructions in in the file `file_name`
/// located in the `examples` directory.
///
/// # Arguments
/// * `files` - The names of the assembly files.
/// * `init_values` - The initial values for the VROM.
///
/// # Returns
/// * A trace containing the program execution
pub fn generate_asm_trace(files: &[&str], init_values: Vec<u32>) -> Result<(Trace, FramePointer)> {
    // Read the assembly code from the specified files
    #[allow(clippy::manual_try_fold)]
    let asm_code = files
        .iter()
        .fold(Ok(String::new()), |acc: Result<String>, &file_name| {
            let mut acc = acc?;
            let asm_path = format!("{}/../examples/{}", env!("CARGO_MANIFEST_DIR"), file_name);
            let asm_code = std::fs::read_to_string(asm_path)
                .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", file_name, e))?;
            acc.push_str(&asm_code);
            Ok(acc)
        })?;

    generate_trace(asm_code, Some(init_values), None)
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
pub fn generate_fibonacci_trace(n: u32, res: u32) -> Result<(Trace, FramePointer)> {
    let n = B32::MULTIPLICATIVE_GENERATOR.pow([n as u64]).val();
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: n
    // Slot 3: Arg: Result address
    // Slot 4: Arg: Result
    let init_values = vec![0, 0, n, 4, res];

    generate_asm_trace(&["fib.asm"], init_values)
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
pub fn generate_collatz_trace(n: u32) -> Result<(Trace, FramePointer)> {
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: n
    let init_values = vec![0, 0, n];

    generate_asm_trace(&["collatz.asm"], init_values)
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
/// * The final frame pointer value
pub fn generate_trace(
    asm_code: String,
    init_values: Option<Vec<u32>>,
    vrom_writes: Option<Vec<(u32, u32, u32)>>,
) -> Result<(Trace, petravm_asm::execution::FramePointer)> {
    // Compile the assembly code
    let compiled_program = Assembler::from_code(&asm_code)?;
    trace!("compiled program = {compiled_program:?}");

    // Remove prover-only instructions for the verifier
    let mut verifier_program = compiled_program
        .prom
        .clone()
        .into_iter()
        .filter(|instr| !instr.prover_only)
        .collect::<Vec<_>>();

    // TODO: pad program to 128 instructions required by lookup gadget
    let prom_size = verifier_program.len().next_power_of_two().max(128);
    let mut max_pc = verifier_program
        .last()
        .map_or(B32::ZERO, |instr| instr.field_pc);

    for _ in verifier_program.len()..prom_size {
        max_pc *= B32::MULTIPLICATIVE_GENERATOR;
        verifier_program.push(InterpreterInstruction::new(
            Instruction::default(),
            max_pc,
            None,
            false,
        ));
    }

    // Initialize memory with return PC = 0, return FP = 0 if not provided
    let vrom = ValueRom::new_with_init_vals(&init_values.unwrap_or_else(|| vec![0, 0]));
    let memory = Memory::new(compiled_program.prom, vrom);

    // Generate the trace from the compiled program
    let (petra_trace, boundary_values) = PetraTrace::generate(
        Box::new(GenericISA),
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_index_pc,
    )
    .map_err(|e| anyhow::anyhow!("Failed to generate trace: {:?}", e))?;

    // Convert to Trace format for the prover
    let mut zkvm_trace = Trace::from_petra_trace(verifier_program, petra_trace);
    let actual_vrom_writes = zkvm_trace.trace.vrom().sorted_access_counts();

    // Validate that manually specified multiplicities match the actual ones if
    // provided.
    if let Some(vrom_writes) = vrom_writes {
        assert_eq!(actual_vrom_writes, vrom_writes);
    }

    // Add other VROM writes
    let mut max_dst = 0;
    for (dst, val, multiplicity) in actual_vrom_writes {
        zkvm_trace.add_vrom_write(dst, val, multiplicity);
        max_dst = max_dst.max(dst);
    }

    zkvm_trace.max_vrom_addr = max_dst as usize;
    Ok((zkvm_trace, boundary_values.final_fp))
}

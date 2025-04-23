use std::time::Instant;

use anyhow::Result;
use binius_field::{BinaryField, Field};
use binius_m3::builder::B32;
use zcrayvm_assembly::isa::GenericISA;
use zcrayvm_assembly::{
    Assembler, Instruction, InterpreterInstruction, Memory, ValueRom, ZCrayTrace,
};
use zcrayvm_prover::model::Trace;
use zcrayvm_prover::prover::{verify_proof, Prover};

/// Creates an execution trace for a Fibonacci program.
///
/// # Arguments
/// * `n` - The Fibonacci number to calculate.
/// * `res` - The result of the Fibonacci number.
///
/// # Returns
/// * A trace containing the Fibonacci program execution
fn generate_fibonacci_trace(n: u32, res: u32) -> Result<Trace> {
    // Read the Fibonacci assembly code from examples directory
    let asm_code = std::fs::read_to_string("../examples/fib.asm")
        .map_err(|e| anyhow::anyhow!("Failed to read fib.asm: {}", e))?;

    let n = B32::MULTIPLICATIVE_GENERATOR.pow([n as u64]).val();
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: n
    // Slot 3: Arg: Result
    let init_values = [0, 0, n, res];

    generate_test_trace(asm_code, init_values)
}

/// Creates an execution trace for the instructions in `asm_code`.
///
/// # Arguments
/// * `asm_code` - The assembly code.
/// * `init_values` - The initial values for the VROM.
///
/// # Returns
/// * A Trace containing executed instructions
fn generate_test_trace<const N: usize>(asm_code: String, init_values: [u32; N]) -> Result<Trace> {
    // Compile the assembly code
    let compiled_program = Assembler::from_code(&asm_code)?;

    // Keep a copy of the program for later
    let mut program = compiled_program.prom.clone();

    // Pad program to 128 instructions required by lookup gadget
    let prom_size = program.len().next_power_of_two().max(128);
    let mut max_pc = program.last().map_or(B32::ZERO, |instr| instr.field_pc);

    for _ in program.len()..prom_size {
        max_pc *= B32::MULTIPLICATIVE_GENERATOR;
        program.push(InterpreterInstruction::new(Instruction::default(), max_pc));
    }

    // Initialize memory with return PC = 0, return FP = 0
    let vrom = ValueRom::new_with_init_vals(&init_values);
    let memory = Memory::new(compiled_program.prom, vrom);

    // Generate the trace from the compiled program
    let (zcray_trace, _) = ZCrayTrace::generate(
        Box::new(GenericISA),
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_int,
    )
    .map_err(|e| anyhow::anyhow!("Failed to generate trace: {:?}", e))?;

    // Convert to Trace format for the prover
    let mut zkvm_trace = Trace::from_zcray_trace(program, zcray_trace);

    // Get the VROM writes from the trace
    let vrom_writes = zkvm_trace.trace.vrom().sorted_access_counts();

    // Add other VROM writes
    let mut max_dst = 0;
    // The lookup gadget requires a minimum of 128 entries
    let vrom_write_size = vrom_writes.len().next_power_of_two().max(128);
    for (dst, val, multiplicity) in vrom_writes {
        zkvm_trace.add_vrom_write(dst, val, multiplicity);
        max_dst = max_dst.max(dst);
    }

    // Add a zero multiplicity entry at the end and pad to 128 due
    // to the requirements in the lookup gadget
    for _ in zkvm_trace.vrom_writes.len()..vrom_write_size {
        max_dst += 1;
        zkvm_trace.add_vrom_write(max_dst, 0, 0);
    }

    zkvm_trace.max_vrom_addr = max_dst as usize;
    Ok(zkvm_trace)
}

fn fibonacci(n: u32) -> u32 {
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

#[test]
fn test_fibonacci() -> Result<()> {
    // Step 1: Generate trace
    let n = 11;
    let res = fibonacci(n);

    let start = Instant::now();
    let trace = generate_fibonacci_trace(n, res)?;
    let trace_time = start.elapsed();
    println!("Trace generation time: {:?}", trace_time);

    // Step 2: Validate trace
    trace.validate()?;

    // Step 3: Create prover
    let prover = Prover::new(Box::new(GenericISA));

    // Step 4: Generate proof
    let start = Instant::now();
    let (proof, statement, compiled_cs) = prover.prove(&trace)?;
    let proving_time = start.elapsed();
    println!("Proof generation time: {:?}", proving_time);

    // Step 5: Verify proof
    let start = Instant::now();
    verify_proof(&statement, &compiled_cs, proof)?;
    let verification_time = start.elapsed();
    println!("Proof verification time: {:?}", verification_time);

    Ok(())
}

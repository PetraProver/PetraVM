//! Test the zCrayVM proving system with LDI and RET instructions.
//!
//! This file contains an integration test that verifies the complete
//! proving system pipeline from assembly to proof verification.

use anyhow::Result;
use log::trace;
use zcrayvm_assembly::{Assembler, Memory, ValueRom, ZCrayTrace};
use zcrayvm_prover::model::Trace;
use zcrayvm_prover::prover::{verify_proof, Prover};

/// Creates an execution trace for the instructions in `asm_code`.
///
/// # Arguments
/// * `asm_code` - The assembly code.
/// * `init_values` - The initial values for the VROM.
/// * `vrom_writes` - The VROM writes to be added to the trace.
///
/// # Returns
/// * A Trace containing executed instructions that loads `value` into VROM at
///   address fp+2, followed by a RET instruction
fn generate_test_trace<const N: usize, F: FnOnce(&Trace) -> Vec<(u32, u32)>>(
    asm_code: String,
    init_values: [u32; N],
    vrom_writes: F,
) -> Result<Trace> {
    // Compile the assembly code
    let compiled_program = Assembler::from_code(&asm_code)?;
    trace!("compiled program = {:?}", compiled_program);

    // Keep a copy of the program for later
    let program = compiled_program.prom.clone();

    // Initialize memory with return PC = 0, return FP = 0
    let vrom = ValueRom::new_with_init_vals(&init_values);
    let memory = Memory::new(compiled_program.prom, vrom);

    // Generate the trace from the compiled program
    let (zcray_trace, _) = ZCrayTrace::generate(
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_int,
    )
    .map_err(|e| anyhow::anyhow!("Failed to generate trace: {:?}", e))?;

    // Convert to Trace format for the prover
    let mut zkvm_trace = Trace::from_zcray_trace(zcray_trace);

    // Add the program instructions to the trace
    zkvm_trace.add_instructions(program);

    let vrom_writes: Vec<_> = vrom_writes(&zkvm_trace);

    // Add initial VROM values for return PC and return FP
    zkvm_trace.add_vrom_write(0, 0); // Initial return PC = 0
    zkvm_trace.add_vrom_write(1, 0); // Initial return FP = 0

    // Add other VROM writes
    for (dst, imm) in vrom_writes {
        zkvm_trace.add_vrom_write(dst, imm);
    }

    dbg!(&zkvm_trace);

    Ok(zkvm_trace)
}

fn generate_ldi_ret_trace(value: u32) -> Result<Trace> {
    // Create a simple assembly program with LDI and RET
    // Note: Format follows the grammar requirements:
    // - Program must start with a label followed by an instruction
    // - Used framesize for stack allocation
    let asm_code = format!(
        "#[framesize(0x10)]\n\
         _start: LDI.W @2, #{}\n\
         RET\n",
        value
    );

    // Initialize memory with return PC = 0, return FP = 0
    let init_values = [0, 0, value];

    // Add VROM writes from LDI eventsff
    let vrom_writes = |zkvm_trace: &Trace| {
        zkvm_trace
            .ldi_events()
            .iter()
            .map(|event| (event.dst as u32, event.imm))
            .collect()
    };

    generate_test_trace(asm_code, init_values, vrom_writes)
}

#[test]
fn test_zcrayvm_proving_pipeline() -> Result<()> {
    env_logger::init();

    // Test value to load
    let value = 0x12345678;

    // Step 1: Generate trace from assembly
    trace!("Generating trace from assembly...");
    let trace = generate_ldi_ret_trace(value)?;

    // Verify trace has correct structure
    assert_eq!(
        trace.program.len(),
        2,
        "Program should have exactly 2 instructions"
    );
    assert_eq!(
        trace.ldi_events().len(),
        1,
        "Should have exactly one LDI event"
    );
    assert_eq!(
        trace.ret_events().len(),
        1,
        "Should have exactly one RET event"
    );
    assert_eq!(trace.vrom_writes.len(), 3, "Should have three VROM writes");

    // Step 2: Validate trace
    trace!("Validating trace internal structure...");
    trace.validate()?;

    // Step 3: Create prover
    trace!("Creating prover...");
    let prover = Prover::new();

    // Step 4: Generate proof
    trace!("Generating proof...");
    let (proof, statement, compiled_cs) = prover.prove(&trace)?;

    // Step 5: Verify proof
    trace!("Verifying proof...");
    verify_proof(&statement, &compiled_cs, proof)?;

    trace!("All steps completed successfully!");
    Ok(())
}

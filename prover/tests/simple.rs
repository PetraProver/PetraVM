//! Test the zCrayVM proving system with LDI and RET instructions.
//!
//! This file contains an integration test that verifies the complete
//! proving system pipeline from assembly to proof verification.

use anyhow::Result;
use binius_m3::builder::B32;
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
/// * A Trace containing executed instructions
// TODO: we should extract VROM writes from zcray_trace
fn generate_test_trace<const N: usize>(
    asm_code: String,
    init_values: [u32; N],
    vrom_writes: Vec<(u32, u32, u32)>,
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

    // Add other VROM writes
    let mut max_dst = 0;
    for (dst, imm, multiplicity) in vrom_writes {
        zkvm_trace.add_vrom_write(dst, imm, multiplicity);
        max_dst = max_dst.max(dst);
    }

    // TODO: we have to add a zero multiplicity entry due to the bug in the lookup
    // gadget
    zkvm_trace.add_vrom_write(max_dst + 1, 0, 0);

    Ok(zkvm_trace)
}

/// Creates a basic execution trace with just LDI, B32_MUL and RET instructions.
///
/// # Arguments
/// * `value` - The value to load into VROM.
///
/// # Returns
/// * A trace containing an LDI, B32_MUL and RET instruction
fn generate_ldi_ret_mul32_trace(value: u32) -> Result<Trace> {
    // Create a simple assembly program with LDI and RET
    // Note: Format follows the grammar requirements:
    // - Program must start with a label followed by an instruction
    // - Used framesize for stack allocation
    let asm_code = format!(
        "#[framesize(0x10)]\n\
         _start:
           LDI.W @2, #{}\n\
           LDI.W @3, #2\n\
           B32_MUL @4, @2, @3\n\
           RET\n",
        value
    );

    // Initialize memory with return PC = 0, return FP = 0
    let init_values = [0, 0];

    let mul_result = (B32::new(value) * B32::new(2)).val();
    let vrom_writes = vec![
        // LDI events
        (2, value, 2),
        (3, 2, 2),
        // Initial values
        (0, 0, 1),
        (1, 0, 1),
        // B32_MUL event
        (4, mul_result, 1),
    ];

    generate_test_trace(asm_code, init_values, vrom_writes)
}

/// Creates a basic execution trace with just BNZ and RET instructions.
///
/// # Arguments
/// * `con_val` - The condition checked by the BNZ instruction
///
/// # Returns
/// * A Trace containing an BNZ instruction that loads `value` into VROM at
///   address fp+2, followed by a RET instruction
fn generate_bnz_ret_trace(cond_val: u32) -> Result<Trace> {
    // Create a simple assembly program with LDI and RET
    // Note: Format follows the grammar requirements:
    // - Program must start with a label followed by an instruction
    // - Used framesize for stack allocation
    let asm_code = "#[framesize(0x10)]\n\
        _start:\n\
            BNZ ret, @2 \n\
        ret:\n\
            RET\n"
        .to_string();

    trace!("asm_code:\n {:?}", asm_code);

    let init_values = [0, 0, cond_val];

    // Add VROM writes from BNZ events
    let vrom_writes = if cond_val != 0 {
        vec![
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            (2, 1, 1),
        ]
    } else {
        vec![
            // Initial values
            (0, 0, 1),
            (1, 0, 1),
            (2, 0, 1),
        ]
    };

    generate_test_trace(asm_code, init_values, vrom_writes)
}

/// Creates an execution trace for a simple program that uses only LDI.W, MVV.W,
/// BNZ, TAILI, and RET.
///
/// # Returns
/// * A Trace containing a simple program with a loop using TAILI
fn generate_simple_taili_trace() -> Result<Trace> {
    // Create a very simple assembly program that:
    // 1. Sets up a counter in @2 (starts at 2)
    // 2. BNZ executes in loop to check if counter is non-zero
    // 3. Loop decrements counter and is implemented with TAILI
    let asm_code = "#[framesize(0x10)]\n\
         _start:
           LDI.W @2, #2\n\
         loop:
           BNZ continue, @2\n\
           RET\n\
         continue:
           LDI.W @3, #0\n\
           MVV.W @4[2], @3\n\
           TAILI loop, @4\n"
        .to_string();

    // Initialize memory with return PC = 0, return FP = 0
    let init_values = [0, 0];

    // Build the vector of VROM writes
    let vrom_writes = vec![
        // Initial values
        (0, 0, 1), // Return PC
        (1, 0, 1), // Return FP
        // Initial LDI events
        (2, 2, 1), // counter = 2
        // First iteration
        (3, 0, 1), // LDI.W @3, #0
        (2, 0, 1), /* MVV.W @4[2], @3 - Write 0 to location 2
                    * Second iteration
                    * No more iterations because counter became 0 */
    ];

    generate_test_trace(asm_code, init_values, vrom_writes)
}

fn test_from_trace_generator<F, G>(
    trace_generator: F,
    check_events: G,
    n_vrom_writes: usize,
) -> Result<()>
where
    F: FnOnce() -> Result<Trace>,
    G: FnOnce(&Trace),
{
    // Step 1: Generate trace
    let trace = trace_generator()?;
    // Verify trace has correct structure
    check_events(&trace);

    assert_eq!(
        trace.vrom_writes.len(),
        n_vrom_writes,
        "Should have {} VROM writes",
        n_vrom_writes
    );

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

#[test]
fn test_ldi_b32_mul_ret() -> Result<()> {
    test_from_trace_generator(
        || {
            // Test value to load
            let value = 0x12345678;
            generate_ldi_ret_mul32_trace(value)
        },
        |trace| {
            assert_eq!(
                trace.program.len(),
                4,
                "Program should have exactly 4 instructions"
            );
            assert_eq!(
                trace.ldi_events().len(),
                2,
                "Should have exactly two LDI events"
            );
            assert_eq!(
                trace.ret_events().len(),
                1,
                "Should have exactly one RET event"
            );
            assert_eq!(
                trace.b32_mul_events().len(),
                1,
                "Should have exactly one B32_MUL event"
            );
        },
        6,
    )
}

#[test]
fn test_bnz_non_zero_branch_ret() -> Result<()> {
    test_from_trace_generator(
        || generate_bnz_ret_trace(1),
        |trace| {
            assert_eq!(
                trace.program.len(),
                2,
                "Program should have exactly 2 instructions"
            );
            assert_eq!(
                trace.bnz_events().len(),
                1,
                "Should have exactly one LDI event"
            );
            assert_eq!(
                trace.ret_events().len(),
                1,
                "Should have exactly one RET event"
            );
        },
        4,
    )
}

#[test]
fn test_bnz_zero_branch_ret() -> Result<()> {
    test_from_trace_generator(
        || generate_bnz_ret_trace(0),
        |trace| {
            assert_eq!(
                trace.program.len(),
                2,
                "Program should have exactly 2 instructions"
            );
            assert_eq!(
                trace.bz_events().len(),
                1,
                "Should have exactly one bz event"
            );
            assert_eq!(
                trace.ret_events().len(),
                1,
                "Should have exactly one RET event"
            );
        },
        4,
    )
}

#[test]
fn test_simple_taili_loop() -> Result<()> {
    test_from_trace_generator(
        generate_simple_taili_trace,
        |trace| {
            // Verify the program has expected number of instructions
            assert!(
                trace.program.len() == 7,
                "Program should have 7 instructions"
            );

            // Verify we have the expected BNZ operations
            let bnz_events = trace.bnz_events();
            assert_eq!(
                bnz_events.len(),
                1,
                "Should have one BNZ event that's taken"
            );

            // Verify we have one RET event
            assert_eq!(
                trace.ret_events().len(),
                1,
                "Should have exactly 1 RET event"
            );
        },
        5, // 5 VROM writes total
    )
}

//! Test the zCrayVM proving system with LDI and RET instructions.
//!
//! This file contains an integration test that verifies the complete
//! proving system pipeline from assembly to proof verification.

use anyhow::Result;
use binius_m3::builder::B32;
use log::trace;
use zcrayvm_assembly::isa::GenericISA;
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
        Box::new(GenericISA),
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_int,
    )
    .map_err(|e| anyhow::anyhow!("Failed to generate trace: {:?}", e))?;

    dbg!(&zcray_trace);

    // Convert to Trace format for the prover
    let mut zkvm_trace = Trace::from_zcray_trace(program, zcray_trace);

    // Add other VROM writes
    let mut max_dst = 0;
    // TODO: the lookup gadget requires a minimum of 128 entries
    let vrom_write_size = vrom_writes.len().next_power_of_two().max(128);
    for (dst, imm, multiplicity) in vrom_writes {
        zkvm_trace.add_vrom_write(dst, imm, multiplicity);
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

/// Creates an execution trace for a simple program that uses only MVI.W,
/// BNZ, TAILI, and RET.
///
/// # Returns
/// * A Trace containing a simple program with a loop using TAILI
fn generate_simple_taili_trace() -> Result<Trace> {
    // Create a very simple assembly program that:
    // 1. _start sets up initial values and tail calls to loop
    // 2. loop checks if @2 is non-zero and either returns or continues
    // 3. case_recurse tail calls back to loop
    let asm_code = "#[framesize(0x10)]\n\
         _start:\n\
           LDI.W @2, #2\n\
           MVV.W @3[2], @2\n\
           TAILI loop, @3\n\
         #[framesize(0x10)]\n\
         loop:\n\
           BNZ case_recurse, @2\n\
           RET\n\
         case_recurse:\n\
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
        // Initial LDI event
        (2, 2, 1), // LDI.W @2, #2
        // Initial MVV.W event
        (5, 2, 1), // MVV.W @3[2], @2 - Write 2 to location 5 (FP 3 + offset 2)
        // LDI.W in case_recurse
        (3, 0, 1), // LDI.W @3, #0 - Set @3 to 0
        // Additional MVV.W in case_recurse
        (6, 0, 1), // MVV.W @4[2], @3 - Write 0 to location 6 (FP 4 + offset 2)
    ];

    generate_test_trace(asm_code, init_values, vrom_writes)
}

fn test_from_trace_generator<F, G>(trace_generator: F, check_events: G) -> Result<()>
where
    F: FnOnce() -> Result<Trace>,
    G: FnOnce(&Trace),
{
    // Step 1: Generate trace
    let trace = trace_generator()?;
    // Verify trace has correct structure
    check_events(&trace);

    // Step 2: Validate trace
    trace!("Validating trace internal structure...");
    trace.validate()?;

    // Step 3: Create prover
    trace!("Creating prover...");
    let prover = Prover::new(Box::new(GenericISA));

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
    )
}

#[test]
fn test_bnz_zero_branch_ret() -> Result<()> {
    test_from_trace_generator(
        || generate_bnz_ret_trace(0),
        |trace| {
            assert_eq!(
                trace.program.len(),
                4,
                "Program should have exactly 4 instructions"
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
    )
}

#[test]
fn test_simple_taili_loop() -> Result<()> {
    test_from_trace_generator(generate_simple_taili_trace, |trace| {
        // Verify exact number of instructions (easier to maintain)
        assert_eq!(
            trace.program.len(),
            8,
            "Program should have exactly 8 instructions"
        );

        // Verify we have one LDI event (for @2 initialization)
        assert_eq!(
            trace.ldi_events().len(),
            2,
            "Should have exactly two LDI events"
        );

        // Verify we have one BNZ event (first is taken, continues to case_recurse)
        let bnz_events = trace.bnz_events();
        assert_eq!(bnz_events.len(), 1, "Should have exactly one BNZ event");

        // Verify we have one RET event (after counter becomes 0)
        assert_eq!(
            trace.ret_events().len(),
            1,
            "Should have exactly one RET event"
        );

        // Verify there are no B32_MUL operations (we aren't using them)
        assert_eq!(
            trace.b32_mul_events().len(),
            0,
            "Should have no B32_MUL events"
        );
    })
}

#[test]
fn test_simple_taili_loop() -> Result<()> {
    test_from_trace_generator(generate_simple_taili_trace, |trace| {
        // Verify exact number of instructions (easier to maintain)
        assert_eq!(
            trace.program.len(),
            8,
            "Program should have exactly 8 instructions"
        );

        // Verify we have one LDI event (for @2 initialization)
        assert_eq!(
            trace.ldi_events().len(),
            2,
            "Should have exactly two LDI events"
        );

        // Verify we have one BNZ event (first is taken, continues to case_recurse)
        let bnz_events = trace.bnz_events();
        assert_eq!(bnz_events.len(), 1, "Should have exactly one BNZ event");

        // Verify we have one RET event (after counter becomes 0)
        assert_eq!(
            trace.ret_events().len(),
            1,
            "Should have exactly one RET event"
        );

        // Verify there are no B32_MUL operations (we aren't using them)
        assert_eq!(
            trace.b32_mul_events().len(),
            0,
            "Should have no B32_MUL events"
        );
    })
}

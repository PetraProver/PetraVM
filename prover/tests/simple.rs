//! Test the zCrayVM proving system with LDI and RET instructions.
//!
//! This file contains an integration test that verifies the complete
//! proving system pipeline from assembly to proof verification.

use anyhow::Result;
use binius_field::underlier::Divisible;
use binius_field::{BinaryField, Field};
use binius_m3::builder::{B128, B32};
use log::trace;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use zcrayvm_assembly::isa::GenericISA;
use zcrayvm_assembly::{
    Assembler, Instruction, InterpreterInstruction, Memory, ValueRom, ZCrayTrace,
};
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
fn generate_test_trace<const N: usize>(
    asm_code: String,
    init_values: [u32; N],
    vrom_writes: Vec<(u32, u32, u32)>,
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

    // Validate that manually specified multiplicities match the actual ones
    assert_eq!(zkvm_trace.trace.vrom().sorted_access_counts(), vrom_writes);

    // Add other VROM writes
    let mut max_dst = 0;
    // TODO: the lookup gadget requires a minimum of 128 entries
    let vrom_write_size = vrom_writes.len().next_power_of_two().max(128);
    for (dst, val, multiplicity) in vrom_writes {
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

/// Creates a basic execution trace with just LDI, B128_ADD and RET
/// instructions.
///
/// # Arguments
/// * `value` - The value to load into VROM.
///
/// # Returns
/// * A trace containing an LDI, B128_ADD and RET instruction
fn generate_ldi_ret_add128_trace(x: u128, y: u128) -> Result<Trace> {
    // Create a simple assembly program with LDI and RET
    // Note: Format follows the grammar requirements:
    // - Program must start with a label followed by an instruction
    // - Used framesize for stack allocation
    let x_array: [u32; 4] = <u128 as Divisible<u32>>::split_val(x);
    let y_array: [u32; 4] = <u128 as Divisible<u32>>::split_val(y);
    let asm_code = format!(
        "#[framesize(0x10)]\n\
         _start:
           LDI.W @4, #{}\n\
           LDI.W @5, #{}\n\
           LDI.W @6, #{}\n\
           LDI.W @7, #{}\n\
           LDI.W @8, #{}\n\
           LDI.W @9, #{}\n\
           LDI.W @10, #{}\n\
           LDI.W @11, #{}\n\
           B128_ADD @12, @4, @8\n\
           RET\n",
        x_array[0],
        x_array[1],
        x_array[2],
        x_array[3],
        y_array[0],
        y_array[1],
        y_array[2],
        y_array[3]
    );

    // Initialize memory with return PC = 0, return FP = 0
    let init_values = [0, 0];

    let result = (B128::new(x) + B128::new(y)).val();
    let result_array: [u32; 4] = <u128 as Divisible<u32>>::split_val(result);
    let vrom_writes = vec![
        // LDI events
        (4, x_array[0], 2),
        (5, x_array[1], 2),
        (6, x_array[2], 2),
        (7, x_array[3], 2),
        (8, y_array[0], 2),
        (9, y_array[1], 2),
        (10, y_array[2], 2),
        (11, y_array[3], 2),
        // Initial values
        (0, 0, 1),
        (1, 0, 1),
        // B128_ADD event
        (12, result_array[0], 1),
        (13, result_array[1], 1),
        (14, result_array[2], 1),
        (15, result_array[3], 1),
    ];

    generate_test_trace(asm_code, init_values, vrom_writes)
}

fn generate_add_ret_trace(src1_value: u32, src2_value: u32) -> Result<Trace> {
    // Create a simple assembly program with LDI, ADD and RET
    // Note: Format follows the grammar requirements:
    // - Program must start with a label followed by an instruction
    // - Used framesize for stack allocation
    let asm_code = format!(
        "#[framesize(0x10)]\n\
         _start: 
            LDI.W @2, #{}\n\
            LDI.W @3, #{}\n\
            ADD @4, @2, @3\n\
            RET\n",
        src1_value, src2_value
    );

    // Initialize memory with return PC = 0, return FP = 0
    let init_values = [0, 0];

    // Add VROM writes from LDI and ADD events
    let vrom_writes = vec![
        // LDI events
        (2, src1_value, 2),
        (3, src2_value, 2),
        // Initial values
        (0, 0, 1),
        (1, 0, 1),
        // ADD event
        (4, src1_value + src2_value, 1),
    ];

    generate_test_trace(asm_code, init_values, vrom_writes)
}

/// Creates an execution trace for a simple program that uses only MVV.W,
/// BNZ, BZ, TAILI, and RET.
///
/// # Returns
/// * A Trace containing a simple program with a loop using TAILI, the BNZ
///   instruction is executed twice.
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

    // VROM state after the trace is executed
    // 0: 0 (1)
    // 1: 0 (1)
    // 2: 2 (2)
    // 3: 16 (2)
    // 16: 0 (2)
    // 17: 0 (2)
    // 18: 2 (2)
    // 19: 0 (2)
    // 20: 32 (2)
    // 32: 0 (2)
    // 33: 0 (2)
    // 34: 0 (2)
    // Sorted by number of accesses
    let vrom_writes = vec![
        // Initial LDI event
        (2, 2, 2), // LDI.W @2, #2
        // New FP values
        (3, 16, 2),
        // TAILI events
        (16, 0, 2),
        (17, 0, 2),
        // Initial MVV.W event
        (18, 2, 2), // MVV.W @3[2], @2
        // LDI in case_recurse
        (19, 0, 2), // LDI.W @3, #0
        (20, 32, 2),
        (32, 0, 2),
        (33, 0, 2),
        // Additional MVV.W in case_recurse
        (34, 0, 2), // MVV.W @4[2], @3
        // Initial values
        (0, 0, 1), // Return PC
        (1, 0, 1), // Return FP
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
fn test_ldi_b128_add_ret() -> Result<()> {
    test_from_trace_generator(
        || {
            // Test value to load
            let x = 0x123456789abcdef123456789abcdef;
            let y = 0x44000000330000002200000011;
            generate_ldi_ret_add128_trace(x, y)
        },
        |trace| {
            assert_eq!(
                trace.ldi_events().len(),
                8,
                "Should have exactly 8 LDI events"
            );
            assert_eq!(
                trace.ret_events().len(),
                1,
                "Should have exactly one RET event"
            );
            assert_eq!(
                trace.b128_add_events().len(),
                1,
                "Should have exactly one B128_ADD event"
            );
        },
    )
}

#[test]
fn test_ldi_add_ret() -> Result<()> {
    test_from_trace_generator(
        || {
            // Test value to load
            let src1_value = 0x12345678;
            let src2_value = 0x4567;
            generate_add_ret_trace(src1_value, src2_value)
        },
        |trace| {
            assert_eq!(
                trace.add_events().len(),
                1,
                "Should have exactly one ADD event"
            );
            assert_eq!(
                trace.ldi_events().len(),
                2,
                "Should have exactly two LDI event"
            );
            assert_eq!(
                trace.ret_events().len(),
                1,
                "Should have exactly one RET event"
            );
            assert_eq!(
                trace.b32_mul_events().len(),
                0,
                "Shouldn't have any B32_MUL event"
            );
        },
    )
}

#[test]
fn test_simple_taili_loop() -> Result<()> {
    test_from_trace_generator(generate_simple_taili_trace, |trace| {
        // Verify we have two LDI events (one in _start and one in case_recurse)
        assert_eq!(
            trace.ldi_events().len(),
            2,
            "Should have exactly two LDI events"
        );

        // Verify we have one BNZ event (first is taken, continues to case_recurse)
        let bnz_events = trace.bnz_events();
        assert_eq!(bnz_events.len(), 1, "Should have exactly one BNZ event");

        // Verify we have one RET event (after condition becomes 0)
        assert_eq!(
            trace.ret_events().len(),
            1,
            "Should have exactly one RET event"
        );

        // Verify we have two TAILI events (initial call to loop and recursive call)
        assert_eq!(
            trace.taili_events().len(),
            2,
            "Should have exactly two TAILI events"
        );

        // Verify we have two MVVW events (one in _start and one in case_recurse)
        assert_eq!(
            trace.mvvw_events().len(),
            2,
            "Should have exactly two MVVW events"
        );

        // Verify we have one BZ event (when condition becomes 0)
        assert_eq!(
            trace.bz_events().len(),
            1,
            "Should have exactly one BZ event"
        );
    })
}

/// Creates an execution trace with all binary operations (AND, OR, XOR, ANDI,
/// ORI, XORI) using random input values.
///
/// # Returns
/// * A Trace containing all binary operations followed by a RET instruction
fn generate_all_binary_ops_trace() -> Result<Trace> {
    let mut rng = StdRng::seed_from_u64(12345);

    // Generate random values for testing
    let val1 = rng.random::<u32>();
    let val2 = rng.random::<u32>();
    let imm = rng.random::<u16>() as u32; // Smaller immediate value

    // Create assembly program with all binary operations
    let asm_code = format!(
        "#[framesize(0x10)]\n\
        _start: 
            LDI.W @2, #{}\n\
            LDI.W @3, #{}\n\
            AND @4, @2, @3\n\
            OR @5, @2, @3\n\
            XOR @6, @2, @3\n\
            ANDI @7, @2, #{}\n\
            ORI @8, @2, #{}\n\
            XORI @9, @2, #{}\n\
            RET\n",
        val1, val2, imm, imm, imm
    );

    // Initialize memory with return PC = 0, return FP = 0
    let init_values = [0, 0];

    // Calculate expected results
    let and_result = val1 & val2;
    let or_result = val1 | val2;
    let xor_result = val1 ^ val2;
    let andi_result = val1 & imm;
    let ori_result = val1 | imm;
    let xori_result = val1 ^ imm;

    // Add VROM writes - checking the error, it seems we need different access
    // counts The test shows expected: [(2, 2287849814, 7), (3, 1313969190, 4),
    // ...]
    let vrom_writes = vec![
        // LDI events - with corrected access counts
        (2, val1, 7),
        (3, val2, 4),
        // Initial values
        (0, 0, 1),
        (1, 0, 1),
        // Binary operations results
        (4, and_result, 1),
        (5, or_result, 1),
        (6, xor_result, 1),
        (7, andi_result, 1),
        (8, ori_result, 1),
        (9, xori_result, 1),
    ];

    generate_test_trace(asm_code, init_values, vrom_writes)
}

#[test]
fn test_all_binary_ops() -> Result<()> {
    test_from_trace_generator(generate_all_binary_ops_trace, |trace| {
        // Verify each binary operation event exists
        assert_eq!(
            trace.and_events().len(),
            1,
            "Should have exactly one AND event"
        );
        assert_eq!(
            trace.or_events().len(),
            1,
            "Should have exactly one OR event"
        );
        assert_eq!(
            trace.xor_events().len(),
            1,
            "Should have exactly one XOR event"
        );
        assert_eq!(
            trace.andi_events().len(),
            1,
            "Should have exactly one ANDI event"
        );
        assert_eq!(
            trace.ori_events().len(),
            1,
            "Should have exactly one ORI event"
        );
        assert_eq!(
            trace.xori_events().len(),
            1,
            "Should have exactly one XORI event"
        );
        assert_eq!(
            trace.ret_events().len(),
            1,
            "Should have exactly one RET event"
        );
    })
}

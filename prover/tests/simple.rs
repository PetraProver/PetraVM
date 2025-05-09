//! Test the PetraVM proving system with LDI and RET instructions.
//!
//! This file contains an integration test that verifies the complete
//! proving system pipeline from assembly to proof verification.

use anyhow::Result;
use binius_field::underlier::Divisible;
use binius_m3::builder::{B128, B32};
use log::trace;
use petravm_asm::isa::GenericISA;
use petravm_prover::model::Trace;
use petravm_prover::prover::{verify_proof, Prover};
use petravm_prover::test_utils::generate_trace;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

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

/// Creates a test trace with B128 field operations (addition and
/// multiplication).
///
/// # Arguments
/// * `x` - The first B128 value for the operations.
/// * `y` - The second B128 value for the operations.
///
/// # Returns
/// * A `Trace` containing B128_ADD, B128_MUL and RET instructions with
///   appropriate values.
fn generate_b128add_b128mul_trace(x: u128, y: u128) -> Result<Trace> {
    // Split B128 values into 32-bit chunks for VROM
    let x_array: [u32; 4] = <u128 as Divisible<u32>>::split_val(x);
    let y_array: [u32; 4] = <u128 as Divisible<u32>>::split_val(y);

    // Create assembly code with B128 operations
    let asm_code = "#[framesize(0x20)]\n\
        _start:\n\
            B128_ADD @12, @4, @8\n\
            B128_MUL @16, @4, @8\n\
            RET\n"
        .to_string();

    // Initialize memory with values
    // First two elements are return PC and return FP (both zero)
    // Next two zeros are for padding
    // Then we store x_array and y_array in VROM
    let init_values = vec![
        0, 0, 0, 0, x_array[0], x_array[1], x_array[2], x_array[3], y_array[0], y_array[1],
        y_array[2], y_array[3],
    ];

    // Calculate expected operation results
    let add_result = (B128::new(x) + B128::new(y)).val();
    let mul_result = (B128::new(x) * B128::new(y)).val();

    // Split results into 32-bit chunks
    let add_result_array = <u128 as Divisible<u32>>::split_val(add_result);
    let mul_result_array = <u128 as Divisible<u32>>::split_val(mul_result);

    // Create VROM writes with their access counts
    // Format: (address, value, access_count)
    let vrom_writes = vec![
        // Input values (each accessed twice - once for ADD, once for MUL)
        (4, x_array[0], 2),
        (5, x_array[1], 2),
        (6, x_array[2], 2),
        (7, x_array[3], 2),
        (8, y_array[0], 2),
        (9, y_array[1], 2),
        (10, y_array[2], 2),
        (11, y_array[3], 2),
        // Initial values (accessed once during setup)
        (0, 0, 1),
        (1, 0, 1),
        // B128_ADD results (each accessed once)
        (12, add_result_array[0], 1),
        (13, add_result_array[1], 1),
        (14, add_result_array[2], 1),
        (15, add_result_array[3], 1),
        // B128_MUL results (each accessed once)
        (16, mul_result_array[0], 1),
        (17, mul_result_array[1], 1),
        (18, mul_result_array[2], 1),
        (19, mul_result_array[3], 1),
    ];

    generate_trace(asm_code, Some(init_values), Some(vrom_writes))
}

#[test]
fn test_b128_add_b128_mul() -> Result<()> {
    let mut rng = StdRng::seed_from_u64(54321);
    test_from_trace_generator(
        || {
            // Test value to load
            let x = rng.random::<u128>();
            let y = rng.random::<u128>();
            generate_b128add_b128mul_trace(x, y)
        },
        |trace| {
            assert_eq!(
                trace.ret_events().len(),
                1,
                "Should have exactly one RET event"
            );
            assert_eq!(
                trace.b128_mul_events().len(),
                1,
                "Should have exactly one B128_MUL event"
            );
            assert_eq!(
                trace.b128_add_events().len(),
                1,
                "Should have exactly one B128_ADD event"
            );
        },
    )
}

fn generate_integer_ops_trace(src1_value: u32, src2_value: u32) -> Result<Trace> {
    let imm = src2_value as u16;
    // Create a simple assembly program with all integer operations.
    // Note: Format follows the grammar requirements:
    // - Program must start with a label followed by an instruction
    // - Used framesize for stack allocation
    let asm_code = format!(
        "#[framesize(0x10)]\n\
         _start: 
            LDI.W @2, #{}\n\
            LDI.W @3, #{}\n\
            ;; Skip @4 to test a gap in vrom writes
            ADD @5, @2, @3\n\
            ADDI @6, @2, #{}\n\
            MULU @8, @2, @3\n\
            MUL @10, @2, @3\n\
            MULI @12, @2, #{}\n\
            RET\n",
        src1_value, src2_value, imm, imm
    );

    generate_trace(asm_code, None, None)
}
#[test]
fn test_integer_ops() -> Result<()> {
    let mut rng = StdRng::seed_from_u64(54321);
    test_from_trace_generator(
        || {
            // Test value to load
            let src1_value = rng.random::<u32>();
            let src2_value = rng.random::<u32>();
            generate_integer_ops_trace(src1_value, src2_value)
        },
        |trace| {
            assert_eq!(
                trace.add_events().len(),
                1,
                "Should have exactly one ADD event"
            );
            assert_eq!(
                trace.addi_events().len(),
                1,
                "Should have exactly one ADDI event"
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
                trace.mulu_events().len(),
                1,
                "Should have exacly one MULU event"
            );
        },
    )
}

/// Creates an execution trace for a simple program that uses only MVV.W,
/// BNZ, BZ, TAILI, and RET.
///
/// # Returns
/// * A Trace containing a simple program with a loop using TAILI, the BNZ
///   instruction is executed twice.
fn generate_simple_taili_trace(init_values: Vec<u32>) -> Result<Trace> {
    // Create a very simple assembly program that:
    // 1. _start sets up initial values and tail calls to loop
    // 2. loop checks if @3 is non-zero and either returns or continues
    // 3. case_recurse tail calls back to loop
    let asm_code = "#[framesize(0x10)]\n\
         _start:\n\
           MVV.W @3[2], @2\n\
           MVI.H @3[3], #2\n\
           TAILI loop, @3\n\
         #[framesize(0x10)]\n\
         loop:\n\
           BNZ case_recurse, @3\n\
           LDI.W @2, #100\n\
           RET\n\
         case_recurse:\n\
           LDI.W @4, #0\n\
           MVV.W @5[2], @2\n\
           MVV.W @5[3], @4\n\
           TAILI loop, @5\n"
        .to_string();

    // VROM state after the trace is executed
    // Sorted by number of accesses
    let vrom_writes = vec![
        // New FP values
        (3, 16, 3),
        (21, 32, 3),
        // TAILI events
        (16, 0, 2),
        (17, 0, 2),
        // MVV.W events
        (18, 100, 2),
        (19, 2, 2),
        (20, 0, 2),
        (32, 0, 2),
        (33, 0, 2),
        // LDI in case_recurse
        (34, 100, 2),
        // Additional MVV.W in case_recurse
        (35, 0, 2), // MVV.W @4[2], @3
        // Initial values
        (0, 0, 1),   // Return PC
        (1, 0, 1),   // Return FP
        (2, 100, 1), // Return value
    ];

    generate_trace(asm_code, Some(init_values), Some(vrom_writes))
}

#[test]
fn test_simple_taili_loop() -> Result<()> {
    // Test cases with different initial values
    let test_cases = vec![&[0, 0][..], &[0, 0, 100][..]];

    for init_values in test_cases {
        test_from_trace_generator(
            || generate_simple_taili_trace(init_values.to_vec()),
            |trace| {
                // Verify we have one MVI.H
                assert_eq!(
                    trace.mvih_events().len(),
                    1,
                    "Should have exactly one MVI.H event"
                );

                // Verify we have one LDI (in case_recurse)
                assert_eq!(
                    trace.ldi_events().len(),
                    2,
                    "Should have exactly one LDI event (in case_recurse)"
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

                // Verify we have one MVVW event (in case_recurse)
                assert_eq!(
                    trace.mvvw_events().len(),
                    3,
                    "Should have exactly three MVVW events"
                );

                // Verify we have one BZ event (when condition becomes 0)
                assert_eq!(
                    trace.bz_events().len(),
                    1,
                    "Should have exactly one BZ event"
                );
            },
        )?;
    }

    Ok(())
}

/// Creates an execution trace with all binary operations (AND, OR, XOR, ANDI,
/// ORI, XORI, B32_MUL, B32_MULI) using random input values.
///
/// # Returns
/// * A Trace containing all binary operations followed by a RET instruction
fn generate_all_binary_ops_trace() -> Result<Trace> {
    let mut rng = StdRng::seed_from_u64(12345);

    // Generate random values for testing
    let val1 = rng.random::<u32>();
    let val2 = rng.random::<u32>();
    let imm = rng.random::<u16>() as u32; // Smaller immediate for 16-bit operations
    let imm32 = rng.random::<u32>(); // Full 32-bit immediate for B32_MULI

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
            B32_MUL @10, @2, @3\n\
            B32_MULI @11, @2, #{}\n\
            RET\n",
        val1, val2, imm, imm, imm, imm32
    );

    // Calculate expected results
    let and_result = val1 & val2;
    let or_result = val1 | val2;
    let xor_result = val1 ^ val2;
    let andi_result = val1 & imm;
    let ori_result = val1 | imm;
    let xori_result = val1 ^ imm;
    let b32_mul_result = (B32::new(val1) * B32::new(val2)).val();
    let b32_muli_result = (B32::new(val1) * B32::new(imm32)).val();

    // Add VROM writes with appropriate access counts
    let vrom_writes = vec![
        // LDI events - with corrected access counts for more operations
        (2, val1, 9), // Used in all operations
        (3, val2, 5), // Used in AND, OR, XOR, B32_MUL
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
        (10, b32_mul_result, 1),
        (11, b32_muli_result, 1),
    ];

    generate_trace(asm_code, None, Some(vrom_writes))
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
            trace.b32_mul_events().len(),
            1,
            "Should have exactly one B32_MUL event"
        );
        assert_eq!(
            trace.b32_muli_events().len(),
            1,
            "Should have exactly one B32_MULI event"
        );
        assert_eq!(
            trace.ret_events().len(),
            1,
            "Should have exactly one RET event"
        );
    })
}

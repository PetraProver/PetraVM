//! Test the zCrayVM proving system with LDI and RET instructions.
//!
//! This file contains an integration test that verifies the complete
//! proving system pipeline from assembly to proof verification.

use anyhow::Result;
use binius_field::{BinaryField, Field};
use binius_m3::builder::B32;
use log::trace;
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
/// BNZ, TAILI, and RET.
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

// Creates a basic execution trace with just ANDI and RET instructions.
///
/// # Returns
/// * A Trace containing an ANDI instruction followed by a RET instruction
fn generate_andi_ret_trace() -> Result<Trace> {
    // Create a simple assembly program with ANDI and RET
    // Note: Format follows the grammar requirements:
    // - Program must start with a label followed by an instruction
    // - Used framesize for stack allocation
    let asm_code = "#[framesize(0x10)]\n\
        _start: ANDI @3, @2, #2\n\
        RET\n"
        .to_string();

    trace!("asm_code:\n {:?}", asm_code);

    let init_values = [0, 0, 1];

    let vrom_writes = vec![
        // Initial values
        (0, 0, 1),
        (1, 0, 1),
        (2, 1, 1),
        // ANDI event
        (3, 1 & 2, 1),
    ];

    generate_test_trace(asm_code, init_values, vrom_writes)
}

/// Creates a basic execution trace with just ANDI and RET instructions.
///
/// # Returns
/// * A Trace containing an ANDI instruction followed by a RET instruction
fn generate_xori_ret_trace() -> Result<Trace> {
    // Create a simple assembly program with ANDI and RET
    // Note: Format follows the grammar requirements:
    // - Program must start with a label followed by an instruction
    // - Used framesize for stack allocation
    let asm_code = "#[framesize(0x10)]\n\
        _start: XORI @3, @2, #2\n\
        RET\n"
        .to_string();

    trace!("asm_code:\n {:?}", asm_code);

    let init_values = [0, 0, 1];

    let vrom_writes = vec![
        // Initial values
        (0, 0, 1),
        (1, 0, 1),
        (2, 1, 1),
        // XORI event
        (3, 1 ^ 2, 1),
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

// Creates a basic execution trace with just AND and RET instructions.
///
/// # Returns
/// * A Trace containing an AND instruction followed by a RET instruction
fn generate_and_ret_trace() -> Result<Trace> {
    // Create a simple assembly program with AND and RET
    // Note: Format follows the grammar requirements:
    // - Program must start with a label followed by an instruction
    // - Used framesize for stack allocation
    let asm_code = "#[framesize(0x10)]\n\
        _start: AND @4, @3, @2\n\
        RET\n"
        .to_string();

    trace!("asm_code:\n {:?}", asm_code);

    let init_values = [0, 0, 1, 2];

    let vrom_writes = vec![
        // Initial values
        (0, 0, 1),
        (1, 0, 1),
        (2, 1, 1),
        (3, 2, 1),
        // AND event
        (4, 1 & 2, 1),
    ];

    generate_test_trace(asm_code, init_values, vrom_writes)
}

#[test]
fn test_and_ret() -> Result<()> {
    test_from_trace_generator(generate_and_ret_trace, |trace| {
        assert_eq!(
            trace.and_events().len(),
            1,
            "Should have exactly one AND event"
        );
        assert_eq!(
            trace.ret_events().len(),
            1,
            "Should have exactly one RET event"
        );
    })
}

// Creates a basic execution trace with just XOR and RET instructions.
///
/// # Returns
/// * A Trace containing an XOR instruction followed by a RET instruction
fn generate_xor_ret_trace() -> Result<Trace> {
    // Create a simple assembly program with XOR and RET
    // Note: Format follows the grammar requirements:
    // - Program must start with a label followed by an instruction
    // - Used framesize for stack allocation
    let asm_code = "#[framesize(0x10)]\n\
        _start: XOR @4, @3, @2\n\
        RET\n"
        .to_string();

    trace!("asm_code:\n {:?}", asm_code);

    let init_values = [0, 0, 7, 42];

    let vrom_writes = vec![
        // Initial values
        (0, 0, 1),
        (1, 0, 1),
        (2, 7, 1),
        (3, 42, 1),
        // XOR event
        (4, 42 ^ 7, 1),
    ];

    generate_test_trace(asm_code, init_values, vrom_writes)
}

// Creates a basic execution trace with just OR and RET instructions.
///
/// # Returns
/// * A Trace containing an OR instruction followed by a RET instruction
fn generate_or_ret_trace() -> Result<Trace> {
    // Create a simple assembly program with OR and RET
    // Note: Format follows the grammar requirements:
    // - Program must start with a label followed by an instruction
    // - Used framesize for stack allocation
    let asm_code = "#[framesize(0x10)]\n\
        _start: OR @4, @3, @2\n\
        RET\n"
        .to_string();

    trace!("asm_code:\n {:?}", asm_code);

    let init_values = [0, 0, 7, 42];

    let vrom_writes = vec![
        // Initial values
        (0, 0, 1),
        (1, 0, 1),
        (2, 7, 1),
        (3, 42, 1),
        // OR event
        (4, 42 | 7, 1),
    ];

    generate_test_trace(asm_code, init_values, vrom_writes)
}

// Creates a basic execution trace with just ORI and RET instructions.
///
/// # Returns
/// * A Trace containing an ORI instruction followed by a RET instruction
fn generate_ori_ret_trace() -> Result<Trace> {
    // Create a simple assembly program with OR and RET
    // Note: Format follows the grammar requirements:
    // - Program must start with a label followed by an instruction
    // - Used framesize for stack allocation
    let asm_code = "#[framesize(0x10)]\n\
        _start: ORI @3, @2, #7\n\
        RET\n"
        .to_string();

    trace!("asm_code:\n {:?}", asm_code);

    let init_values = [0, 0, 42];

    let vrom_writes = vec![
        // Initial values
        (0, 0, 1),
        (1, 0, 1),
        (2, 42, 1),
        // ORI event
        (3, 42 | 7, 1),
    ];

    generate_test_trace(asm_code, init_values, vrom_writes)
}

#[test]
fn test_xor_ret() -> Result<()> {
    test_from_trace_generator(generate_xor_ret_trace, |trace| {
        assert_eq!(
            trace.xor_events().len(),
            1,
            "Should have exactly one XOR event"
        );
        assert_eq!(
            trace.ret_events().len(),
            1,
            "Should have exactly one RET event"
        );
    })
}

#[test]
fn test_or_ret() -> Result<()> {
    test_from_trace_generator(generate_or_ret_trace, |trace| {
        assert_eq!(
            trace.or_events().len(),
            1,
            "Should have exactly one OR event"
        );
        assert_eq!(
            trace.ret_events().len(),
            1,
            "Should have exactly one RET event"
        );
    })
}

#[test]
fn test_ori_ret() -> Result<()> {
    test_from_trace_generator(generate_ori_ret_trace, |trace| {
        assert_eq!(
            trace.ori_events().len(),
            1,
            "Should have exactly one ORI event"
        );
        assert_eq!(
            trace.ret_events().len(),
            1,
            "Should have exactly one RET event"
        );
    })
}

#[test]
fn test_bnz_non_zero_branch_ret() -> Result<()> {
    test_from_trace_generator(
        || generate_bnz_ret_trace(1),
        |trace| {
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

#[test]
fn test_andi_ret() -> Result<()> {
    test_from_trace_generator(generate_andi_ret_trace, |trace| {
        assert_eq!(
            trace.andi_events().len(),
            1,
            "Should have exactly one LDI event"
        );
        assert_eq!(
            trace.ret_events().len(),
            1,
            "Should have exactly one RET event"
        );
    })
}

#[test]
fn test_xori_ret() -> Result<()> {
    test_from_trace_generator(generate_xori_ret_trace, |trace| {
        assert_eq!(
            trace.xori_events().len(),
            1,
            "Should have exactly one bz event"
        );
        assert_eq!(
            trace.ret_events().len(),
            1,
            "Should have exactly one RET event"
        );
    })
}

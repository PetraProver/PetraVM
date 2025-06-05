use anyhow::Result;
use binius_field::{BinaryField, Field};
use binius_hash::groestl::{GroestlShortImpl, GroestlShortInternal};
use binius_m3::builder::B32;
use log::trace;
use petravm_asm::{
    isa::{GenericISA, RecursionISA, ISA},
    transpose_in_aes, transpose_in_bin,
    util::{bytes_to_u32, u32_to_bytes},
    Assembler, Instruction, InterpreterInstruction, Memory, PetraTrace, ValueRom,
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
pub fn generate_asm_trace(
    files: &[&str],
    init_values: Vec<u32>,
    isa: Box<dyn ISA>,
) -> Result<Trace> {
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

    generate_trace(asm_code, Some(init_values), None, isa)
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
    let n = B32::MULTIPLICATIVE_GENERATOR.pow([n as u64]).val();
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: n
    // Slot 3: Arg: Result
    let init_values = vec![0, 0, n, res];
    let isa = Box::new(GenericISA);
    generate_asm_trace(&["fib.asm"], init_values, isa)
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
    // Initialize memory with:
    // Slot 0: Return PC = 0
    // Slot 1: Return FP = 0
    // Slot 2: Arg: n
    let init_values = vec![0, 0, n];
    let isa = Box::new(GenericISA);
    generate_asm_trace(&["collatz.asm"], init_values, isa)
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
    isa: Box<dyn ISA>,
) -> Result<Trace> {
    // Compile the assembly code
    let compiled_program = Assembler::from_code(&asm_code)?;
    trace!("compiled program = {compiled_program:?}");

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
        ));
    }

    // Initialize memory with return PC = 0, return FP = 0 if not provided
    let vrom = ValueRom::new_with_init_vals(&init_values.unwrap_or_else(|| vec![0, 0]));
    let memory = Memory::new(compiled_program.prom, vrom);

    // Generate the trace from the compiled program
    let (petra_trace, _) = PetraTrace::generate(
        isa,
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

/// Creates an execution trace for a simple program that uses only
/// GROESTL256_COMPRESS, GROESTL256_OUTPUT, and RET.
///
/// # Returns
/// * A Trace containing a simple program with a loop using TAILI, the BNZ
///   instruction is executed twice.
pub fn generate_groestl_ret_trace(src1_val: [u32; 16], src2_val: [u32; 16]) -> Result<Trace> {
    // Frame:
    // Slot 0: PC
    // Slot 1: FP
    // Slots 2-15: Padding
    // Slots 16-31: src1_val
    // Slots 32-47: src2_val
    // Slots 48-63: compression_output
    // Slots 64-79: groestl_output
    let src1_offset = 16;
    let src2_offset = 32;
    let compression_output_offset = 48;
    let groestl_output_offset = 64;
    let asm_code = format!(
        "#[framesize(0x10)]\n\
         _start: 
            GROESTL256_COMPRESS @{}, @{}, @{}\n\
            GROESTL256_OUTPUT @{}, @{}, @{}\n\
            RET\n",
        compression_output_offset,
        src1_offset,
        src2_offset,
        groestl_output_offset,
        compression_output_offset,     // lower bits of the new input state
        compression_output_offset + 8  // higher bits of the new input state
    );

    let mut init_values = vec![0; 48];
    init_values[src1_offset..src1_offset + 16].copy_from_slice(&src1_val);
    init_values[src2_offset..src2_offset + 16].copy_from_slice(&src2_val);

    //////////////////////////
    //// COMPRESSION STEP ////
    //////////////////////////

    // Compute the output of the compression step.
    let src1_bytes = u32_to_bytes(&src1_val);
    let src2_bytes = u32_to_bytes(&src2_val);

    let src1_val_new = transpose_in_aes(&src1_bytes);

    let mut compression_output =
        GroestlShortImpl::state_from_bytes(&src1_val_new.clone().try_into().unwrap());

    <GroestlShortImpl as GroestlShortInternal>::compress(
        &mut compression_output,
        &src2_bytes.clone().try_into().unwrap(),
    );

    // The output of the compression gadget is transposed compared to the Groestl
    // specs, and in the wrong basis. We therefore need to transpose it and change
    // its basis before writing to memory. However, the Output gadget expects the
    // input to be in this form, so we can pass it directly as is to the next
    // gadget.
    let out_state_bytes_transposed = GroestlShortImpl::state_to_bytes(&compression_output);
    let out_state_bytes = transpose_in_bin(&out_state_bytes_transposed);

    // Output state that is stored as the input of the next compression step.
    let compression_output_u32 = bytes_to_u32(&out_state_bytes);

    //////////////////////////////////////////
    //// 2-to-1 COMPRESSION (OUTPUT) STEP ////
    //////////////////////////////////////////

    // Compute the output of the 2-to-1 groestl compression.
    let mut state = compression_output;
    GroestlShortImpl::p_perm(&mut state);

    // Calculate the output: dst_val = P(state_in) XOR init
    let dst_val: [u64; 8] = state
        .iter()
        .zip(compression_output.iter())
        .map(|(&x, &y)| x ^ y)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    // Convert dst_val to a big endian representation.
    let output_state_bytes = GroestlShortImpl::state_to_bytes(&dst_val);

    let dst_val: [u8; 32] = output_state_bytes[32..].try_into().unwrap();
    let groestl_output = bytes_to_u32(&dst_val);

    // Add VROM writes from GROESTL and RET events.
    let mut vrom_writes = vec![];
    // Write outputs.
    vrom_writes.extend(
        compression_output_u32
            .iter()
            .enumerate()
            .map(|(i, v)| (i as u32 + compression_output_offset, *v, 2u32)),
    );
    // FP and PC.
    vrom_writes.extend_from_slice(&[(0, 0, 1), (1, 0, 1)]);
    // Inputs.
    vrom_writes.extend(
        src1_val
            .iter()
            .enumerate()
            .map(|(i, v)| ((i + src1_offset) as u32, *v, 1)),
    );
    vrom_writes.extend(
        src2_val
            .iter()
            .enumerate()
            .map(|(i, v)| ((i + src2_offset) as u32, *v, 1)),
    );
    // Final output
    vrom_writes.extend(
        groestl_output
            .iter()
            .enumerate()
            .map(|(i, v)| (i as u32 + groestl_output_offset, *v, 1)),
    );

    let isa = Box::new(RecursionISA);
    generate_trace(asm_code, Some(init_values), Some(vrom_writes), isa)
}

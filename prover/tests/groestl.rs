use anyhow::Result;
use binius_hash::compression;
use binius_hash::groestl::{GroestlShortImpl, GroestlShortInternal};
use bytemuck::cast_slice;
use generic_array::{typenum, GenericArray};
use log::trace;
use zcrayvm_assembly::isa::RecursionISA;
use zcrayvm_prover::model::Trace;
use zcrayvm_prover::prover::{verify_proof, Prover};
use zcrayvm_prover::test_utils::generate_trace;

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
    println!("validated");

    // Step 3: Create prover
    println!("Creating prover...");
    let prover = Prover::new(Box::new(RecursionISA));

    // Step 4: Generate proof
    println!("Generating proof...");
    let (proof, statement, compiled_cs) = prover.prove(&trace)?;

    // Step 5: Verify proof
    println!("Verifying proof...");
    verify_proof(&statement, &compiled_cs, proof)?;

    trace!("All steps completed successfully!");
    Ok(())
}

/// Creates an execution trace for a simple program that uses only
/// GROESTL256_COMPRESS, GROESTL256_OUTPUT, and RET.
///
/// # Returns
/// * A Trace containing a simple program with a loop using TAILI, the BNZ
///   instruction is executed twice.
fn generate_groestl_ret_trace(
    init_values: Vec<u32>,
    src1_val: [u32; 16],
    src2_val: [u32; 16],
) -> Result<Trace> {
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
            RET\n",
        compression_output_offset,
        src1_offset,
        src2_offset,
        // groestl_output_offset,
        // compression_output_offset,     // lower bits of the new input state
        // compression_output_offset + 8  // higher bits of the new input state
    );

    // Compute the output of the compression step.
    let src1_bytes = cast_slice::<u32, u8>(&src1_val);
    let src2_bytes = cast_slice::<u32, u8>(&src2_val);
    // let mut compression_output =
    // GroestlShortImpl::state_from_bytes(src1_bytes.try_into().unwrap());
    let mut compression_output: [u64; 8] = cast_slice::<u32, u64>(&src1_val.to_vec())
        .try_into()
        .unwrap();
    <GroestlShortImpl as GroestlShortInternal>::compress(
        &mut compression_output,
        src2_bytes.try_into().unwrap(),
    );
    let compression_output = cast_slice::<u64, u32>(&compression_output);
    println!("compression_output: {:?}", compression_output);

    // Compute the output of the 2-to-1 groestl compression.
    let input = cast_slice::<u32, u8>(&compression_output);
    let input = GroestlShortImpl::state_from_bytes(input.try_into().unwrap());
    let mut state_in = input.clone();
    GroestlShortImpl::p_perm(&mut state_in);

    // Calculate the output: dst_val = P(state_in) XOR init
    let dst_val: [u64; 8] = state_in
        .iter()
        .zip(input.iter())
        .map(|(&x, &y)| x ^ y)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    // Convert dst_val to a big endian representation.
    let output_state_bytes = GroestlShortImpl::state_to_bytes(&dst_val);
    let dst_val = GenericArray::<u8, typenum::U32>::from_slice(&output_state_bytes[32..]);
    let groestl_output = cast_slice::<u8, u32>(&dst_val);

    // Add VROM writes from GROESTL and RET events.
    let mut vrom_writes = vec![];
    // Write outputs.
    // vrom_writes.extend(
    //     compression_output
    //         .iter()
    //         .enumerate()
    //         .map(|(i, v)| (i as u32 + compression_output_offset, *v, 2u32)),
    // );
    // FP and PC.
    vrom_writes.extend_from_slice(&[(0, 0, 1), (1, 0, 1)]);
    // Inputs.
    vrom_writes.extend(
        src1_val
            .iter()
            .enumerate()
            .map(|(i, v)| (i as u32 + src1_offset, *v, 1)),
    );
    vrom_writes.extend(
        src2_val
            .iter()
            .enumerate()
            .map(|(i, v)| (i as u32 + src2_offset, *v, 1)),
    );

    vrom_writes.extend(
        compression_output
            .iter()
            .enumerate()
            .map(|(i, v)| (i as u32 + compression_output_offset, *v, 1u32)),
    );
    // // Final output
    // vrom_writes.extend(
    //     groestl_output
    //         .iter()
    //         .enumerate()
    //         .map(|(i, v)| (i as u32 + groestl_output_offset, *v, 1)),
    // );

    let isa = Box::new(RecursionISA);
    generate_trace(asm_code, Some(init_values), Some(vrom_writes), isa)
}

#[test]
fn test_groestl_proving() -> Result<()> {
    test_from_trace_generator(
        || {
            // Test value to load
            let mut src1_val = [0; 16];
            src1_val[0] = 1;
            let mut src2_val = [0; 16];
            src2_val[0] = 2;

            let src1_offset = 16;
            let src2_offset = 32;
            let mut init_values = vec![0; 48];
            init_values[src1_offset as usize] = src1_val[0] as u32;
            init_values[src2_offset as usize] = src2_val[0] as u32;

            generate_groestl_ret_trace(init_values, src1_val, src2_val)
        },
        |trace| {
            assert_eq!(
                trace.groestl_compress_events().len(),
                1,
                "Should have exactly one GROESTL256_COMPRESS event"
            );
            // assert_eq!(
            //     trace.groestl_output_events().len(),
            //     1,
            //     "Should have exactly GROESTL256_OUTPUT event"
            // );
            assert_eq!(
                trace.ret_events().len(),
                1,
                "Should have exactly one RET event"
            );
        },
    )
}

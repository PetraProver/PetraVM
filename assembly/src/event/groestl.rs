use binius_field::AESTowerField8b;
use binius_hash::groestl::{GroestlShortImpl, GroestlShortInternal};
use binius_m3::builder::{B16, B32, B8};
use bytemuck::cast_slice;

use super::{context::EventContext, Event};
use crate::{
    execution::{FramePointer, InterpreterChannels, InterpreterError},
    macros::fire_non_jump_event,
    util::bytes_to_u32,
};

/// Event for GROESTL256_COMPRESS.
///
/// Performs a Groestl compression between two 512-bit inputs.
///
/// The first input comes from the output of the previous Groestl compression
/// gadget, which returns a transposed output compared to the Groestl specs.
/// Thus, we need to start by transposing the first input. We also need to
/// transpose the output here, as the gadget returns a transposed output
/// compared to the specs. Similarly, we assume a change of basis (from AES to
/// binary) has already been applied to the first input. And since we are
/// chaining compression gadgets, we also apply the reverse operation to the
/// output. So we need to change bases for the first input and the output:
/// - go from binary to AES for the first input
/// - go from AES to binary for the output.
#[derive(Debug, Clone)]
pub struct Groestl256CompressEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    /// dst is the offset where the output is stored.
    /// Since we are reading 16 words from memory, it needs to be 16-word
    /// aligned.
    pub dst: u16,
    pub dst_val: [u64; 8],
    /// src1 is the offset where the output is stored.
    /// Since we are reading 16 words from memory, it needs to be 16-word
    /// aligned.
    pub src1: u16,
    pub src1_val: [u8; 64],
    /// src2 is the offset where the output is stored.
    /// Since we are reading 16 words from memory, it needs to be 16-word
    /// aligned.
    pub src2: u16,
    pub src2_val: [u8; 64],
}

impl Event for Groestl256CompressEvent {
    fn generate(
        ctx: &mut EventContext,
        dst: B16,
        src1: B16,
        src2: B16,
    ) -> Result<(), InterpreterError> {
        let src1_val = read_bytes::<16>(ctx, src1)?;
        let src2_val = read_bytes::<16>(ctx, src2)?;

        // We transpose the first input as we supposed it from the previous Groestl
        // compression gadget in a transposed form. We also change its bases to the AES
        // one.
        let src1_val_aes = transpose_in_aes(&src1_val);

        let mut out_val = GroestlShortImpl::state_from_bytes(
            &src1_val_aes
                .try_into()
                .expect("src1_val_aes is exactly 64 bytes"),
        );

        <GroestlShortImpl as GroestlShortInternal>::compress(
            &mut out_val,
            &src2_val
                .clone()
                .try_into()
                .expect("src2_val is exactly 64 bytes"),
        );

        // The output of the Groestl compression gadget is transposed compared to the
        // Groestl specs. This is to make the permutation gadgets, as well as chaining
        // them, more efficient.
        let out_state_bytes_transposed = GroestlShortImpl::state_to_bytes(&out_val);
        let out_state_bytes = transpose_in_bin(&out_state_bytes_transposed);

        let dst_val: [u64; 8] = cast_slice::<u8, u64>(&out_state_bytes)
            .try_into()
            .expect("out_state_bytes is exactly 64 bytes");

        for i in 0..8 {
            ctx.vrom_write::<u64>(ctx.addr(dst.val() + 2 * i), dst_val[i as usize])?;
        }

        let (_pc, field_pc, fp, timestamp) = ctx.program_state();
        ctx.incr_pc();

        let event = Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_val,
            src1: src1.val(),
            src1_val: src1_val.try_into().expect("src1_val should be 64 bytes"),
            src2: src2.val(),
            src2_val: src2_val.try_into().expect("src2_val should be 64 bytes"),
        };

        ctx.trace.groestl_compress.push(event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        fire_non_jump_event!(self, channels);
    }
}

/// Event for GROESTL256_OUTPUT.
///
/// Performs the output step of a Groestl hash.
/// It corresponds to a 2-to-1 compresssion.
/// The input comes from the last Groestl compression gadget, which returns a
/// transposed output compared to the Groestl specs. The returned value has
/// already been changed from the AES basis to the bonary basis. Thus, we need
/// to start by transposing the input and applying a change of basis from binary
/// to AES).
#[derive(Debug, Clone)]
pub struct Groestl256OutputEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    /// dst is the offset where the output is stored.
    /// Since we are reading 8 words from memory, it needs to be 8-word aligned.
    pub dst: u16,
    pub dst_val: [u32; 8],
    /// src1 is the offset where the first 32 bytes of the input are stored.
    /// Since we are reading 8 words from memory, it needs to be 8-word aligned.
    pub src1: u16,
    pub src1_val: [u8; 32],
    /// src2 is the offset where the last 32 bytes of the input are stored.
    /// Since we are reading 8 words from memory, it needs to be 8-word aligned.
    pub src2: u16,
    pub src2_val: [u8; 32],
}

impl Event for Groestl256OutputEvent {
    #[allow(clippy::default_constructed_unit_structs)]
    fn generate(
        ctx: &mut EventContext,
        dst: B16,
        src1: B16,
        src2: B16,
    ) -> Result<(), InterpreterError> {
        let src1_val = read_bytes::<8>(ctx, src1)?;
        let src2_val = read_bytes::<8>(ctx, src2)?;

        let transposed_full_input_aes_inv: [u8; 64] = [src1_val.clone(), src2_val.clone()]
            .concat()
            .try_into()
            .expect("src1_val_aes and src2_val_aes are exactly 32 bytes each.");

        // The input of the Groestl output gadget comes from the output of the Groestl
        // compress gadget, which is transposed compared to the specs. So we need to
        // transpose the input here.
        // We also assume (due to chaining of compression gadgets) that the inputs were
        // switched from the AES to the binary basis, so we should start by applying the
        // inverse operation.
        let full_input: [u8; 64] = transpose_in_aes(&transposed_full_input_aes_inv)
            .try_into()
            .expect("full_input is exactly 64 bytes");

        let state_in = GroestlShortImpl::state_from_bytes(&full_input);
        let mut state = state_in;

        // First, carry out the P permutation on the input.
        GroestlShortImpl::p_perm(&mut state);
        GroestlShortImpl::xor_state(&mut state, &state_in);

        // The state is now in the AES basis. The Output gadget is
        // the final element of the Groestl chain. We started with an input that was
        // switched from the AES to the binary basis. So the final output should
        // indeed be in the AES basis, and there is therefore no need to switch bases
        // again.

        // Get the output in the correct format.
        let out_state_bytes = GroestlShortImpl::state_to_bytes(&state);

        let dst_val: [u8; 32] = out_state_bytes[32..]
            .try_into()
            .expect("out_state_bytes is 64 bytes");
        let dst_val = bytes_to_u32(&dst_val);
        for i in 0..8 {
            ctx.vrom_write(ctx.addr(dst.val() + i), dst_val[i as usize])?;
        }

        let (_pc, field_pc, fp, timestamp) = ctx.program_state();
        ctx.incr_pc();

        let event = Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_val: dst_val.try_into().expect("dst_val is exactly 32 bytes"),
            src1: src1.val(),
            src1_val: src1_val.try_into().expect("src1_val is exactly 32 bytes"),
            src2: src2.val(),
            src2_val: src2_val.try_into().expect("src2_val is exactly 32 bytes"),
        };

        ctx.trace.groestl_output.push(event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        fire_non_jump_event!(self, channels);
    }
}

fn read_bytes<const N: usize>(
    ctx: &mut EventContext,
    src: B16,
) -> Result<Vec<u8>, InterpreterError> {
    let mut src_val = Vec::with_capacity(N * 4);
    for i in 0..N {
        src_val.extend(
            ctx.vrom_read::<u32>(ctx.addr(src.val() + i as u16))?
                .to_le_bytes(),
        );
    }

    Ok(src_val)
}

pub fn transpose_in_aes(src_val: &[u8]) -> Vec<u8> {
    (0..8)
        .flat_map(|i| {
            (0..8)
                .map(|j| AESTowerField8b::from(B8::new(src_val[j * 8 + i])).val())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
}

pub fn transpose_in_bin(src_val: &[u8]) -> Vec<u8> {
    (0..8)
        .flat_map(|i| {
            (0..8)
                .map(|j| B8::from(AESTowerField8b::new(src_val[j * 8 + i])).val())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use binius_field::Field;
    use generic_array::{typenum, GenericArray};

    use super::*;
    use crate::{isa::RecursionISA, test_util::code_to_prom, Memory, Opcode, PetraTrace, ValueRom};

    #[test]
    fn test_groestl_compress() {
        // Frame:
        // Slot 0: PC
        // Slot 1: FP
        // Slots 2-15: Padding
        // Slots 16-31: src1_val
        // Slots 32-47: src2_val
        // Slots 48-63: dst_val

        let src1_val = [rand::random::<u8>(); 64];
        let src1_val_packed = bytes_to_u32(src1_val.as_slice());

        let src2_val = [rand::random::<u8>(); 64];
        let src2_val_packed = bytes_to_u32(src2_val.as_slice());

        let dst_offset = 48;
        let src1_offset = 16;
        let src2_offset = 32;
        let mut init_values = vec![0; 48];
        init_values[src1_offset..(16 + src1_offset)].copy_from_slice(&src1_val_packed[..16]);
        init_values[src2_offset..(16 + src2_offset)].copy_from_slice(&src2_val_packed[..16]);

        let vrom = ValueRom::new_with_init_vals(&init_values);

        // Construct a simple program with the Groestl256Compress instruction
        // 1. GROESTL256_Compress @output, @src1, @src2
        // 2. RET
        let zero = B16::ZERO;
        let dst = B16::from(dst_offset as u16);
        let src1 = B16::from(src1_offset as u16);
        let src2 = B16::from(src2_offset as u16);
        let instructions = vec![
            [Opcode::Groestl256Compress.get_field_elt(), dst, src1, src2],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];
        // Set up frame sizes
        let mut frames = HashMap::new();
        frames.insert(B32::ONE, 64);

        // Create the PROM
        let prom = code_to_prom(&instructions);
        let memory = Memory::new(prom, vrom);

        // Create an interpreter and run the program
        let (trace, boundary_values) =
            PetraTrace::generate(Box::new(RecursionISA), memory, frames, HashMap::new())
                .expect("Trace generation should not fail.");

        // Validate the trace (this consumes boundary_values)
        trace.validate(boundary_values);

        // Calculate the output.
        let src1_val_new = transpose_in_aes(&src1_val);

        let mut state = GroestlShortImpl::state_from_bytes(
            &src1_val_new
                .try_into()
                .expect("src1_val_new is exactly 64 bytes"),
        );
        <GroestlShortImpl as GroestlShortInternal>::compress(&mut state, &src2_val);

        // Reshape the output to match the expected format.
        let out_state_bytes_transposed = GroestlShortImpl::state_to_bytes(&state);
        let out_state_bytes = transpose_in_bin(&out_state_bytes_transposed);

        let dst_vals = bytes_to_u32(&out_state_bytes);

        let actual_dst_vals = (0..16)
            .map(|i| trace.vrom().read::<u32>(dst_offset + i).unwrap())
            .collect::<Vec<_>>();
        for i in 0..16 {
            assert_eq!(dst_vals[i], actual_dst_vals[i]);
        }
    }

    #[test]
    fn test_groestl_output() {
        // Frame:
        // Slot 0: PC
        // Slot 1: FP
        // Slots 2-7: Padding
        // Slots 8-15: src1_val
        // Slots 16-23: src2_val
        // Slots 24-31: dst_val

        let src1_val = [rand::random::<u8>(); 32];
        let src1_val_packed = bytes_to_u32(src1_val.as_slice());

        let src2_val = [rand::random::<u8>(); 32];
        let src2_val_packed = bytes_to_u32(src2_val.as_slice());

        let dst_offset = 24;
        let src1_offset = 8;
        let src2_offset = 16;
        let mut init_values = vec![0; 24];
        init_values[src1_offset..(8 + src1_offset)].copy_from_slice(&src1_val_packed[..8]);
        init_values[src2_offset..(8 + src2_offset)].copy_from_slice(&src2_val_packed[..8]);
        let vrom = ValueRom::new_with_init_vals(&init_values);

        // Construct a simple program with the Groestl256Output instruction
        // 1. GROESTL256_OUTPUT @output, @src1, @src2
        // 2. RET
        let zero = B16::ZERO;
        let dst = B16::from(dst_offset as u16);
        let src1 = B16::from(src1_offset as u16);
        let src2 = B16::from(src2_offset as u16);
        let instructions = vec![
            [Opcode::Groestl256Output.get_field_elt(), dst, src1, src2],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];
        // Set up frame sizes
        let mut frames = HashMap::new();
        frames.insert(B32::ONE, 32);

        // Create the PROM
        let prom = code_to_prom(&instructions);
        let memory = Memory::new(prom, vrom);

        // Create an interpreter and run the program
        let (trace, boundary_values) =
            PetraTrace::generate(Box::new(RecursionISA), memory, frames, HashMap::new())
                .expect("Trace generation should not fail.");

        // Validate the trace (this consumes boundary_values)
        trace.validate(boundary_values);

        // The input needs to be transposed.
        let full_input_transposed: [u8; 64] = [src1_val, src2_val]
            .concat()
            .try_into()
            .expect("src1_val_new and src2_val_new are exactly 32 bytes each");

        let full_input = transpose_in_aes(&full_input_transposed);
        let init = GroestlShortImpl::state_from_bytes(
            &full_input
                .try_into()
                .expect("full_input is exactly 64 bytes"),
        );
        let mut state_in = init;
        GroestlShortImpl::p_perm(&mut state_in);

        // Calculate the output: dst_val = P(state_in) XOR init
        let dst_val: [u64; 8] = state_in
            .iter()
            .zip(init.iter())
            .map(|(&x, &y)| x ^ y)
            .collect::<Vec<_>>()
            .try_into()
            .expect("state_in and init are exactly 64 bytes each");

        // Convert dst_val to a big endian representation.
        let output_state_bytes = GroestlShortImpl::state_to_bytes(&dst_val);

        let dst_val = GenericArray::<u8, typenum::U32>::from_slice(&output_state_bytes[32..]);
        let dst_val = bytes_to_u32(dst_val);

        let actual_dst_vals = (0..8)
            .map(|i| trace.vrom().read::<u32>(dst_offset + i).unwrap())
            .collect::<Vec<_>>();
        for i in 0..8 {
            assert_eq!(dst_val[i], actual_dst_vals[i]);
        }
    }

    #[test]
    fn test_groestl_chaining() {
        // Note that when chaining, we should be able to apply a "normal" Groestl
        // computation, except for the very first input, which is supposed to be in the
        // wrong initial base, and transposed.

        // Frame:
        // Slot 0: PC
        // Slot 1: FP
        // Slots 2-15: Padding
        // Slots 16-31: src1_val
        // Slots 32-47: src2_val
        // Slots 48-63: compression_output
        // Slots 64-79: groestl_output

        let src1_val = [rand::random::<u8>(); 64];
        let src1_val_packed = bytes_to_u32(src1_val.as_slice());

        let src2_val = [rand::random::<u8>(); 64];
        let src2_val_packed = bytes_to_u32(src2_val.as_slice());

        let src1_offset = 16;
        let src2_offset = 32;
        let compression_output_offset = 48;
        let groestl_output_offset = 64;

        let mut init_values = vec![0; 48];
        init_values[src1_offset..(16 + src1_offset)].copy_from_slice(&src1_val_packed[..16]);
        init_values[src2_offset..(16 + src2_offset)].copy_from_slice(&src2_val_packed[..16]);
        let vrom = ValueRom::new_with_init_vals(&init_values);

        // Construct a simple program with the Groestl256Output instruction
        // 1. GROESTL256_COMPRESS @compression_output, @src1, @src2
        // 2. GROESTL256_OUTPUT @groestl_output, @compression_output,
        //    @compression_output + 8
        // 3. RET
        let zero = B16::ZERO;
        let compression_dst = B16::from(compression_output_offset as u16);
        let compression_dst_offset = B16::from(compression_output_offset as u16 + 8);
        let output_dst = B16::from(groestl_output_offset as u16);
        let src1 = B16::from(src1_offset as u16);
        let src2 = B16::from(src2_offset as u16);
        let instructions = vec![
            [
                Opcode::Groestl256Compress.get_field_elt(),
                compression_dst,
                src1,
                src2,
            ],
            [
                Opcode::Groestl256Output.get_field_elt(),
                output_dst,
                compression_dst,
                compression_dst_offset,
            ],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];
        // Set up frame sizes
        let mut frames = HashMap::new();
        frames.insert(B32::ONE, 32);

        // Create the PROM
        let prom = code_to_prom(&instructions);
        let memory = Memory::new(prom, vrom);

        // Create an interpreter and run the program
        let (trace, boundary_values) =
            PetraTrace::generate(Box::new(RecursionISA), memory, frames, HashMap::new())
                .expect("Trace generation should not fail.");

        // Validate the trace (this consumes boundary_values)
        trace.validate(boundary_values);

        // Create the input state. The first input needs to be transposed and switched
        // from binary to AES basis.
        let src1_val_new = transpose_in_aes(&src1_val);
        let mut state = GroestlShortImpl::state_from_bytes(
            &src1_val_new
                .try_into()
                .expect("src1_val_new is exactly 64 bytes"),
        );

        // Carry out a normal Groestl computation on the new state.
        GroestlShortImpl::compress(&mut state, &src2_val);
        let mut new_state = state;
        GroestlShortImpl::p_perm(&mut new_state);

        // Calculate the output: dst_val = P(state_in) XOR init
        let dst_val: [u64; 8] = new_state
            .iter()
            .zip(state.iter())
            .map(|(&x, &y)| x ^ y)
            .collect::<Vec<_>>()
            .try_into()
            .expect("state_in and init are exactly 64 bytes each");

        let output_state_bytes = GroestlShortImpl::state_to_bytes(&dst_val);
        let dst_val = output_state_bytes[32..]
            .try_into()
            .expect("output_state_bytes is exactly 32 bytes");
        let groestl_output = bytes_to_u32(dst_val);

        // Read the output from the trace.
        let actual_dst_vals = (0..8)
            .map(|i| trace.vrom().read::<u32>(groestl_output_offset + i).unwrap())
            .collect::<Vec<_>>();
        for i in 0..8 {
            assert_eq!(groestl_output[i], actual_dst_vals[i]);
        }
    }
}

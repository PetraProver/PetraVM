use binius_field::AESTowerField8b;
use binius_hash::groestl::{GroestlShortImpl, GroestlShortInternal};
use binius_m3::builder::{B16, B32, B8};

use super::{context::EventContext, Event};
use crate::{
    execution::{FramePointer, InterpreterChannels, InterpreterError},
    macros::fire_non_jump_event,
    util::bytes_to_u32,
};

/// Event for GROESTL256_COMPRESS.
///
/// Performs a Groestl compression between two 512-bit inputs.
#[derive(Debug, Clone)]
pub struct Groestl256CompressEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub dst_val: [u32; 16],
    pub src1: u16,
    pub src1_val: [u8; 64],
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
        let mut src1_val = Vec::with_capacity(64);
        for i in 0..16 {
            src1_val.extend(
                ctx.vrom_read::<u32>(ctx.addr(src1.val() + i))?
                    .to_le_bytes(),
            );
        }
        let mut src2_val = Vec::with_capacity(64);
        for i in 0..16 {
            src2_val.extend(
                ctx.vrom_read::<u32>(ctx.addr(src2.val() + i))?
                    .to_le_bytes(),
            );
        }

        // Change the input bases to match the one used in P/Q permutations.
        let src1_val_aes = src1_val
            .iter()
            .map(|s1| AESTowerField8b::from(B8::from(*s1)).val())
            .collect::<Vec<_>>();
        let src2_val_aes = src2_val
            .iter()
            .map(|s2| AESTowerField8b::from(B8::from(*s2)).val())
            .collect::<Vec<_>>();

        let mut out_val = GroestlShortImpl::state_from_bytes(
            &src1_val_aes
                .try_into()
                .expect("src1_val_aes is exactly 64 bytes"),
        );

        <GroestlShortImpl as GroestlShortInternal>::compress(
            &mut out_val,
            &src2_val_aes
                .try_into()
                .expect("src2_val is exactly 64 bytes"),
        );

        let out_state_bytes = GroestlShortImpl::state_to_bytes(&out_val);
        let out_state_bytes =
            out_state_bytes.map(|byte| B8::from(AESTowerField8b::new(byte)).val());

        let dst_val: [u32; 16] = bytes_to_u32(&out_state_bytes)
            .try_into()
            .expect("out_state_bytes is exactly 64 bytes");

        for i in 0..16 {
            ctx.vrom_write::<u32>(ctx.addr(dst.val() + i), dst_val[i as usize])?;
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
#[derive(Debug, Clone)]
pub struct Groestl256OutputEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub dst_val: [u32; 8],
    pub src1: u16,
    pub src1_val: [u8; 32],
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
        let mut src1_val = Vec::with_capacity(32);
        for i in 0..8 {
            src1_val.extend(
                ctx.vrom_read::<u32>(ctx.addr(src1.val() + i))?
                    .to_le_bytes(),
            );
        }
        let mut src2_val = Vec::with_capacity(32);
        for i in 0..8 {
            src2_val.extend(
                ctx.vrom_read::<u32>(ctx.addr(src2.val() + i))?
                    .to_le_bytes(),
            );
        }

        // Change the input bases to match the the one used in P/Q permutations.
        let src1_val_aes = src1_val
            .iter()
            .map(|s1| AESTowerField8b::from(B8::from(*s1)).val())
            .collect::<Vec<_>>();
        let src2_val_aes = src2_val
            .iter()
            .map(|s2| AESTowerField8b::from(B8::from(*s2)).val())
            .collect::<Vec<_>>();
        let full_input: [u8; 64] = [src1_val_aes, src2_val_aes]
            .concat()
            .try_into()
            .expect("src1_val_aes and src2_val_aes are exactly 32 bytes each.");
        let state_in = GroestlShortImpl::state_from_bytes(&full_input);
        let mut state = state_in;

        // First, carry out the P permutation on the input.
        GroestlShortImpl::p_perm(&mut state);
        GroestlShortImpl::xor_state(&mut state, &state_in);

        // Get the output in the correct format.
        let out_state_bytes = GroestlShortImpl::state_to_bytes(&state);
        let out_state_bytes =
            out_state_bytes.map(|byte| B8::from(binius_field::AESTowerField8b::new(byte)).val());

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

        let mut src1_val = [0u8; 64];
        src1_val[0] = 1;
        let mut src2_val = [0u8; 64];
        src2_val[0] = 2;

        let dst_offset = 48;
        let src1_offset = 16;
        let src2_offset = 32;
        let mut init_values = vec![0; 48];
        init_values[src1_offset as usize] = src1_val[0] as u32;
        init_values[src2_offset as usize] = src2_val[0] as u32;
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
        let src1_val_new = src1_val
            .iter()
            .map(|s1| AESTowerField8b::from(B8::from(*s1)).val())
            .collect::<Vec<_>>();
        let src2_val_new = src2_val
            .iter()
            .map(|s2| AESTowerField8b::from(B8::from(*s2)).val())
            .collect::<Vec<_>>();

        let mut state = GroestlShortImpl::state_from_bytes(
            &src1_val_new
                .try_into()
                .expect("src1_val_new is exactly 64 bytes"),
        );
        <GroestlShortImpl as GroestlShortInternal>::compress(
            &mut state,
            &src2_val_new
                .try_into()
                .expect("src2_val_new is exactly 64 bytes"),
        );

        // Reshape the output to match the expected format.
        let out_state_bytes = GroestlShortImpl::state_to_bytes(&state);
        let out_state_bytes =
            out_state_bytes.map(|byte| B8::from(binius_field::AESTowerField8b::new(byte)).val());
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

        let mut src1_val = [0u8; 32];
        src1_val[0] = 1;
        let mut src2_val = [0u8; 32];
        src2_val[0] = 2;

        let dst_offset = 24;
        let src1_offset = 8;
        let src2_offset = 16;
        let mut init_values = vec![0; 24];
        init_values[src1_offset as usize] = src1_val[0] as u32;
        init_values[src2_offset as usize] = src2_val[0] as u32;
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

        // Create the input state.
        let src1_val_new = src1_val
            .iter()
            .map(|s1| AESTowerField8b::from(B8::from(*s1)).val())
            .collect::<Vec<_>>();
        let src2_val_new = src2_val
            .iter()
            .map(|s2| AESTowerField8b::from(B8::from(*s2)).val())
            .collect::<Vec<_>>();
        let init = GroestlShortImpl::state_from_bytes(
            &[src1_val_new, src2_val_new]
                .concat()
                .try_into()
                .expect("src1_val_new and src2_val_new are exactly 32 bytes each"),
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
        let output_state_bytes =
            output_state_bytes.map(|byte| B8::from(binius_field::AESTowerField8b::new(byte)).val());
        let dst_val = GenericArray::<u8, typenum::U32>::from_slice(&output_state_bytes[32..]);
        let dst_val = bytes_to_u32(dst_val);

        let actual_dst_vals = (0..8)
            .map(|i| trace.vrom().read::<u32>(dst_offset + i).unwrap())
            .collect::<Vec<_>>();
        for i in 0..8 {
            assert_eq!(dst_val[i], actual_dst_vals[i]);
        }
    }
}

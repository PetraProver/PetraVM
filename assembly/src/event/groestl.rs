use binius_hash::{
    compression,
    groestl::{Groestl256ByteCompression, GroestlShortImpl, GroestlShortInternal},
    PseudoCompressionFunction,
};
use binius_m3::builder::{B16, B32};
use bytemuck::{cast_slice, Pod};
use generic_array::GenericArray;

use super::{context::EventContext, Event};
use crate::{
    execution::{FramePointer, InterpreterChannels, InterpreterError},
    fire_non_jump_event,
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
    pub dst_val: [u64; 8],
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
        let mut src1_val = Vec::with_capacity(16);
        for i in 0..16 {
            src1_val.push(ctx.vrom_read::<u32>(ctx.addr(src1.val() + i))?);
        }
        let src1_val = cast_slice::<u32, u8>(&src1_val);
        let mut src2_val = Vec::with_capacity(16);
        for i in 0..16 {
            src2_val.push(ctx.vrom_read::<u32>(ctx.addr(src2.val() + i))?);
        }
        let src2_val = cast_slice::<u32, u8>(&src2_val);

        // let mut dst_val =
        // GroestlShortImpl::state_from_bytes(src1_val.try_into().unwrap());
        let mut dst_val = cast_slice::<u8, u64>(&src1_val.to_vec())
            .try_into()
            .unwrap();
        <GroestlShortImpl as GroestlShortInternal>::compress(
            &mut dst_val,
            src2_val.try_into().unwrap(),
        );

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
            src1_val: src1_val.try_into().unwrap(),
            src2: src2.val(),
            src2_val: src2_val.try_into().unwrap(),
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
/// Performs a Groestl compression between two 512-bit inputs.
#[derive(Debug, Clone)]
pub struct Groestl256OutputEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub dst: u16,
    pub dst_val: [u64; 4],
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
        let mut src1_val = Vec::with_capacity(16);
        for i in 0..8 {
            src1_val.push(ctx.vrom_read::<u32>(ctx.addr(src1.val() + i))?);
        }
        let src1_val = cast_slice::<u32, u8>(&src1_val);
        let mut src2_val = Vec::with_capacity(16);
        for i in 0..8 {
            src2_val.push(ctx.vrom_read::<u32>(ctx.addr(src2.val() + i))?);
        }
        let src2_val = cast_slice::<u32, u8>(&src2_val);

        // This actually gives the Groestl output transformation.
        let compression = Groestl256ByteCompression::default();
        let src1_array = GenericArray::from_slice(src1_val);
        let src2_array = GenericArray::from_slice(src2_val);
        let dst_val = compression.compress([*src1_array, *src2_array]);
        let dst_val = cast_slice::<u8, u64>(&dst_val);
        for i in 0..4 {
            ctx.vrom_write(ctx.addr(dst.val() + 2 * i), dst_val[i as usize])?;
        }

        let (_pc, field_pc, fp, timestamp) = ctx.program_state();
        ctx.incr_pc();

        let event = Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_val: dst_val.try_into().unwrap(),
            src1: src1.val(),
            src1_val: src1_val.try_into().unwrap(),
            src2: src2.val(),
            src2_val: src2_val.try_into().unwrap(),
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
    use binius_hash::groestl::Groestl256;
    use generic_array::typenum;
    use num_traits::cast;

    use super::*;
    use crate::{
        execution::Interpreter, isa::RecursionISA, util::code_to_prom, Memory, Opcode, ValueRom,
        ZCrayTrace,
    };

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

        // Construct a simple program with the Groestl25Compress instruction
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
            ZCrayTrace::generate(Box::new(RecursionISA), memory, frames, HashMap::new())
                .expect("Trace generation should not fail.");

        // Capture the final PC before boundary_values is moved
        let final_pc = boundary_values.final_pc;

        // Validate the trace (this consumes boundary_values)
        trace.validate(boundary_values);

        // Calculate the output.
        // let mut dst_val = GroestlShortImpl::state_from_bytes(&src1_val);
        let mut dst_val = cast_slice::<u8, u64>(&src1_val.to_vec())
            .try_into()
            .unwrap();
        <GroestlShortImpl as GroestlShortInternal>::compress(&mut dst_val, &src2_val);

        let actual_dst_vals = (0..8)
            .map(|i| trace.vrom().read::<u64>(dst_offset + 2 * i).unwrap())
            .collect::<Vec<_>>();
        for i in 0..8 {
            assert_eq!(dst_val[i], actual_dst_vals[i]);
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
            ZCrayTrace::generate(Box::new(RecursionISA), memory, frames, HashMap::new())
                .expect("Trace generation should not fail.");

        // Capture the final PC before boundary_values is moved
        let final_pc = boundary_values.final_pc;

        // Validate the trace (this consumes boundary_values)
        trace.validate(boundary_values);

        // Create the input state.
        let init =
            GroestlShortImpl::state_from_bytes(&[src1_val, src2_val].concat().try_into().unwrap());
        let mut state_in = init.clone();
        GroestlShortImpl::p_perm(&mut state_in);

        // Calculate the output: dst_val = P(state_in) XOR init
        let dst_val: [u64; 8] = state_in
            .iter()
            .zip(init.iter())
            .map(|(&x, &y)| x ^ y)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        // Convert dst_val to a big endian representation.
        let output_state_bytes = GroestlShortImpl::state_to_bytes(&dst_val);
        let dst_val = GenericArray::<u8, typenum::U32>::from_slice(&output_state_bytes[32..]);
        let dst_val = cast_slice::<u8, u64>(&dst_val);

        let actual_dst_vals = (0..4)
            .map(|i| trace.vrom().read::<u64>(dst_offset + 2 * i).unwrap())
            .collect::<Vec<_>>();
        for i in 0..4 {
            assert_eq!(dst_val[i], actual_dst_vals[i]);
        }
    }
}

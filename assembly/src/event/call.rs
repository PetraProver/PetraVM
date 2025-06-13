use binius_m3::builder::{B16, B32};

use super::context::EventContext;
use crate::{
    event::Event,
    execution::{FramePointer, InterpreterChannels, InterpreterError, G},
    Opcode,
};

/// Event for TAILI.
///
/// Performs a tail function call to the target address given by an immediate.
///
/// Logic:
///   1. [FP[next_fp] + 0] = FP[0] (return address)
///   2. [FP[next_fp] + 1] = FP[1] (old frame pointer)
///   3. FP = FP[next_fp]
///   4. PC = target
#[derive(Debug, Clone)]
pub struct TailiEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub target: u32,
    pub next_fp: u16,
    pub next_fp_val: u32,
    pub return_addr: u32,
    pub old_fp_val: u16,
}

impl Event for TailiEvent {
    fn generate(
        ctx: &mut EventContext,
        target_low: B16,
        target_high: B16,
        next_fp: B16,
    ) -> Result<(), InterpreterError> {
        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

        // Perform a single packed read to get both u32 values at once.
        let pack = ctx.vrom_read::<u64>(*ctx.fp)?; // no address offset
        let (return_addr, old_fp_val) = { (pack as u32, (pack >> 32) as u32) };

        // Get the target address, to which we should jump.
        let target = B32::new(target_low.val() as u32 + ((target_high.val() as u32) << 16));
        let advice = ctx
            .advice
            .ok_or(InterpreterError::MissingAdvice(Opcode::Taili))?;

        // Allocate a new frame for the call and set the value of the next frame
        // pointer.
        let next_fp_val = ctx.setup_call_frame(next_fp)?;

        // Jump to the target, received as advice.
        ctx.jump_to_u32(target, advice);

        // Perform a single packed write to store both u32 values at once.
        ctx.vrom_write::<u64>(*ctx.fp, pack)?;

        let event = Self {
            pc: field_pc,
            fp,
            timestamp,
            target: target.val(),
            next_fp: next_fp.val(),
            next_fp_val,
            return_addr,
            old_fp_val: old_fp_val as u16,
        };

        ctx.trace.taili.push(event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels
            .state_channel
            .push((B32::new(self.target), self.next_fp_val, self.timestamp));
    }
}

/// Event for TAILV.
///
/// Performs a tail function call to the indirect target address read from VROM.
///
/// Logic:
///   1. [FP[next_fp] + 0] = FP[0] (return address)
///   2. [FP[next_fp] + 1] = FP[1] (old frame pointer)
///   3. FP = FP[next_fp]
///   4. PC = FP[offset]
#[derive(Debug, Clone)]
pub struct TailvEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub offset: u16,
    pub next_fp: u16,
    pub next_fp_val: u32,
    pub return_addr: u32,
    pub old_fp_val: u16,
    pub target: u32,
}

impl Event for TailvEvent {
    fn generate(
        ctx: &mut EventContext,
        offset: B16,
        next_fp: B16,
        _unused: B16,
    ) -> Result<(), InterpreterError> {
        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

        // Perform a single packed read to get both u32 values at once.
        let pack = ctx.vrom_read::<u64>(*ctx.fp)?; // no address offset
        let (return_addr, old_fp_val) = { (pack as u32, (pack >> 32) as u32) };

        // Get the target address, to which we should jump.
        let target = ctx.vrom_read::<u32>(ctx.addr(offset.val()))?;

        // Allocate a new frame for the call and set the value of the next frame
        // pointer.
        let next_fp_val = ctx.setup_call_frame(next_fp)?;

        // Jump to the target,
        ctx.jump_to(B32::new(target));

        // Perform a single packed write to store both u32 values at once.
        ctx.vrom_write::<u64>(*ctx.fp, pack)?;

        let event = Self {
            pc: field_pc,
            fp,
            timestamp,
            offset: offset.val(),
            next_fp: next_fp.val(),
            next_fp_val,
            return_addr,
            old_fp_val: old_fp_val as u16,
            target,
        };

        ctx.trace.tailv.push(event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels
            .state_channel
            .push((B32::new(self.target), self.next_fp_val, self.timestamp));
    }
}

/// Event for CALLI.
///
/// Performs a function call to the target address given by an immediate.
///
/// Logic:
///   1. [FP[next_fp] + 0] = PC + 1 (return address)
///   2. [FP[next_fp] + 1] = FP (old frame pointer)
///   3. FP = FP[next_fp]
///   4. PC = target

#[derive(Debug, Clone)]
pub struct CalliEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub target: u32,
    pub next_fp: u16,
    pub next_fp_val: u32,
}

impl Event for CalliEvent {
    fn generate(
        ctx: &mut EventContext,
        target_low: B16,
        target_high: B16,
        next_fp: B16,
    ) -> Result<(), InterpreterError> {
        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

        let target = B32::new(target_low.val() as u32 + ((target_high.val() as u32) << 16));
        let advice = ctx
            .advice
            .ok_or(InterpreterError::MissingAdvice(Opcode::Calli))?;

        // Allocate a new frame for the call and set the value of the next frame
        // pointer.
        let next_fp_val = ctx.setup_call_frame(next_fp)?;

        // Jump to the target, received as advice.
        ctx.jump_to_u32(target, advice);

        let return_pc = (field_pc * G).val();

        // Perform a single packed write to store both u32 values at once.
        ctx.vrom_write::<u64>(*ctx.fp, return_pc as u64 + ((*fp as u64) << 32))?;

        let event = Self {
            pc: field_pc,
            fp,
            timestamp,
            target: target.val(),
            next_fp: next_fp.val(),
            next_fp_val,
        };

        ctx.trace.calli.push(event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels
            .state_channel
            .push((B32::new(self.target), self.next_fp_val, self.timestamp));
    }
}

/// Event for CALLV.
///
/// Performs a call to the indirect target address read from VROM.
///
/// Logic:
///   1. [FP[next_fp] + 0] = PC + 1 (return address)
///   2. [FP[next_fp] + 1] = FP (old frame pointer)
///   3. FP = FP[next_fp]
///   4. PC = FP[offset]
#[derive(Debug, Clone)]
pub struct CallvEvent {
    pub pc: B32,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub offset: u16,
    pub next_fp: u16,
    pub next_fp_val: u32,
    pub target: u32,
}

impl Event for CallvEvent {
    fn generate(
        ctx: &mut EventContext,
        offset: B16,
        next_fp: B16,
        _unused: B16,
    ) -> Result<(), InterpreterError> {
        let (_pc, field_pc, fp, timestamp) = ctx.program_state();

        // Get the target address, to which we should jump.
        let target = ctx.vrom_read::<u32>(ctx.addr(offset.val()))?;

        // Allocate a new frame for the call and set the value of the next frame
        // pointer.
        let next_fp_val = ctx.setup_call_frame(next_fp)?;

        // Jump to the target,
        ctx.jump_to(B32::new(target));

        let return_pc = (field_pc * G).val();

        // Perform a single packed write to store both u32 values at once.
        ctx.vrom_write::<u64>(*ctx.fp, return_pc as u64 + ((*fp as u64) << 32))?;

        let event = Self {
            pc: field_pc,
            fp,
            timestamp,
            offset: offset.val(),
            next_fp: next_fp.val(),
            next_fp_val,
            target,
        };

        ctx.trace.callv.push(event);
        Ok(())
    }

    fn fire(&self, channels: &mut InterpreterChannels) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels
            .state_channel
            .push((B32::new(self.target), self.next_fp_val, self.timestamp));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use binius_field::{Field, PackedField};
    use binius_m3::builder::{B16, B32};

    use crate::{
        execution::G, isa::GenericISA, opcodes::Opcode, test_util::code_to_prom, Memory,
        PetraTrace, ValueRom,
    };

    #[test]
    fn test_tailv() {
        let zero = B16::zero();

        // Frame:
        // Slot 0: FP
        // Slot 1: PC
        // Slot 2: Target
        // Slot 3: Next_fp
        // Slot 4: unused_dst_addr (should never be written)

        let ret_prom_index = 3;
        let ret_pc = 3;
        let target = G.pow(ret_pc - 1);
        let target_addr = 2.into();
        let next_fp_addr = 3.into();
        let next_fp_size = 2.into();

        let unaccessed_dst_addr = 4.into();
        let unused_imm = 10.into();

        let instructions = vec![
            (
                [
                    Opcode::Alloci.get_field_elt(),
                    next_fp_addr,
                    next_fp_size,
                    zero,
                ],
                true,
            ),
            (
                [
                    Opcode::Tailv.get_field_elt(),
                    target_addr,
                    next_fp_addr,
                    zero,
                ],
                false,
            ),
            // Code that should not be accessed.
            (
                [
                    Opcode::Ldi.get_field_elt(),
                    unaccessed_dst_addr,
                    unused_imm,
                    zero,
                ],
                false,
            ),
            ([Opcode::Ret.get_field_elt(), zero, zero, zero], false),
        ];

        let mut frames = HashMap::new();
        frames.insert(B32::ONE, 5);
        frames.insert(target, 2);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        // Initialize VROM values: offsets 0, 1, and source value at offset 2.
        vrom.write(0, 0u32, false).unwrap();
        vrom.write(1, 0u32, false).unwrap();
        vrom.write(target_addr.val() as u32, target.val(), false)
            .unwrap();

        let mut pc_field_to_index_pc = HashMap::new();
        pc_field_to_index_pc.insert(target, (ret_prom_index, ret_pc as u32));
        let memory = Memory::new(prom, vrom);
        let (trace, _) =
            PetraTrace::generate(Box::new(GenericISA), memory, frames, pc_field_to_index_pc)
                .expect("Trace generation should not fail.");

        // Check that the next frame pointer was set correctly.
        assert_eq!(trace.vrom().read::<u32>(3).unwrap(), 6u32);
        // Check that the load instruction was not executed.
        assert!(trace
            .vrom()
            .read::<u32>(unaccessed_dst_addr.val() as u32)
            .is_err());
    }

    #[test]
    fn test_callv() {
        let zero = B16::zero();

        // Frame:
        // Slot 0: FP
        // Slot 1: PC
        // Slot 2: Target
        // Slot 3: Next_fp
        // Slot 4: dst

        let ret_prom_index = 3;
        let ret_pc = 3;
        let target = G.pow(ret_pc - 1);
        let ldi_prom_index = 2;
        let ldi_pc = 2;
        let ldi = G.pow(ldi_pc - 1);
        let target_addr = 2.into();
        let next_fp_addr = 3.into();

        let dst_addr = 4.into();
        let imm = 10.into();

        // CALLV jumps into a new frame at the RET opcode level. Then we return to the
        // initial frame, at the LDI opcode level.
        let instructions = vec![
            (
                [Opcode::Alloci.get_field_elt(), next_fp_addr, 2.into(), zero],
                true,
            ),
            (
                [
                    Opcode::Callv.get_field_elt(),
                    target_addr,
                    next_fp_addr,
                    zero,
                ],
                false,
            ),
            ([Opcode::Ldi.get_field_elt(), dst_addr, imm, zero], false),
            ([Opcode::Ret.get_field_elt(), zero, zero, zero], false),
        ];

        let mut frames = HashMap::new();
        frames.insert(B32::ONE, 5);
        frames.insert(target, 2);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        // Initialize VROM values: offsets 0, 1, and source value at offset 2.
        vrom.write(0, 0u32, false).unwrap();
        vrom.write(1, 0u32, false).unwrap();
        vrom.write(target_addr.val() as u32, target.val(), false)
            .unwrap();

        let mut pc_field_to_index_pc = HashMap::new();
        pc_field_to_index_pc.insert(target, (ret_prom_index, ret_pc as u32));
        pc_field_to_index_pc.insert(ldi, (ldi_prom_index, ldi_pc as u32));
        let memory = Memory::new(prom, vrom);
        let (trace, _) =
            PetraTrace::generate(Box::new(GenericISA), memory, frames, pc_field_to_index_pc)
                .expect("Trace generation should not fail.");

        assert_eq!(trace.vrom().read::<u32>(3).unwrap(), 6u32);
        // Check that the load instruction was executed.
        assert_eq!(
            trace.vrom().read::<u32>(dst_addr.val() as u32).unwrap(),
            imm.val() as u32
        );
    }
}

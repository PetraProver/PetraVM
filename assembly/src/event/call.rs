use binius_field::{BinaryField16b, BinaryField32b, ExtensionField};

use super::context::EventContext;
use crate::{
    event::Event,
    execution::{
        FramePointer, Interpreter, InterpreterChannels, InterpreterError, InterpreterTables,
        ZCrayTrace, G,
    },
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
    pub pc: BinaryField32b,
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
        target_low: BinaryField16b,
        target_high: BinaryField16b,
        next_fp: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let (pc, field_pc, fp, timestamp) = ctx.program_state();

        let return_addr = ctx.load_vrom_u32(ctx.addr(0u32))?;
        let old_fp_val = ctx.load_vrom_u32(ctx.addr(1u32))?;

        // Get the target address, to which we should jump.
        let target = BinaryField32b::from_bases([target_low, target_high])
            .map_err(|_| InterpreterError::InvalidInput)?;

        // Allocate a new frame for the call and set the value of the next frame
        // pointer.
        let next_fp_val = ctx.setup_call_frame(next_fp, target)?;

        ctx.jump_to(target);

        ctx.store_vrom_u32(ctx.addr(0u32), return_addr)?;
        ctx.store_vrom_u32(ctx.addr(1u32), old_fp_val)?;

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

    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.target),
            self.next_fp_val,
            self.timestamp,
        ));
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
pub struct TailVEvent {
    pub pc: BinaryField32b,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub offset: u16,
    pub next_fp: u16,
    pub next_fp_val: u32,
    pub return_addr: u32,
    pub old_fp_val: u16,
    pub target: u32,
}

impl Event for TailVEvent {
    fn generate(
        ctx: &mut EventContext,
        offset: BinaryField16b,
        next_fp: BinaryField16b,
        _unused: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let (pc, field_pc, fp, timestamp) = ctx.program_state();

        let return_addr = ctx.load_vrom_u32(ctx.addr(0u32))?;
        let old_fp_val = ctx.load_vrom_u32(ctx.addr(1u32))?;

        // Get the target address, to which we should jump.
        let target = ctx.load_vrom_u32(ctx.addr(offset.val()))?;

        // Allocate a new frame for the call and set the value of the next frame
        // pointer.
        let next_fp_val = ctx.setup_call_frame(next_fp, target.into())?;

        // Jump to the target,
        ctx.jump_to(BinaryField32b::new(target));

        ctx.store_vrom_u32(ctx.addr(0u32), return_addr)?;
        ctx.store_vrom_u32(ctx.addr(1u32), old_fp_val)?;

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

    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.target),
            self.next_fp_val,
            self.timestamp,
        ));
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
    pub pc: BinaryField32b,
    pub fp: FramePointer,
    pub timestamp: u32,
    pub target: u32,
    pub next_fp: u16,
    pub next_fp_val: u32,
}

impl Event for CalliEvent {
    fn generate(
        ctx: &mut EventContext,
        target_low: BinaryField16b,
        target_high: BinaryField16b,
        next_fp: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let (pc, field_pc, fp, timestamp) = ctx.program_state();

        let target = BinaryField32b::from_bases([target_low, target_high])
            .map_err(|_| InterpreterError::InvalidInput)?;

        // Allocate a new frame for the call and set the value of the next frame
        // pointer.
        let next_fp_val = ctx.setup_call_frame(next_fp, target)?;

        ctx.jump_to(target);

        let return_pc = (field_pc * G).val();
        ctx.store_vrom_u32(ctx.addr(0u32), return_pc)?;
        ctx.store_vrom_u32(ctx.addr(1u32), *fp)?;

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

    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.target),
            self.next_fp_val,
            self.timestamp,
        ));
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
    pub pc: BinaryField32b,
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
        offset: BinaryField16b,
        next_fp: BinaryField16b,
        _unused: BinaryField16b,
    ) -> Result<(), InterpreterError> {
        let (pc, field_pc, fp, timestamp) = ctx.program_state();

        // Get the target address, to which we should jump.
        let target = ctx.load_vrom_u32(ctx.addr(offset.val()))?;

        // Allocate a new frame for the call and set the value of the next frame
        // pointer.
        let next_fp_val = ctx.setup_call_frame(next_fp, target.into())?;

        // Jump to the target,
        ctx.jump_to(BinaryField32b::new(target));

        let return_pc = (field_pc * G).val();
        ctx.store_vrom_u32(ctx.addr(0u32), return_pc)?;
        ctx.store_vrom_u32(ctx.addr(1u32), *fp)?;

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

    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, *self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.target),
            self.next_fp_val,
            self.timestamp,
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use binius_field::{BinaryField16b, BinaryField32b, Field, PackedField};

    use crate::{execution::G, opcodes::Opcode, util::code_to_prom, Memory, ValueRom, ZCrayTrace};

    #[test]
    fn test_tailv() {
        let zero = BinaryField16b::zero();

        // Frame:
        // Slot 0: FP
        // Slot 1: PC
        // Slot 2: Target
        // Slot 3: Next_fp
        // Slot 4: unused_dst_addr (should never be written)

        let ret_pc = 3;
        let target = G.pow(ret_pc - 1);
        let target_addr = 2.into();
        let next_fp_addr = 3.into();

        let unaccessed_dst_addr = 4.into();
        let unused_imm = 10.into();

        let instructions = vec![
            [
                Opcode::Tailv.get_field_elt(),
                target_addr,
                next_fp_addr,
                zero,
            ],
            // Code that should not be accessed.
            [
                Opcode::Ldi.get_field_elt(),
                unaccessed_dst_addr,
                unused_imm,
                zero,
            ],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, 5);
        frames.insert(target, 2);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        // Initialize VROM values: offsets 0, 1, and source value at offset 2.
        vrom.set_u32(0, 0).unwrap();
        vrom.set_u32(1, 0).unwrap();
        vrom.set_u32(target_addr.val() as u32, target.val())
            .unwrap();

        let mut pc_field_to_int = HashMap::new();
        pc_field_to_int.insert(target, ret_pc as u32);
        let memory = Memory::new(prom, vrom);
        let (trace, _) = ZCrayTrace::generate(memory, frames, pc_field_to_int)
            .expect("Trace generation should not fail.");

        // Check that there are no MOVE events that have yet to be executed.
        assert!(trace.vrom_pending_updates().is_empty());
        // Check that the next frame pointer was set correctly.
        assert_eq!(trace.get_vrom_u32(3).unwrap(), 6u32);
        // Check that the load instruction was not executed.
        assert!(trace
            .get_vrom_u32(unaccessed_dst_addr.val() as u32)
            .is_err());
    }

    #[test]
    fn test_callv() {
        let zero = BinaryField16b::zero();

        // Frame:
        // Slot 0: FP
        // Slot 1: PC
        // Slot 2: Target
        // Slot 3: Next_fp
        // Slot 4: dst

        let ret_pc = 3;
        let target = G.pow(ret_pc - 1);
        let ldi_pc = 2;
        let ldi = G.pow(ldi_pc - 1);
        let target_addr = 2.into();
        let next_fp_addr = 3.into();

        let dst_addr = 4.into();
        let imm = 10.into();

        // CALLV jumps into a new frame at the RET opcode level. Then we return to the
        // initial frame, at the LDI opcode level.
        let instructions = vec![
            [
                Opcode::Callv.get_field_elt(),
                target_addr,
                next_fp_addr,
                zero,
            ],
            [Opcode::Ldi.get_field_elt(), dst_addr, imm, zero],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];

        let mut frames = HashMap::new();
        frames.insert(BinaryField32b::ONE, 5);
        frames.insert(target, 2);

        let prom = code_to_prom(&instructions);
        let mut vrom = ValueRom::default();
        // Initialize VROM values: offsets 0, 1, and source value at offset 2.
        vrom.set_u32(0, 0).unwrap();
        vrom.set_u32(1, 0).unwrap();
        vrom.set_u32(target_addr.val() as u32, target.val())
            .unwrap();

        let mut pc_field_to_int = HashMap::new();
        pc_field_to_int.insert(target, ret_pc as u32);
        pc_field_to_int.insert(ldi, ldi_pc as u32);
        let memory = Memory::new(prom, vrom);
        let (trace, _) = ZCrayTrace::generate(memory, frames, pc_field_to_int)
            .expect("Trace generation should not fail.");

        assert!(trace.vrom_pending_updates().is_empty());
        assert_eq!(trace.get_vrom_u32(3).unwrap(), 6u32);
        // Check that the load instruction was executed.
        assert_eq!(
            trace.get_vrom_u32(dst_addr.val() as u32).unwrap(),
            imm.val() as u32
        );
    }
}

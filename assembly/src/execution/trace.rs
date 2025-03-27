//! This module stores all `Event`s generated during a program execution and
//! generates the associated execution trace.

use std::collections::HashMap;

use binius_field::{BinaryField32b, Field, PackedField};

#[cfg(test)]
use crate::memory::VromPendingUpdates;
use crate::{
    assembler::LabelsFrameSizes,
    event::{
        b128::{B128AddEvent, B128MulEvent},
        b32::{
            AndEvent, AndiEvent, B32MulEvent, B32MuliEvent, OrEvent, OriEvent, XorEvent, XoriEvent,
        },
        branch::{BnzEvent, BzEvent},
        call::{CalliEvent, CallvEvent, TailVEvent, TailiEvent},
        integer_ops::{
            Add32Gadget, Add64Gadget, AddEvent, AddiEvent, MulOp, MuliEvent, MulsuOp, MuluEvent,
            SignedMulEvent, SltEvent, SltiEvent, SltiuEvent, SltuEvent, SubEvent,
        },
        jump::{JumpiEvent, JumpvEvent},
        mv::{LDIEvent, MVEventOutput, MVIHEvent, MVVLEvent, MVVWEvent},
        ret::RetEvent,
        shift::{self, ShiftEvent},
        Event,
    },
    execution::{Interpreter, InterpreterChannels, InterpreterError, InterpreterTables, G},
    memory::{Memory, MemoryError, ProgramRom, ValueRom, VromUpdate},
};
#[derive(Debug, Default)]
pub struct ZCrayTrace {
    pub(crate) bnz: Vec<BnzEvent>,
    pub(crate) jumpi: Vec<JumpiEvent>,
    pub(crate) jumpv: Vec<JumpvEvent>,
    pub(crate) xor: Vec<XorEvent>,
    pub(crate) bz: Vec<BzEvent>,
    pub(crate) or: Vec<OrEvent>,
    pub(crate) ori: Vec<OriEvent>,
    pub(crate) xori: Vec<XoriEvent>,
    pub(crate) and: Vec<AndEvent>,
    pub(crate) andi: Vec<AndiEvent>,
    pub(crate) sub: Vec<SubEvent>,
    pub(crate) slt: Vec<SltEvent>,
    pub(crate) slti: Vec<SltiEvent>,
    pub(crate) sltu: Vec<SltuEvent>,
    pub(crate) sltiu: Vec<SltiuEvent>,
    // TODO(Robin): Re-unify shifts
    pub(crate) imm_logic_left_shift: Vec<ShiftEvent<shift::ImmediateShift, shift::LogicalLeft>>,
    pub(crate) off_logic_left_shift: Vec<ShiftEvent<shift::VromOffsetShift, shift::LogicalLeft>>,
    pub(crate) imm_logic_right_shift: Vec<ShiftEvent<shift::ImmediateShift, shift::LogicalRight>>,
    pub(crate) off_logic_right_shift: Vec<ShiftEvent<shift::VromOffsetShift, shift::LogicalRight>>,
    pub(crate) imm_arith_right_shift:
        Vec<ShiftEvent<shift::ImmediateShift, shift::ArithmeticRight>>,
    pub(crate) off_arith_right_shift:
        Vec<ShiftEvent<shift::VromOffsetShift, shift::ArithmeticRight>>,
    pub(crate) add: Vec<AddEvent>,
    pub(crate) addi: Vec<AddiEvent>,
    pub(crate) add32: Vec<Add32Gadget>,
    pub(crate) add64: Vec<Add64Gadget>,
    pub(crate) muli: Vec<MuliEvent>,
    // TODO(Robin): Re-unify mul / mulsu
    pub(crate) signed_mul: Vec<SignedMulEvent<MulOp>>,
    pub(crate) signed_mulsu: Vec<SignedMulEvent<MulsuOp>>,
    pub(crate) mulu: Vec<MuluEvent>,
    pub(crate) taili: Vec<TailiEvent>,
    pub(crate) tailv: Vec<TailVEvent>,
    pub(crate) calli: Vec<CalliEvent>,
    pub(crate) callv: Vec<CallvEvent>,
    pub(crate) ret: Vec<RetEvent>,
    pub(crate) mvih: Vec<MVIHEvent>,
    pub(crate) mvvw: Vec<MVVWEvent>,
    pub(crate) mvvl: Vec<MVVLEvent>,
    pub(crate) ldi: Vec<LDIEvent>,
    pub(crate) b32_mul: Vec<B32MulEvent>,
    pub(crate) b32_muli: Vec<B32MuliEvent>,
    pub(crate) b128_add: Vec<B128AddEvent>,
    pub(crate) b128_mul: Vec<B128MulEvent>,

    memory: Memory,
}

pub struct BoundaryValues {
    pub(crate) final_pc: BinaryField32b,
    pub(crate) final_fp: u32,
    pub(crate) timestamp: u32,
}

/// Convenience macro to `fire` all events logged.
/// This will execute all the flushes that these events trigger.
#[macro_use]
macro_rules! fire_events {
    ($events:expr, $channels:expr, $tables:expr) => {
        $events
            .iter()
            .for_each(|event| event.fire($channels, $tables));
    };
}

impl ZCrayTrace {
    pub(crate) fn new(memory: Memory) -> Self {
        Self {
            memory,
            ..Default::default()
        }
    }

    pub(crate) const fn prom(&self) -> &ProgramRom {
        self.memory.prom()
    }

    pub fn generate(
        memory: Memory,
        frames: LabelsFrameSizes,
        pc_field_to_int: HashMap<BinaryField32b, u32>,
    ) -> Result<(Self, BoundaryValues), InterpreterError> {
        let mut interpreter = Interpreter::new(frames, pc_field_to_int);

        let mut trace = interpreter.run(memory)?;

        let final_pc = if interpreter.pc == 0 {
            BinaryField32b::zero()
        } else {
            G.pow(interpreter.pc as u64)
        };

        let boundary_values = BoundaryValues {
            final_pc,
            final_fp: interpreter.fp,
            timestamp: interpreter.timestamp,
        };
        Ok((trace, boundary_values))
    }

    pub fn validate(&self, boundary_values: BoundaryValues) {
        let mut channels = InterpreterChannels::default();

        let tables = InterpreterTables::default();

        // Initial boundary push: PC = 1, FP = 0, TIMESTAMP = 0.
        channels.state_channel.push((BinaryField32b::ONE, 0, 0));
        // Final boundary pull.
        channels.state_channel.pull((
            boundary_values.final_pc,
            boundary_values.final_fp,
            boundary_values.timestamp,
        ));

        fire_events!(self.bnz, &mut channels, &tables);
        fire_events!(self.jumpi, &mut channels, &tables);
        fire_events!(self.jumpv, &mut channels, &tables);
        fire_events!(self.xor, &mut channels, &tables);
        fire_events!(self.bz, &mut channels, &tables);
        fire_events!(self.or, &mut channels, &tables);
        fire_events!(self.ori, &mut channels, &tables);
        fire_events!(self.xori, &mut channels, &tables);
        fire_events!(self.and, &mut channels, &tables);
        fire_events!(self.andi, &mut channels, &tables);
        fire_events!(self.sub, &mut channels, &tables);
        fire_events!(self.slt, &mut channels, &tables);
        fire_events!(self.slti, &mut channels, &tables);
        fire_events!(self.sltu, &mut channels, &tables);
        fire_events!(self.sltiu, &mut channels, &tables);
        fire_events!(self.imm_logic_left_shift, &mut channels, &tables);
        fire_events!(self.off_logic_left_shift, &mut channels, &tables);
        fire_events!(self.imm_logic_right_shift, &mut channels, &tables);
        fire_events!(self.off_logic_right_shift, &mut channels, &tables);
        fire_events!(self.imm_arith_right_shift, &mut channels, &tables);
        fire_events!(self.off_arith_right_shift, &mut channels, &tables);
        fire_events!(self.add, &mut channels, &tables);
        fire_events!(self.addi, &mut channels, &tables);
        // add32 gadgets do not incur any flushes
        // add64 gadgets do not incur any flushes
        fire_events!(self.muli, &mut channels, &tables);
        fire_events!(self.signed_mul, &mut channels, &tables);
        fire_events!(self.signed_mulsu, &mut channels, &tables);
        fire_events!(self.mulu, &mut channels, &tables);
        fire_events!(self.taili, &mut channels, &tables);
        fire_events!(self.tailv, &mut channels, &tables);
        fire_events!(self.calli, &mut channels, &tables);
        fire_events!(self.callv, &mut channels, &tables);
        fire_events!(self.ret, &mut channels, &tables);
        fire_events!(self.mvih, &mut channels, &tables);
        fire_events!(self.mvvw, &mut channels, &tables);
        fire_events!(self.mvvl, &mut channels, &tables);
        fire_events!(self.ldi, &mut channels, &tables);
        fire_events!(self.b32_mul, &mut channels, &tables);
        fire_events!(self.b32_muli, &mut channels, &tables);
        fire_events!(self.b128_add, &mut channels, &tables);
        fire_events!(self.b128_mul, &mut channels, &tables);

        assert!(channels.state_channel.is_balanced());
    }

    /// Sets a u32 value at the specified index.
    pub(crate) fn set_vrom_u32(&mut self, index: u32, value: u32) -> Result<(), MemoryError> {
        self.memory.set_vrom_u32(index, value)?;

        if let Some(pending_updates) = self.memory.vrom_pending_updates_mut().remove(&index) {
            for pending_update in pending_updates {
                let (parent, opcode, field_pc, fp, timestamp, dst, src, offset) = pending_update;
                self.set_vrom_u32(parent, value);
                let event_out = MVEventOutput::new(
                    parent,
                    opcode,
                    field_pc,
                    fp,
                    timestamp,
                    dst,
                    src,
                    offset,
                    value as u128,
                );
                event_out.push_mv_event(self);
            }
        }

        Ok(())
    }

    /// Sets a u64 value at the specified index.
    pub(crate) fn set_vrom_u64(&mut self, index: u32, value: u64) -> Result<(), MemoryError> {
        self.memory.set_vrom_u64(index, value)?;

        if let Some(pending_updates) = self.memory.vrom_pending_updates_mut().remove(&index) {
            for pending_update in pending_updates {
                let (parent, opcode, field_pc, fp, timestamp, dst, src, offset) = pending_update;
                self.set_vrom_u64(parent, value);
                let event_out = MVEventOutput::new(
                    parent,
                    opcode,
                    field_pc,
                    fp,
                    timestamp,
                    dst,
                    src,
                    offset,
                    value as u128,
                );
                event_out.push_mv_event(self);
            }
        }

        Ok(())
    }

    /// Sets a u128 value at the specified index.
    pub(crate) fn set_vrom_u128(&mut self, index: u32, value: u128) -> Result<(), MemoryError> {
        self.memory.set_vrom_u128(index, value)?;

        if let Some(pending_updates) = self.memory.vrom_pending_updates_mut().remove(&index) {
            for pending_update in pending_updates {
                let (parent, opcode, field_pc, fp, timestamp, dst, src, offset) = pending_update;
                self.set_vrom_u128(parent, value)?;
                let event_out = MVEventOutput::new(
                    parent, opcode, field_pc, fp, timestamp, dst, src, offset, value,
                );
                event_out.push_mv_event(self);
            }
        }

        Ok(())
    }

    /// Reads a 32-bit value in VROM at the provided index.
    ///
    /// Returns an error if the value is not found. This method should be used
    /// instead of `get_vrom_opt_u32` everywhere outside of CALL procedures.
    pub fn get_vrom_u32(&self, index: u32) -> Result<u32, MemoryError> {
        self.memory.get_vrom_u32(index)
    }

    /// Reads an optional 32-bit value in VROM at the provided index.
    ///
    /// Used for MOVE operations that are part of a CALL procedure, since the
    /// value to move may not yet be known.
    pub(crate) fn get_vrom_opt_u32(&self, index: u32) -> Result<Option<u32>, MemoryError> {
        self.memory.get_vrom_opt_u32(index)
    }

    /// Reads a 128-bit value in VROM at the provided index.
    ///
    /// Returns an error if the value is not found. This method should be used
    /// instead of `get_vrom_opt_u128` everywhere outside of CALL procedures.
    pub(crate) fn get_vrom_u128(&self, index: u32) -> Result<u128, MemoryError> {
        self.memory.get_vrom_u128(index)
    }

    /// Reads a 64-bit value in VROM at the provided index.
    ///
    /// Returns an error if the value is not found.
    pub(crate) fn get_vrom_u64(&self, index: u32) -> Result<u64, MemoryError> {
        self.memory.get_vrom_u64(index)
    }

    /// Reads an optional 128-bit value in VROM at the provided index.
    ///
    /// Used for MOVE operations that are part of a CALL procedure, since the
    /// value to move may not yet be known.
    pub(crate) fn get_vrom_opt_u128(&self, index: u32) -> Result<Option<u128>, MemoryError> {
        self.memory.get_vrom_opt_u128(index)
    }

    /// Inserts a pending value to be set later.
    ///
    /// Maps a destination address to a `VromUpdate` which contains necessary
    /// information to create a MOVE event once the value is available.
    pub(crate) fn insert_pending(
        &mut self,
        parent: u32,
        pending_value: VromUpdate,
    ) -> Result<(), MemoryError> {
        self.memory.insert_pending(parent, pending_value)?;

        Ok(())
    }

    /// Returns a mutable reference to the VROM.
    pub(crate) fn vrom_mut(&mut self) -> &mut ValueRom {
        self.memory.vrom_mut()
    }

    #[cfg(test)]
    pub(crate) fn vrom_pending_updates(&self) -> &VromPendingUpdates {
        self.memory.vrom_pending_updates()
    }
}

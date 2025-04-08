//! This module stores all `Event`s generated during a program execution and
//! generates the associated execution trace.

use std::collections::HashMap;

use binius_field::{Field, PackedField};
use binius_m3::builder::B32;

use super::FramePointer;
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
            AddEvent, AddiEvent, GenericSignedMulEvent, MulOp, MuliEvent, MulsuOp, MuluEvent,
            SignedMulEvent, SltEvent, SltiEvent, SltiuEvent, SltuEvent, SubEvent,
        },
        jump::{JumpiEvent, JumpvEvent},
        mv::{LDIEvent, MVEventOutput, MVIHEvent, MVVLEvent, MVVWEvent},
        ret::RetEvent,
        shift::{self, GenericShiftEvent, ShiftEvent},
        Event,
    },
    execution::{Interpreter, InterpreterChannels, InterpreterError, InterpreterTables, G},
    gadgets::{Add32Gadget, Add64Gadget},
    memory::{Memory, MemoryError, ProgramRom, Ram, ValueRom, VromUpdate},
};
#[derive(Debug, Default)]
pub struct ZCrayTrace {
    pub bnz: Vec<BnzEvent>,
    pub jumpi: Vec<JumpiEvent>,
    pub jumpv: Vec<JumpvEvent>,
    pub xor: Vec<XorEvent>,
    pub bz: Vec<BzEvent>,
    pub or: Vec<OrEvent>,
    pub ori: Vec<OriEvent>,
    pub xori: Vec<XoriEvent>,
    pub and: Vec<AndEvent>,
    pub andi: Vec<AndiEvent>,
    pub sub: Vec<SubEvent>,
    pub slt: Vec<SltEvent>,
    pub slti: Vec<SltiEvent>,
    pub sltu: Vec<SltuEvent>,
    pub sltiu: Vec<SltiuEvent>,
    pub shifts: Vec<Box<dyn GenericShiftEvent>>,
    pub add: Vec<AddEvent>,
    pub addi: Vec<AddiEvent>,
    pub add32: Vec<Add32Gadget>,
    pub add64: Vec<Add64Gadget>,
    pub muli: Vec<MuliEvent>,
    pub signed_mul: Vec<Box<dyn GenericSignedMulEvent>>,
    pub mulu: Vec<MuluEvent>,
    pub taili: Vec<TailiEvent>,
    pub tailv: Vec<TailVEvent>,
    pub calli: Vec<CalliEvent>,
    pub callv: Vec<CallvEvent>,
    pub ret: Vec<RetEvent>,
    pub mvih: Vec<MVIHEvent>,
    pub mvvw: Vec<MVVWEvent>,
    pub mvvl: Vec<MVVLEvent>,
    pub ldi: Vec<LDIEvent>,
    pub b32_mul: Vec<B32MulEvent>,
    pub b32_muli: Vec<B32MuliEvent>,
    pub b128_add: Vec<B128AddEvent>,
    pub b128_mul: Vec<B128MulEvent>,

    memory: Memory,
}

pub struct BoundaryValues {
    pub final_pc: B32,
    pub final_fp: FramePointer,
    pub timestamp: u32,
}

/// Convenience macro to execute all the flushing rules of a given kind of
/// instructions present in a [`ZCrayTrace`].
///
/// It takes as argument the list events for the targeted instruction in a
/// trace, the [`InterpreterChannels`] against which the flushing rules will be
/// performed, and the [`InterpreterTables`].
///
/// # Example
///
/// ```ignore
/// fire_events!(&trace.bnz, &mut channels, &tables);
/// ```
#[macro_export]
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
        pc_field_to_int: HashMap<B32, u32>,
    ) -> Result<(Self, BoundaryValues), InterpreterError> {
        let mut interpreter = Interpreter::new(frames, pc_field_to_int);

        let mut trace = interpreter.run(memory)?;

        let final_pc = if interpreter.pc == 0 {
            B32::zero()
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
        channels.state_channel.push((B32::ONE, 0, 0));
        // Final boundary pull.
        channels.state_channel.pull((
            boundary_values.final_pc,
            *boundary_values.final_fp,
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
        fire_events!(self.shifts, &mut channels, &tables);
        fire_events!(self.add, &mut channels, &tables);
        fire_events!(self.addi, &mut channels, &tables);
        // add32 gadgets do not incur any flushes
        // add64 gadgets do not incur any flushes
        fire_events!(self.muli, &mut channels, &tables);
        fire_events!(self.signed_mul, &mut channels, &tables);
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

    pub fn vrom_size(&self) -> usize {
        self.memory.vrom().size()
    }

    /// Sets a u32 value at the specified index.
    pub(crate) fn set_vrom_u32(&mut self, index: u32, value: u32) -> Result<(), MemoryError> {
        self.vrom_mut().write(index, value)?;

        if let Some(pending_updates) = self.memory.vrom_pending_updates_mut().remove(&index) {
            for pending_update in pending_updates {
                let (parent, opcode, field_pc, fp, timestamp, dst, src, offset) = pending_update;
                self.set_vrom_u32(parent, value);
                let event_out = MVEventOutput::new(
                    parent,
                    opcode,
                    field_pc,
                    fp.into(),
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
        self.vrom_mut().write(index, value)?;

        if let Some(pending_updates) = self.memory.vrom_pending_updates_mut().remove(&index) {
            for pending_update in pending_updates {
                let (parent, opcode, field_pc, fp, timestamp, dst, src, offset) = pending_update;
                self.set_vrom_u64(parent, value);
                let event_out = MVEventOutput::new(
                    parent,
                    opcode,
                    field_pc,
                    fp.into(),
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
        self.vrom_mut().write(index, value)?;

        if let Some(pending_updates) = self.memory.vrom_pending_updates_mut().remove(&index) {
            for pending_update in pending_updates {
                let (parent, opcode, field_pc, fp, timestamp, dst, src, offset) = pending_update;
                self.set_vrom_u128(parent, value)?;
                let event_out = MVEventOutput::new(
                    parent,
                    opcode,
                    field_pc,
                    fp.into(),
                    timestamp,
                    dst,
                    src,
                    offset,
                    value,
                );
                event_out.push_mv_event(self);
            }
        }

        Ok(())
    }

    /// Inserts a pending value in VROM to be set later.
    ///
    /// Maps a destination address to a `VromUpdate` which contains necessary
    /// information to create a MOVE event once the value is available.
    pub(crate) fn insert_pending(
        &mut self,
        parent: u32,
        pending_value: VromUpdate,
    ) -> Result<(), MemoryError> {
        self.vrom_mut().insert_pending(parent, pending_value)?;

        Ok(())
    }

    /// Returns a reference to the VROM.
    pub const fn vrom(&self) -> &ValueRom {
        self.memory.vrom()
    }

    /// Returns a mutable reference to the VROM.
    pub(crate) fn vrom_mut(&mut self) -> &mut ValueRom {
        self.memory.vrom_mut()
    }

    /// Returns a  reference to the RAM.
    pub const fn ram(&self) -> &Ram {
        self.memory.ram()
    }

    /// Returns a mutable reference to the RAM.
    pub fn ram_mut(&mut self) -> &mut Ram {
        self.memory.ram_mut()
    }

    #[cfg(test)]
    pub(crate) fn vrom_pending_updates(&self) -> &VromPendingUpdates {
        self.memory.vrom_pending_updates()
    }
}

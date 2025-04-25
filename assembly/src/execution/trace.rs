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
        call::{CalliEvent, CallvEvent, TailiEvent, TailvEvent},
        integer_ops::{
            AddEvent, AddiEvent, GenericSignedMulEvent, MuliEvent, MuluEvent, SltEvent, SltiEvent,
            SltiuEvent, SltuEvent, SubEvent,
        },
        jump::{JumpiEvent, JumpvEvent},
        mv::{LdiEvent, MVEventOutput, MvihEvent, MvvlEvent, MvvwEvent},
        ret::RetEvent,
        shift::GenericShiftEvent,
        Event,
    },
    execution::{Interpreter, InterpreterChannels, InterpreterError, G},
    gadgets::{Add32Gadget, Add64Gadget},
    isa::ISA,
    memory::{Memory, MemoryError, ProgramRom, Ram, ValueRom, VromUpdate, VromValueT},
    AnyShiftEvent, SrliEvent,
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
    // TODO: In the meanwhile I'm adding this, because the srli_events() method must
    // return a reference to a slice.
    pub srli: Vec<SrliEvent>,
    pub add: Vec<AddEvent>,
    pub addi: Vec<AddiEvent>,
    pub add32: Vec<Add32Gadget>,
    pub add64: Vec<Add64Gadget>,
    pub muli: Vec<MuliEvent>,
    pub signed_mul: Vec<Box<dyn GenericSignedMulEvent>>,
    pub mulu: Vec<MuluEvent>,
    pub taili: Vec<TailiEvent>,
    pub tailv: Vec<TailvEvent>,
    pub calli: Vec<CalliEvent>,
    pub callv: Vec<CallvEvent>,
    pub ret: Vec<RetEvent>,
    pub mvih: Vec<MvihEvent>,
    pub mvvw: Vec<MvvwEvent>,
    pub mvvl: Vec<MvvlEvent>,
    pub ldi: Vec<LdiEvent>,
    pub b32_mul: Vec<B32MulEvent>,
    pub b32_muli: Vec<B32MuliEvent>,
    pub b128_add: Vec<B128AddEvent>,
    pub b128_mul: Vec<B128MulEvent>,

    memory: Memory,
    /// A map of an instruction's field PC to the number of times that
    /// instruction has been executed.
    pub instruction_counter: HashMap<B32, u32>,
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
/// fire_events!(&trace.bnz, &mut channels);
/// ```
#[macro_export]
macro_rules! fire_events {
    ($events:expr, $channels:expr) => {
        $events.iter().for_each(|event| event.fire($channels));
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
        isa: Box<dyn ISA>,
        memory: Memory,
        frames: LabelsFrameSizes,
        pc_field_to_int: HashMap<B32, u32>,
    ) -> Result<(Self, BoundaryValues), InterpreterError> {
        let mut interpreter = Interpreter::new(isa, frames, pc_field_to_int);

        let mut trace = interpreter.run(memory)?;
        // FIXME: I'm doing this for now, but we should probably find a better way.
        trace.srli = trace
            .shifts
            .iter()
            .filter_map(|event| match event.as_any() {
                AnyShiftEvent::Srli(event) => Some(event),
                _ => None,
            })
            .collect();

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

        // Initial boundary push: PC = 1, FP = 0, TIMESTAMP = 0.
        channels.state_channel.push((B32::ONE, 0, 0));
        // Final boundary pull.
        channels.state_channel.pull((
            boundary_values.final_pc,
            *boundary_values.final_fp,
            boundary_values.timestamp,
        ));

        fire_events!(self.bnz, &mut channels);
        fire_events!(self.jumpi, &mut channels);
        fire_events!(self.jumpv, &mut channels);
        fire_events!(self.xor, &mut channels);
        fire_events!(self.bz, &mut channels);
        fire_events!(self.or, &mut channels);
        fire_events!(self.ori, &mut channels);
        fire_events!(self.xori, &mut channels);
        fire_events!(self.and, &mut channels);
        fire_events!(self.andi, &mut channels);
        fire_events!(self.sub, &mut channels);
        fire_events!(self.slt, &mut channels);
        fire_events!(self.slti, &mut channels);
        fire_events!(self.sltu, &mut channels);
        fire_events!(self.sltiu, &mut channels);
        fire_events!(self.shifts, &mut channels);
        fire_events!(self.add, &mut channels);
        fire_events!(self.addi, &mut channels);
        // add32 gadgets do not incur any flushes
        // add64 gadgets do not incur any flushes
        fire_events!(self.muli, &mut channels);
        fire_events!(self.signed_mul, &mut channels);
        fire_events!(self.mulu, &mut channels);
        fire_events!(self.taili, &mut channels);
        fire_events!(self.tailv, &mut channels);
        fire_events!(self.calli, &mut channels);
        fire_events!(self.callv, &mut channels);
        fire_events!(self.ret, &mut channels);
        fire_events!(self.mvih, &mut channels);
        fire_events!(self.mvvw, &mut channels);
        fire_events!(self.mvvl, &mut channels);
        fire_events!(self.ldi, &mut channels);
        fire_events!(self.b32_mul, &mut channels);
        fire_events!(self.b32_muli, &mut channels);
        fire_events!(self.b128_add, &mut channels);
        fire_events!(self.b128_mul, &mut channels);

        assert!(channels.state_channel.is_balanced());
    }

    pub fn vrom_size(&self) -> usize {
        self.memory.vrom().size()
    }

    /// Sets a value of one of the supported types at the provided index in
    /// VROM.
    ///
    /// This will also execute pending VROM updates if necessary.
    pub(crate) fn vrom_write<T: VromValueT>(
        &mut self,
        index: u32,
        value: T,
    ) -> Result<(), MemoryError> {
        self.vrom_mut().write(index, value)?;
        if let Some(pending_updates) = self.memory.vrom_pending_updates_mut().remove(&index) {
            for pending_update in pending_updates {
                let (parent, opcode, field_pc, fp, timestamp, dst, dst_addr, src, offset) =
                    pending_update;
                self.vrom_write(parent, value)?;
                let event_out = MVEventOutput::new(
                    opcode,
                    field_pc,
                    fp.into(),
                    timestamp,
                    dst,
                    dst_addr,
                    src,
                    offset,
                    value.to_u128(),
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

    pub(crate) fn record_instruction(&mut self, field_pc: B32) {
        *self.instruction_counter.entry(field_pc).or_insert(0) += 1;
    }
}

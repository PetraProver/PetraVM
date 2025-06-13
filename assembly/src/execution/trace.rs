//! This module stores all `Event`s generated during a program execution and
//! generates the associated execution trace.

use std::collections::HashMap;

use binius_field::{Field, PackedField};
use binius_m3::builder::B32;

use super::FramePointer;
use crate::{
    assembler::LabelsFrameSizes,
    event::{
        b128::{B128AddEvent, B128MulEvent},
        b32::{
            AndEvent, AndiEvent, B32MulEvent, B32MuliEvent, OrEvent, OriEvent, XorEvent, XoriEvent,
        },
        branch::{BnzEvent, BzEvent},
        call::{CalliEvent, CallvEvent, TailiEvent, TailvEvent},
        comparison::{
            SleEvent, SleiEvent, SleiuEvent, SleuEvent, SltEvent, SltiEvent, SltiuEvent, SltuEvent,
        },
        fp::FpEvent,
        gadgets::right_logic_shift::RightLogicShiftGadgetEvent,
        integer_ops::{AddEvent, AddiEvent, MulEvent, MuliEvent, MulsuEvent, MuluEvent, SubEvent},
        jump::{JumpiEvent, JumpvEvent},
        mv::{LdiEvent, MvihEvent, MvvlEvent, MvvwEvent},
        ret::RetEvent,
        shift::{SllEvent, SlliEvent, SraEvent, SraiEvent, SrlEvent, SrliEvent},
        Event,
    },
    execution::{Interpreter, InterpreterChannels, InterpreterError, G},
    isa::ISA,
    memory::{Memory, MemoryError, ProgramRom, Ram, ValueRom, VromValueT},
};

#[derive(Debug, Default)]
pub struct PetraTrace {
    pub fp: Vec<FpEvent>,
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
    pub sle: Vec<SleEvent>,
    pub slei: Vec<SleiEvent>,
    pub sleu: Vec<SleuEvent>,
    pub sleiu: Vec<SleiuEvent>,
    pub sltu: Vec<SltuEvent>,
    pub sltiu: Vec<SltiuEvent>,
    pub srli: Vec<SrliEvent>,
    pub slli: Vec<SlliEvent>,
    pub srai: Vec<SraiEvent>,
    pub sll: Vec<SllEvent>,
    pub srl: Vec<SrlEvent>,
    pub sra: Vec<SraEvent>,
    pub add: Vec<AddEvent>,
    pub addi: Vec<AddiEvent>,
    pub muli: Vec<MuliEvent>,
    pub mul: Vec<MulEvent>,
    pub mulsu: Vec<MulsuEvent>,
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
    /// A vector recording the number of times an instruction has been executed.
    pub instruction_counter: Vec<u32>,

    pub right_logic_shift_gadget: Vec<RightLogicShiftGadgetEvent>,
}

pub struct BoundaryValues {
    pub final_pc: B32,
    pub final_fp: FramePointer,
    pub timestamp: u32,
}

/// Convenience macro to execute all the flushing rules of a given kind of
/// instructions present in a [`PetraTrace`].
///
/// It takes as argument the list of events for the targeted instruction in a
/// trace and the [`InterpreterChannels`] against which the flushing rules will
/// be performed.
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

impl PetraTrace {
    pub(crate) fn new(memory: Memory) -> Self {
        let prom_size = memory.prom().len();
        Self {
            memory,
            instruction_counter: vec![0; prom_size],
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
        pc_field_to_index_pc: HashMap<B32, (u32, u32)>,
    ) -> Result<(Self, BoundaryValues), InterpreterError> {
        let mut interpreter = Interpreter::new(isa, frames, pc_field_to_index_pc);

        let trace = interpreter.run(memory)?;

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
        fire_events!(self.fp, &mut channels);
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
        fire_events!(self.sle, &mut channels);
        fire_events!(self.slei, &mut channels);
        fire_events!(self.sleu, &mut channels);
        fire_events!(self.sleiu, &mut channels);
        fire_events!(self.slt, &mut channels);
        fire_events!(self.slti, &mut channels);
        fire_events!(self.sltu, &mut channels);
        fire_events!(self.sltiu, &mut channels);
        fire_events!(self.slli, &mut channels);
        fire_events!(self.srli, &mut channels);
        fire_events!(self.srai, &mut channels);
        fire_events!(self.sll, &mut channels);
        fire_events!(self.srl, &mut channels);
        fire_events!(self.sra, &mut channels);
        fire_events!(self.add, &mut channels);
        fire_events!(self.addi, &mut channels);
        fire_events!(self.muli, &mut channels);
        fire_events!(self.mul, &mut channels);
        fire_events!(self.mulsu, &mut channels);
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

    pub const fn vrom_size(&self) -> usize {
        self.memory.vrom().size()
    }

    /// Sets a value of one of the supported types at the provided index in
    /// VROM.
    pub(crate) fn vrom_write<T>(
        &mut self,
        index: u32,
        value: T,
        record: bool,
    ) -> Result<(), MemoryError>
    where
        T: VromValueT,
    {
        self.vrom_mut().write(index, value, record)
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

    pub(crate) fn record_instruction(&mut self, pc: u32) {
        self.instruction_counter[pc as usize - 1] += 1;
    }
}

//! This module stores all `Event`s generated during a program execution and
//! generates the associated execution trace.

use std::collections::HashMap;

use binius_field::{BinaryField32b, Field, PackedField};

use crate::{
    event::{
        b128::{B128AddEvent, B128MulEvent},
        b32::{
            AndEvent, AndiEvent, B32MulEvent, B32MuliEvent, OrEvent, OriEvent, XorEvent, XoriEvent,
        },
        branch::{BnzEvent, BzEvent},
        call::{TailVEvent, TailiEvent},
        integer_ops::{Add32Event, Add64Event, AddEvent, AddiEvent, MuliEvent},
        mv::{LDIEvent, MVIHEvent, MVVLEvent, MVVWEvent},
        ret::RetEvent,
        sli::SliEvent,
        Event,
    },
    execution::{Interpreter, InterpreterChannels, InterpreterError, InterpreterTables},
    memory::Memory,
    parser::LabelsFrameSizes,
    ProgramRom, ValueRom, G,
};

#[derive(Debug, Default)]
pub(crate) struct ZCrayTrace {
    pub(crate) bnz: Vec<BnzEvent>,
    pub(crate) xor: Vec<XorEvent>,
    pub(crate) bz: Vec<BzEvent>,
    pub(crate) or: Vec<OrEvent>,
    pub(crate) ori: Vec<OriEvent>,
    pub(crate) xori: Vec<XoriEvent>,
    pub(crate) and: Vec<AndEvent>,
    pub(crate) andi: Vec<AndiEvent>,
    pub(crate) shift: Vec<SliEvent>,
    pub(crate) add: Vec<AddEvent>,
    pub(crate) addi: Vec<AddiEvent>,
    pub(crate) add32: Vec<Add32Event>,
    pub(crate) add64: Vec<Add64Event>,
    pub(crate) muli: Vec<MuliEvent>,
    pub(crate) taili: Vec<TailiEvent>,
    pub(crate) tailv: Vec<TailVEvent>,
    pub(crate) ret: Vec<RetEvent>,
    pub(crate) mvih: Vec<MVIHEvent>,
    pub(crate) mvvw: Vec<MVVWEvent>,
    pub(crate) mvvl: Vec<MVVLEvent>,
    pub(crate) ldi: Vec<LDIEvent>,
    pub(crate) b32_mul: Vec<B32MulEvent>,
    pub(crate) b32_muli: Vec<B32MuliEvent>,
    pub(crate) b128_add: Vec<B128AddEvent>,
    pub(crate) b128_mul: Vec<B128MulEvent>,

    pub(crate) memory: Memory,
}

pub(crate) struct BoundaryValues {
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

    pub const fn prom(&self) -> &ProgramRom {
        self.memory.prom()
    }

    pub(crate) fn generate(
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

    pub(crate) fn validate(&self, boundary_values: BoundaryValues) {
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
        fire_events!(self.xor, &mut channels, &tables);
        fire_events!(self.bz, &mut channels, &tables);
        fire_events!(self.or, &mut channels, &tables);
        fire_events!(self.ori, &mut channels, &tables);
        fire_events!(self.xori, &mut channels, &tables);
        fire_events!(self.and, &mut channels, &tables);
        fire_events!(self.andi, &mut channels, &tables);
        fire_events!(self.shift, &mut channels, &tables);
        fire_events!(self.add, &mut channels, &tables);
        fire_events!(self.addi, &mut channels, &tables);
        fire_events!(self.add32, &mut channels, &tables);
        fire_events!(self.add64, &mut channels, &tables);
        fire_events!(self.muli, &mut channels, &tables);
        fire_events!(self.taili, &mut channels, &tables);
        fire_events!(self.tailv, &mut channels, &tables);
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
}

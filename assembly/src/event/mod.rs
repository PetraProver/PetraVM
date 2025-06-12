//! Defines execution events for the Petra VM.
//!
//! Each instruction executed by the VM, such as arithmetic operations,
//! branching, or function calls, results in an `Event`, which records the state
//! changes of that instruction.
//! These events are then used to fill the respective tables during the
//! proof generation.
//!
//! This module defines a generic [`Event`] trait to be implemented by all
//! supported instructions.

use binius_m3::builder::B16;
use context::EventContext;
use tracing::instrument;

use crate::{
    execution::{InterpreterChannels, InterpreterError},
    stats::AllCycleStats,
    Opcode,
};

pub(crate) mod alloc;
pub(crate) mod binary_ops;
pub(crate) mod branch;
pub(crate) mod call;
pub(crate) mod comparison;
pub(crate) mod context;
pub(crate) mod fp;
pub(crate) mod gadgets;
pub(crate) mod integer_ops;
pub(crate) mod jump;
#[macro_use]
pub(crate) mod macros;
pub(crate) mod mv;
pub(crate) mod ret;
pub(crate) mod shift;

pub(crate) use binary_ops::{b128, b32};

// Re-exports
pub use self::{
    alloc::{AllociEvent, AllocvEvent},
    b128::{B128AddEvent, B128MulEvent},
    b32::{AndEvent, AndiEvent, B32MulEvent, B32MuliEvent, OrEvent, OriEvent, XorEvent, XoriEvent},
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
};

/// An `Event` represents an instruction that can be executed by the VM.
///
/// This trait is implemented by every instruction supported by the VM
/// Instruction Set.
pub trait Event {
    /// Generates a new event and pushes it to its corresponding list in the set
    /// of traces.
    fn generate(
        ctx: &mut EventContext,
        arg0: B16,
        arg1: B16,
        arg2: B16,
    ) -> Result<(), InterpreterError>
    where
        Self: Sized;

    /// Executes the flushing rules associated to this `Event`, pushing to /
    /// pulling from their target channels.
    fn fire(&self, channels: &mut InterpreterChannels);
}

impl Opcode {
    /// Generates the appropriate event for this opcode.
    #[instrument(
        level = "trace",
        skip(ctx),
        fields(
            arg0 = %format!("0x{:x}", arg0.val()),
            arg1 = %format!("0x{:x}", arg1.val()),
            arg2 = %format!("0x{:x}", arg2.val()),
        )
    )]
    pub(crate) fn generate_event(
        self,
        ctx: &mut EventContext,
        arg0: B16,
        arg1: B16,
        arg2: B16,
        all_cycles: &mut AllCycleStats,
    ) -> Result<(), InterpreterError> {
        match self {
            Opcode::Fp => all_cycles.record(self, || fp::FpEvent::generate(ctx, arg0, arg1, arg2)),
            Opcode::Bnz => all_cycles.record(self, || BnzEvent::generate(ctx, arg0, arg1, arg2)),
            Opcode::Bz => {
                unreachable!("BzEvent can only be triggered through the Bnz instruction.")
            }
            Opcode::Jumpi => {
                all_cycles.record(self, || jump::JumpiEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Jumpv => {
                all_cycles.record(self, || jump::JumpvEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Xori => {
                all_cycles.record(self, || b32::XoriEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Xor => {
                all_cycles.record(self, || b32::XorEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Slli => {
                all_cycles.record(self, || shift::SlliEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Srli => {
                all_cycles.record(self, || shift::SrliEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Srai => {
                all_cycles.record(self, || shift::SraiEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Sll => {
                all_cycles.record(self, || shift::SllEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Srl => {
                all_cycles.record(self, || shift::SrlEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Sra => {
                all_cycles.record(self, || shift::SraEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Addi => all_cycles.record(self, || {
                integer_ops::AddiEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Add => all_cycles.record(self, || {
                integer_ops::AddEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Sle => all_cycles.record(self, || {
                comparison::SleEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Slei => all_cycles.record(self, || {
                comparison::SleiEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Sleu => all_cycles.record(self, || {
                comparison::SleuEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Sleiu => all_cycles.record(self, || {
                comparison::SleiuEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Slt => all_cycles.record(self, || {
                comparison::SltEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Slti => all_cycles.record(self, || {
                comparison::SltiEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Sltu => all_cycles.record(self, || {
                comparison::SltuEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Sltiu => all_cycles.record(self, || {
                comparison::SltiuEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Muli => all_cycles.record(self, || {
                integer_ops::MuliEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Mulu => all_cycles.record(self, || {
                integer_ops::MuluEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Mulsu => all_cycles.record(self, || {
                integer_ops::MulsuEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Mul => all_cycles.record(self, || {
                integer_ops::MulEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Sub => all_cycles.record(self, || {
                integer_ops::SubEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Ret => {
                all_cycles.record(self, || ret::RetEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Taili => {
                all_cycles.record(self, || call::TailiEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Tailv => {
                all_cycles.record(self, || call::TailvEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Calli => {
                all_cycles.record(self, || call::CalliEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Callv => {
                all_cycles.record(self, || call::CallvEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::And => {
                all_cycles.record(self, || b32::AndEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Andi => {
                all_cycles.record(self, || b32::AndiEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Or => all_cycles.record(self, || b32::OrEvent::generate(ctx, arg0, arg1, arg2)),
            Opcode::Ori => {
                all_cycles.record(self, || b32::OriEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Mvih => {
                all_cycles.record(self, || mv::MvihEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Mvvw => {
                all_cycles.record(self, || mv::MvvwEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Mvvl => {
                all_cycles.record(self, || mv::MvvlEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Ldi => {
                all_cycles.record(self, || mv::LdiEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::B32Mul => {
                all_cycles.record(self, || b32::B32MulEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::B32Muli => {
                all_cycles.record(self, || b32::B32MuliEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::B128Add => {
                all_cycles.record(self, || b128::B128AddEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::B128Mul => {
                all_cycles.record(self, || b128::B128MulEvent::generate(ctx, arg0, arg1, arg2))
            }
            Opcode::Alloci => all_cycles.record(Opcode::Alloci, || {
                alloc::AllociEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Allocv => all_cycles.record(Opcode::Allocv, || {
                alloc::AllocvEvent::generate(ctx, arg0, arg1, arg2)
            }),
            Opcode::Invalid => Err(InterpreterError::InvalidOpcode),
        }
    }
}

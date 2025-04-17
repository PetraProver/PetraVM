//! Defines execution events for the zCray VM.
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

use crate::{
    execution::{InterpreterChannels, InterpreterError},
    Opcode,
};

pub(crate) mod binary_ops;
pub(crate) mod branch;
pub(crate) mod call;
pub(crate) mod context;
pub(crate) mod integer_ops;
pub(crate) mod jump;
pub(crate) mod macros;
pub(crate) mod mv;
pub(crate) mod ret;
pub(crate) mod shift;

pub(crate) use binary_ops::{b128, b32};

// Re-exports
pub use self::{
    b128::{B128AddEvent, B128MulEvent},
    b32::{AndEvent, AndiEvent, B32MulEvent, B32MuliEvent, OrEvent, OriEvent, XorEvent, XoriEvent},
    branch::{BnzEvent, BzEvent},
    call::{CalliEvent, CallvEvent, TailiEvent, TailvEvent},
    integer_ops::{
        AddEvent, AddiEvent, MulEvent, MuliEvent, MulsuEvent, MuluEvent, SltEvent, SltiEvent,
        SltiuEvent, SltuEvent, SubEvent,
    },
    jump::{JumpiEvent, JumpvEvent},
    mv::{LdiEvent, MvihEvent, MvvlEvent, MvvwEvent},
    ret::RetEvent,
    shift::{AnyShiftEvent, SllEvent, SlliEvent, SraEvent, SraiEvent, SrlEvent, SrliEvent},
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
    pub(crate) fn generate_event(
        &self,
        ctx: &mut EventContext,
        arg0: B16,
        arg1: B16,
        arg2: B16,
    ) -> Result<(), InterpreterError> {
        match self {
            Opcode::Bnz => BnzEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Bz => {
                unreachable!("BzEvent can only be triggered through the Bnz instruction.")
            }
            Opcode::Jumpi => jump::JumpiEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Jumpv => jump::JumpvEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Xori => b32::XoriEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Xor => b32::XorEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Slli => shift::SlliEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Srli => shift::SrliEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Srai => shift::SraiEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Sll => shift::SllEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Srl => shift::SrlEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Sra => shift::SraEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Addi => event_helper::generate_addi(ctx, arg0, arg1, arg2),
            Opcode::Add => event_helper::generate_add(ctx, arg0, arg1, arg2),
            Opcode::Sub => integer_ops::SubEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Slt => integer_ops::SltEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Slti => integer_ops::SltiEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Sltu => integer_ops::SltuEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Sltiu => integer_ops::SltiuEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Muli => integer_ops::MuliEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Mulu => integer_ops::MuluEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Mulsu => integer_ops::MulsuEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Mul => integer_ops::MulEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Ret => ret::RetEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Taili => call::TailiEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Tailv => call::TailvEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Calli => call::CalliEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Callv => call::CallvEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::And => b32::AndEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Andi => b32::AndiEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Or => b32::OrEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Ori => b32::OriEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Mvih => mv::MvihEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Mvvw => mv::MvvwEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Mvvl => mv::MvvlEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Ldi => mv::LdiEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::B32Mul => b32::B32MulEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::B32Muli => b32::B32MuliEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::B128Add => b128::B128AddEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::B128Mul => b128::B128MulEvent::generate(ctx, arg0, arg1, arg2),
            Opcode::Invalid => Err(InterpreterError::InvalidOpcode),
        }
    }
}

mod event_helper {
    use binius_m3::builder::B16;

    use super::{
        branch::{BnzEvent, BzEvent},
        context::EventContext,
        integer_ops::{AddEvent, AddiEvent},
        Event,
    };
    use crate::{execution::InterpreterError, gadgets::Add32Gadget};

    /// Helper method to generate `AddEvent` and associated `Add32Gadget`.
    pub fn generate_add(
        ctx: &mut EventContext,
        dst: B16,
        src1: B16,
        src2: B16,
    ) -> Result<(), InterpreterError> {
        AddEvent::generate(ctx, dst, src1, src2)?;

        // TODO(Robin): Do this directly within AddEvent / AddiEvent?

        // Retrieve event
        let new_add_event = ctx.trace.add.last().expect("Event should have been pushed");

        let new_add32_gadget =
            Add32Gadget::generate_gadget(ctx, new_add_event.src1_val, new_add_event.src2_val);
        ctx.trace.add32.push(new_add32_gadget);

        Ok(())
    }

    /// Helper method to generate `AddiEvent` and associated `Add32Gadget`.
    pub fn generate_addi(
        ctx: &mut EventContext,
        dst: B16,
        src: B16,
        imm: B16,
    ) -> Result<(), InterpreterError> {
        AddiEvent::generate(ctx, dst, src, imm)?;

        // TODO(Robin): Do this directly within AddEvent / AddiEvent?

        // Retrieve event
        let new_addi_event = ctx
            .trace
            .addi
            .last()
            .expect("Event should have been pushed");

        let new_add32_gadget =
            Add32Gadget::generate_gadget(ctx, new_addi_event.src_val, imm.val() as u32);
        ctx.trace.add32.push(new_add32_gadget);

        Ok(())
    }
}

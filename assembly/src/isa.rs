//! Modular Instruction Set Architectures (ISAs) for the zCray Virtual Machine.
//!
//! An ISA defines:
//! - The instructions it supports.
//! - Specific logic associated with it (for instance the notion of RAM).
//!
//! On the prover side, the ISA is fed when initializing a new
//! [`Circuit`](crate::circuit::Circuit), which invokes
//! [`ISA::register_instructions`] to instantiate and wire up all instruction
//! tables needed.

use core::fmt::Debug;
use std::collections::HashSet;

use crate::event::*;
use crate::Opcode;

/// Defines an Instruction Set Architecture for the zCray Virtual Machine.
///
/// Each implementation of this trait should provide the different instructions
/// supported. This can be done easily through the
/// [`register_instruction!`](crate::register_instruction) macro.
pub trait ISA: Debug {
    /// Returns the set of supported opcodes.
    fn supported_opcodes(&self) -> &HashSet<Opcode>;

    /// Convenience method to check if the ISA supports this opcode.
    fn is_supported(&self, opcode: Opcode) -> bool {
        self.supported_opcodes().contains(&opcode)
    }

    // TODO: add other feature markers
}

/// Creates a new ISA and registers all its supported instructions.
///
/// # Example
///
/// ```ignore
/// define_isa!(MinimalISA => [LdiEvent, RetEvent]);
/// ```
#[macro_export]
macro_rules! define_isa {
    (
        $(#[$doc:meta])*
        $isa_ty:ident => [ $( $event_ty:ty ),* $(,)? ]
    ) => {
        #[derive(Debug)]
        $(#[$doc])*
        pub struct $isa_ty;

        impl ISA for $isa_ty {
            fn supported_opcodes(&self) -> &HashSet<Opcode> {
                use once_cell::sync::Lazy;
                static OPCODES: Lazy<HashSet<Opcode>> = Lazy::new(|| {
                    let mut set = HashSet::new();
                    $(
                        set.insert(<$event_ty as $crate::opcodes::InstructionInfo>::opcode());
                    )*
                    set
                });

                &OPCODES
            }
        }
    };
}

// TODO: Implement Recursion VM whenever possible.
// Needs to implement #79.

// define_isa!(
//     /// A minimal ISA for the zCray Virtual Machine,
//     /// tailored for efficient recursion.
//     RecursionISA => [
//         B32MulEvent,
//         B32MuliEvent,
//         B128AddEvent,
//         B128MulEvent,
//         GroestlCompressEvent, // TODO: name TBD
//         GroestlOutputEvent, // TODO: name TBD
//     ]
// );

define_isa!(
    /// The main Instruction Set Architecture (ISA) for the zCray Virtual Machine,
    /// supporting all existing instructions.
    GenericISA => [
        AddEvent,
        AddiEvent,
        AndEvent,
        AndiEvent,
        BnzEvent,
        BzEvent,
        B32MulEvent,
        B32MuliEvent,
        B128AddEvent,
        B128MulEvent,
        CalliEvent,
        CallvEvent,
        JumpiEvent,
        JumpvEvent,
        LdiEvent,
        MulEvent,
        MuliEvent,
        MuluEvent,
        MulsuEvent,
        MvihEvent,
        MvvlEvent,
        MvvwEvent,
        OrEvent,
        OriEvent,
        RetEvent,
        SleEvent,
        SleiEvent,
        SleiuEvent,
        SleuEvent,
        SllEvent,
        SlliEvent,
        SltEvent,
        SltiEvent,
        SltuEvent,
        SltiuEvent,
        SraEvent,
        SraiEvent,
        SrlEvent,
        SrliEvent,
        SubEvent,
        TailiEvent,
        TailvEvent,
        XorEvent,
        XoriEvent,
    ]
);

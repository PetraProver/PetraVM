//! The `assembly` crate provides the core components and functionalities for
//! assembling and executing programs with the zCray Virtual Machine (zCrayVM).
//!
//! This includes instruction definitions, program parsing and program
//! execution.

// TODO: Remove these once stable enough
#![allow(unused)]
#![allow(dead_code)]

// TODO: Add doc

pub mod assembler;
pub mod event;
pub mod execution;
pub mod gadgets;
pub mod memory;
pub mod opcodes;
mod parser;
mod util;

pub use assembler::{AssembledProgram, Assembler, AssemblerError};
pub use event::binary_ops::b32::{B32MulEvent, B32MuliEvent};
pub use event::{
    branch::{BnzEvent, BzEvent},
    integer_ops::AddiEvent,
    mv::LDIEvent,
    ret::RetEvent,
};
pub use execution::emulator::{Instruction, InterpreterInstruction};
pub use execution::trace::BoundaryValues;
pub use execution::trace::ZCrayTrace;
pub use memory::{Memory, ProgramRom, ValueRom};
pub use opcodes::Opcode;

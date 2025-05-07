//! The `assembly` crate provides the core components and functionalities for
//! assembling and executing programs with the Petra Virtual Machine (PetraVM).
//!
//! This includes instruction definitions, program parsing and program
//! execution.

// TODO: Add doc

pub mod assembler;
pub mod event;
pub mod execution;
pub mod isa;
pub mod memory;
pub mod opcodes;
mod parser;
mod util;

#[cfg(test)]
mod test_util;

pub use assembler::{AssembledProgram, Assembler, AssemblerError};
pub use event::*;
pub use execution::emulator::{Instruction, InterpreterInstruction};
pub use execution::trace::BoundaryValues;
pub use execution::trace::PetraTrace;
pub use memory::{Memory, ProgramRom, ValueRom};
pub use opcodes::{InstructionInfo, Opcode};
pub use util::init_logger;

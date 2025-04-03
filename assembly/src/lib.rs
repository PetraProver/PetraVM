// TODO: Remove these once stable enough
#![allow(unused)]
#![allow(dead_code)]

// TODO: Add doc

pub mod assembler;
pub mod event;
pub mod execution;
pub mod memory;
pub mod opcodes;
mod parser;
mod util;

pub use assembler::{AssembledProgram, Assembler, AssemblerError};
pub use event::mv::LDIEvent;
pub use event::ret::RetEvent;
pub use execution::emulator::{Instruction, InterpreterInstruction};
pub use execution::trace::BoundaryValues;
pub use execution::trace::ZCrayTrace;
pub use memory::{Memory, ProgramRom, ValueRom};
pub use opcodes::Opcode;

//! Implements the execution engine for the PetraVM.
//!
//! The emulator is responsible for interpreting and running
//! parsed programs and managing the virtual machine state.

pub mod channels;
pub mod emulator;
pub mod trace;

pub use channels::*;
pub use emulator::*;
pub use trace::PetraTrace;

use std::ops::{Deref, DerefMut};

use binius_m3::builder::{B16, B32};

use crate::{
    execution::{FramePointer, Interpreter, InterpreterError},
    memory::{MemoryError, Ram, RamValueT, VromValueT},
    PetraTrace, ValueRom,
};

/// A context sufficient to generate any `Event`, update the state machine and
/// log associated trace operations.
///
/// It contains a mutable reference to the running [`Interpreter`], the
/// [`PetraTrace`], and also contains the PC associated to the event to be
/// generated. It also contains an optional advice, which provides a PROM index
/// and the discrete logarithm in base `B32::MULTIPLICATIVE_GENERATOR` of a
/// group element defined by the instruction arguments, and a boolean indicating
/// whether the current instruction is prover-only. Prover-only instructions are
/// hints for the emulator to help generating the trace, but do not produce any
/// event and do not change the program state.
pub struct EventContext<'a> {
    pub interpreter: &'a mut Interpreter,
    pub trace: &'a mut PetraTrace,
    pub field_pc: B32,
    pub advice: Option<(u32, u32)>,
    pub prover_only: bool,
}

impl EventContext<'_> {
    /// Computes a VROM address from a provided offset, by scaling the frame
    /// pointer accordingly.
    pub fn addr(&self, offset: impl Into<u32>) -> u32 {
        *self.fp ^ offset.into()
    }

    /// Outputs the current program state tuple, containing:
    ///   - the integer program counter PC, as `u32`
    ///   - the field program counter PC, as `B32`
    ///   - the frame pointer FP, as `u32`
    ///   - the timestamp TS, as `u32`
    pub fn program_state(&self) -> (u32, B32, FramePointer, u32) {
        (self.pc, self.field_pc, self.fp, self.timestamp)
    }

    pub fn set_fp<T: Into<FramePointer>>(&mut self, fp: T) {
        self.fp = fp.into();
    }

    pub const fn vrom(&self) -> &ValueRom {
        self.trace.vrom()
    }

    pub fn vrom_mut(&mut self) -> &mut ValueRom {
        self.trace.vrom_mut()
    }

    pub fn vrom_read<T>(&self, addr: u32) -> Result<T, MemoryError>
    where
        T: VromValueT,
    {
        if self.prover_only {
            self.vrom().peek::<T>(addr)
        } else {
            self.vrom().read::<T>(addr)
        }
    }

    pub fn vrom_check_value_set<T>(&self, addr: u32) -> Result<bool, MemoryError>
    where
        T: VromValueT,
    {
        self.vrom().check_value_set::<T>(addr)
    }

    pub fn vrom_write<T>(&mut self, addr: u32, value: T) -> Result<(), MemoryError>
    where
        T: VromValueT,
    {
        self.trace.vrom().check_alignment::<T>(addr)?;

        // In prover-only mode, we don't need to check for deferred moves, nor to record
        // the access.
        let record_write = !self.prover_only;
        self.trace.vrom_write(addr, value, record_write)
    }

    pub const fn ram(&self) -> &Ram {
        self.trace.ram()
    }

    pub fn ram_mut(&mut self) -> &mut Ram {
        self.trace.ram_mut()
    }

    pub fn ram_read<T>(&mut self, addr: u32, timestamp: u32, pc: B32) -> Result<T, MemoryError>
    where
        T: RamValueT,
    {
        self.ram_mut().read(addr, timestamp, pc)
    }

    pub fn ram_write<T>(
        &mut self,
        addr: u32,
        value: T,
        timestamp: u32,
        pc: B32,
    ) -> Result<(), MemoryError>
    where
        T: RamValueT,
    {
        self.ram_mut().write(addr, value, timestamp, pc)
    }

    /// Increments the underlying [`Interpreter`]'s PC.
    pub fn incr_pc(&mut self) {
        self.interpreter.incr_pc();
    }

    /// Inccrements the PROM index and, if not in prover-only mode, increments
    /// the PC.
    pub fn incr_counters(&mut self) {
        self.interpreter.incr_prom_index();
        if !self.prover_only {
            self.interpreter.incr_pc();
        }
    }

    /// Increments the underlying [`Interpreter`]'s PROM index.
    pub fn incr_prom_index(&mut self) {
        self.interpreter.incr_prom_index();
    }

    /// Helper method to update the [`FramePointer`]. It assumes that the next
    /// frame has already been allocated.
    ///
    /// Returns the updated `fp`.
    pub fn setup_call_frame(&mut self, next_fp_offset: B16) -> Result<u32, InterpreterError> {
        // Address where the value of the next frame pointer is stored.
        let next_fp_addr = self.addr(next_fp_offset.val());

        // We assume that the next frame pointer is already set.
        let next_fp_val = self.vrom_read::<u32>(next_fp_addr)?;

        self.set_fp(next_fp_val);
        Ok(next_fp_val)
    }
}

impl Deref for EventContext<'_> {
    type Target = Interpreter;

    fn deref(&self) -> &Self::Target {
        self.interpreter
    }
}

impl DerefMut for EventContext<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.interpreter
    }
}

// Additional helper methods used for testing.
#[cfg(test)]
impl<'a> EventContext<'a> {
    /// Constructor.
    pub(crate) fn new(interpreter: &'a mut Interpreter, trace: &'a mut PetraTrace) -> Self {
        use binius_field::Field;

        Self {
            interpreter,
            trace,
            field_pc: B32::ONE,
            advice: None,
            prover_only: false,
        }
    }

    /// Helper method to set a value in VROM.
    pub fn set_vrom(&mut self, slot: u16, value: u32) {
        self.vrom_write(self.addr(slot), value).unwrap();
    }
}

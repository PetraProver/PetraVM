use std::ops::{Deref, DerefMut};

use binius_m3::builder::{B16, B32};

use super::mv::{MVKind, MvihEvent, MvvlEvent, MvvwEvent};
use crate::{
    execution::{FramePointer, Interpreter, InterpreterError},
    memory::{AccessSize, MemoryError, Ram, RamValueT, VromValueT},
    ValueRom, ZCrayTrace,
};

/// A context sufficient to generate any `Event`, update the state machine and
/// log associated trace operations.
///
/// It contains a mutable reference to the running [`Interpreter`], the
/// [`ZCrayTrace`], and also contains the PC associated to the event to be
/// generated.
pub struct EventContext<'a> {
    pub interpreter: &'a mut Interpreter,
    pub trace: &'a mut ZCrayTrace,
    pub field_pc: B32,
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
        self.vrom().read(addr)
    }

    pub fn vrom_check_value_set<T>(&self, addr: u32) -> Result<bool, MemoryError>
    where
        T: VromValueT,
    {
        self.vrom().check_value_set::<T>(addr)
    }

    pub(crate) fn vrom_record_access<T: VromValueT>(&self, addr: u32) {
        self.vrom().record_access::<T>(addr);
    }

    pub fn vrom_write<T>(&mut self, addr: u32, value: T) -> Result<(), MemoryError>
    where
        T: VromValueT,
    {
        self.trace.vrom_write(addr, value)
    }

    // /// Inserts a pending value in VROM to be set later.
    // ///
    // /// Maps a destination address to a `VromUpdate` which contains necessary
    // /// information to create a MOVE event once the value is available.
    // pub(crate) fn insert_vrom_pending(
    //     &mut self,
    //     parent: u32,
    //     pending_value: VromUpdate,
    // ) -> Result<(), MemoryError> {
    //     self.vrom_mut().insert_pending(parent, pending_value)?;

    //     Ok(())
    // }

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

    /// Helper method to allocate a new frame, updates the [`FramePointer`] and
    /// handle pending MOVE events.
    ///
    /// Returns the updated `fp`, post frame allocation.
    pub fn setup_call_frame(
        &mut self,
        next_fp_offset: B16,
        target: B32,
    ) -> Result<u32, InterpreterError> {
        // Allocate a frame for the call and set the value of the next frame pointer.
        let next_fp_val = self.allocate_new_frame(target)?;

        // Address where the value of the next frame pointer is stored.
        let next_fp_addr = self.addr(next_fp_offset.val());

        self.vrom_write::<u32>(next_fp_addr, next_fp_val)?;

        // Once we have the next_fp, we know the destination address for the moves in
        // the call procedures. We can then generate events for some moves and correctly
        // delegate the other moves.
        self.handles_call_moves()?;

        self.set_fp(next_fp_val);
        Ok(next_fp_val)
    }

    /// This method should only be called once the frame pointer has been
    /// allocated. It is used to generate events -- whenever possible --
    /// once the next_fp has been set by the allocator. When it is not yet
    /// possible to generate the MOVE event (because we are dealing with a
    /// return value that has not yet been set), we add the move information to
    /// the trace's `pending_updates`, so that it can be generated later on.
    fn handles_call_moves(&mut self) -> Result<(), InterpreterError> {
        while let Some(mv_info) = self.moves_to_apply.pop() {
            match mv_info.mv_kind {
                MVKind::Mvvw => {
                    let opt_event = MvvwEvent::generate_event_from_info(
                        self,
                        mv_info.pc,
                        mv_info.timestamp,
                        self.fp,
                        mv_info.dst,
                        mv_info.offset,
                        mv_info.src,
                    )?;
                    if let Some(event) = opt_event {
                        self.trace.mvvw.push(event);
                    }
                }
                MVKind::Mvvl => {
                    let opt_event = MvvlEvent::generate_event_from_info(
                        self,
                        mv_info.pc,
                        mv_info.timestamp,
                        self.fp,
                        mv_info.dst,
                        mv_info.offset,
                        mv_info.src,
                    )?;
                    if let Some(event) = opt_event {
                        self.trace.mvvl.push(event);
                    }
                }
                MVKind::Mvih => {
                    let event = MvihEvent::generate_event_from_info(
                        self,
                        mv_info.pc,
                        mv_info.timestamp,
                        self.fp,
                        mv_info.dst,
                        mv_info.offset,
                        mv_info.src,
                    )?;
                    self.trace.mvih.push(event);
                }
            }
        }

        debug_assert!(self.moves_to_apply.is_empty());

        Ok(())
    }

    pub(crate) fn allocate_new_frame(&mut self, target: B32) -> Result<u32, InterpreterError> {
        self.interpreter.allocate_new_frame(self.trace, target)
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
    pub(crate) fn new(interpreter: &'a mut Interpreter, trace: &'a mut ZCrayTrace) -> Self {
        use binius_field::Field;

        Self {
            interpreter,
            trace,
            field_pc: B32::ONE,
        }
    }

    /// Helper method to set a value in VROM.
    pub fn set_vrom(&mut self, slot: u16, value: u32) {
        self.vrom_write(self.addr(slot), value).unwrap();
    }
}

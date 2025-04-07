use std::ops::{Deref, DerefMut};

use binius_m3::builder::{B16, B32};

use super::mv::{MVIHEvent, MVKind, MVVLEvent, MVVWEvent};
use crate::{
    execution::{FramePointer, Interpreter, InterpreterError},
    memory::MemoryError,
    ZCrayTrace,
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

    /// Loads a `u32` value stored in VROM at the provided address.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// that will scale the frame pointer with the provided offset to obtain the
    /// corresponding VROM address.
    pub fn load_vrom_u32(&self, address: u32) -> Result<u32, MemoryError> {
        self.trace.get_vrom_u32(address)
    }

    /// Loads an optional `u32` value stored in VROM at the provided address.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// that will scale the frame pointer with the provided offset to obtain the
    /// corresponding VROM address.
    pub fn load_vrom_opt_u32(&self, address: u32) -> Result<Option<u32>, MemoryError> {
        self.trace.get_vrom_opt_u32(address)
    }

    /// Stores a `u32` value in VROM at the provided address.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// that will scale the frame pointer with the provided offset to obtain the
    /// corresponding VROM address.
    pub fn store_vrom_u32(&mut self, address: u32, value: u32) -> Result<(), MemoryError> {
        self.trace.set_vrom_u32(address, value)
    }

    /// Stores a `u64` value in VROM at the provided address.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// that will scale the frame pointer with the provided offset to obtain the
    /// corresponding VROM address.
    pub fn store_vrom_u64(&mut self, address: u32, value: u64) -> Result<(), MemoryError> {
        self.trace.set_vrom_u64(address, value)
    }

    /// Loads a `u128` value stored in VROM at the provided address.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// that will scale the frame pointer with the provided offset to obtain the
    /// corresponding VROM address.
    pub fn load_vrom_u128(&self, address: u32) -> Result<u128, MemoryError> {
        self.trace.get_vrom_u128(address)
    }

    /// Loads an optional `u128` value stored in VROM at the provided address.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// that will scale the frame pointer with the provided offset to obtain the
    /// corresponding VROM address.
    pub fn load_vrom_opt_u128(&self, address: u32) -> Result<Option<u128>, MemoryError> {
        self.trace.get_vrom_opt_u128(address)
    }

    /// Stores a `u128` value in VROM at the provided address.
    ///
    /// *NOTE*: Do not pass an offset to this function. Call `ctx.addr(offset)`
    /// that will scale the frame pointer with the provided offset to obtain the
    /// corresponding VROM address.
    pub fn store_vrom_u128(&mut self, address: u32, value: u128) -> Result<(), MemoryError> {
        self.trace.set_vrom_u128(address, value)
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

        self.store_vrom_u32(next_fp_addr, next_fp_val)?;

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
                    let opt_event = MVVWEvent::generate_event_from_info(
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
                    let opt_event = MVVLEvent::generate_event_from_info(
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
                    let event = MVIHEvent::generate_event_from_info(
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
        self.trace.set_vrom_u32(self.addr(slot), value).unwrap();
    }
}

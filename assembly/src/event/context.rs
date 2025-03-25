use std::ops::{Deref, DerefMut};

use binius_field::BinaryField32b;

use super::mv::{MVIHEvent, MVKind, MVVLEvent, MVVWEvent};
use crate::{
    execution::{Interpreter, InterpreterError},
    memory::MemoryError,
    ZCrayTrace,
};

pub(crate) struct EventContext<'a> {
    pub interpreter: &'a mut Interpreter,
    pub trace: &'a mut ZCrayTrace,
    pub field_pc: BinaryField32b,
}

impl<'a> EventContext<'a> {
    pub fn new(interpreter: &'a mut Interpreter, trace: &'a mut ZCrayTrace) -> Self {
        use binius_field::Field;

        Self {
            interpreter,
            trace,
            field_pc: BinaryField32b::ONE,
        }
    }

    // TODO: merge with #70 if it goes through
    pub fn addr(&self, offset: impl Into<u32>) -> u32 {
        self.interpreter.fp ^ offset.into()
    }

    pub fn load_vrom_u32(&self, offset: impl Into<u32>) -> Result<u32, MemoryError> {
        self.trace.get_vrom_u32(self.addr(offset))
    }

    pub fn load_vrom_opt_u32(&self, offset: impl Into<u32>) -> Result<Option<u32>, MemoryError> {
        self.trace.get_vrom_opt_u32(self.addr(offset))
    }

    pub fn store_vrom_u32(
        &mut self,
        offset: impl Into<u32>,
        value: u32,
    ) -> Result<(), MemoryError> {
        self.trace.set_vrom_u32(self.addr(offset), value)
    }

    pub fn store_vrom_u64(
        &mut self,
        offset: impl Into<u32>,
        value: u64,
    ) -> Result<(), MemoryError> {
        self.trace.set_vrom_u64(self.addr(offset), value)
    }

    pub fn load_vrom_u128(&self, offset: impl Into<u32>) -> Result<u128, MemoryError> {
        self.trace.get_vrom_u128(self.addr(offset))
    }

    pub fn load_vrom_opt_u128(&self, offset: impl Into<u32>) -> Result<Option<u128>, MemoryError> {
        self.trace.get_vrom_opt_u128(self.addr(offset))
    }

    pub fn store_vrom_u128(
        &mut self,
        offset: impl Into<u32>,
        value: u128,
    ) -> Result<(), MemoryError> {
        self.trace.set_vrom_u128(self.addr(offset), value)
    }

    pub fn incr_pc(&mut self) {
        self.interpreter.incr_pc();
    }

    pub fn next_timestamp(&self) -> u32 {
        self.interpreter.timestamp + 1
    }

    /// This method should only be called once the frame pointer has been
    /// allocated. It is used to generate events -- whenever possible --
    /// once the next_fp has been set by the allocator. When it is not yet
    /// possible to generate the MOVE event (because we are dealing with a
    /// return value that has not yet been set), we add the move information to
    /// the trace's `pending_updates`, so that it can be generated later on.
    pub(crate) fn handles_call_moves(&mut self) -> Result<(), InterpreterError> {
        for mv_info in &self.moves_to_apply.clone() {
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

        self.moves_to_apply = vec![];
        Ok(())
    }

    pub(crate) fn allocate_new_frame(
        &mut self,
        target: BinaryField32b,
    ) -> Result<u32, InterpreterError> {
        self.interpreter.allocate_new_frame(self.trace, target)
    }

    /// Helper to set a value in VROM
    #[cfg(test)]
    pub fn set_vrom(&mut self, slot: u16, value: u32) {
        self.trace.set_vrom_u32(self.addr(slot), value).unwrap();
    }
}

impl<'a> Deref for EventContext<'a> {
    type Target = Interpreter;

    fn deref(&self) -> &Self::Target {
        &self.interpreter
    }
}

impl<'a> DerefMut for EventContext<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.interpreter
    }
}

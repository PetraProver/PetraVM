use std::{array::from_fn, collections::HashMap};

use binius_field::{BinaryField16b, BinaryField32b};

use crate::{emulator::InterpreterError, event::mv::MVEventOutput, opcodes::Opcode, ZCrayTrace};

type ToSetValue = (
    u32, // parent addr
    Opcode,
    BinaryField32b, // field PC
    u32,            // fp
    u32,            // timestamp
    BinaryField16b, // dst
    BinaryField16b, // src
    BinaryField16b, // offset
);

#[derive(Debug, Default)]
pub(crate) struct ValueRom {
    vrom: HashMap<u32, u8>,
    // HashMap used to set values and push MV events during a CALL procedure.
    // When a MV occurs with a value that isn't set within a CALL procedure, we
    // assume it is a return value. Then, we add (addr_next_frame,
    // to_set_value) to `moves_to_set`, where `to_set_value` contains enough information to create
    // a move event later. Whenever an address in the HashMap's keys is finally set, we populate
    // the missing values and remove them from the HashMap.
    to_set: HashMap<u32, ToSetValue>,
}

impl ValueRom {
    pub fn new(vrom: HashMap<u32, u8>) -> Self {
        Self {
            vrom,
            to_set: HashMap::new(),
        }
    }

    pub fn new_from_vec(vals: Vec<u8>) -> Self {
        let mut vrom = Self::default();

        for (i, val) in vals.into_iter().enumerate() {
            vrom.set_u8(i as u32, val);
        }

        vrom
    }

    pub fn new_from_vec_u32(vals: Vec<u32>) -> Self {
        let mut vrom = Self::default();

        for (i, val) in vals.into_iter().enumerate() {
            vrom.set_u32(&mut ZCrayTrace::default(), 4 * i as u32, val);
        }

        vrom
    }

    pub(crate) fn set_u8(&mut self, index: u32, value: u8) -> Result<(), InterpreterError> {
        if let Some(prev_val) = self.vrom.insert(index, value) {
            if prev_val != value {
                return Err(InterpreterError::VromRewrite(index));
            }
        }
        Ok(())
    }

    pub(crate) fn set_u16(&mut self, index: u32, value: u16) -> Result<(), InterpreterError> {
        if index % 2 != 0 {
            return Err(InterpreterError::VromMisaligned(16, index));
        }
        let bytes = value.to_le_bytes();
        for i in 0..2 {
            self.set_u8(index + i, bytes[i as usize])?;
        }
        Ok(())
    }

    pub(crate) fn set_u32(
        &mut self,
        trace: &mut ZCrayTrace,
        index: u32,
        value: u32,
    ) -> Result<(), InterpreterError> {
        if index % 4 != 0 {
            return Err(InterpreterError::VromMisaligned(32, index));
        }
        let bytes = value.to_le_bytes();
        for i in 0..4 {
            self.set_u8(index + i, bytes[i as usize])?;
        }

        if let Some((parent, opcode, field_pc, fp, timestamp, dst, src, offset)) =
            self.to_set.remove(&index)
        {
            self.set_u32(trace, parent, value);
            let event_out = MVEventOutput::new(
                parent,
                opcode,
                field_pc,
                fp,
                timestamp,
                dst,
                src,
                offset,
                value as u128,
            );
            event_out.push_mv_event(trace);
        }
        Ok(())
    }

    pub(crate) fn set_u128(
        &mut self,
        trace: &mut ZCrayTrace,
        index: u32,
        value: u128,
    ) -> Result<(), InterpreterError> {
        if index % 16 != 0 {
            return Err(InterpreterError::VromMisaligned(128, index));
        }
        let bytes = value.to_le_bytes();
        for i in 0..16 {
            self.set_u8(index + i, bytes[i as usize])?;
        }

        if let Some((parent, opcode, field_pc, fp, timestamp, dst, src, offset)) =
            self.to_set.remove(&index)
        {
            self.set_u128(trace, parent, value)?;
            let event_out = MVEventOutput::new(
                parent, opcode, field_pc, fp, timestamp, dst, src, offset, value,
            );
            event_out.push_mv_event(trace);
        }
        Ok(())
    }

    pub(crate) fn get_u8(&self, index: u32) -> Result<u8, InterpreterError> {
        match self.vrom.get(&index) {
            Some(&value) => Ok(value),
            None => Err(InterpreterError::VromMissingValue(index)),
        }
    }

    pub(crate) fn get_u8_call_procedure(&self, index: u32) -> Option<u8> {
        self.vrom.get(&index).copied()
    }

    pub(crate) fn get_u16(&self, index: u32) -> Result<u16, InterpreterError> {
        if index % 2 != 0 {
            return Err(InterpreterError::VromMisaligned(16, index));
        }

        let mut bytes = [0; 2];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = self.get_u8(index + i as u32)?;
        }

        Ok(u16::from_le_bytes(bytes))
    }

    pub(crate) fn get_u32(&self, index: u32) -> Result<u32, InterpreterError> {
        if index % 4 != 0 {
            return Err(InterpreterError::VromMisaligned(32, index));
        }
        let mut bytes = [0; 4];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = self.get_u8(index + i as u32)?;
        }

        Ok(u32::from_le_bytes(bytes))
    }

    pub(crate) fn get_u32_move(&self, index: u32) -> Result<Option<u32>, InterpreterError> {
        if index % 4 != 0 {
            return Err(InterpreterError::VromMisaligned(32, index));
        }
        let opt_bytes = from_fn(|i| self.get_u8_call_procedure(index + i as u32));
        if opt_bytes.iter().any(|v| v.is_none()) {
            Ok(None)
        } else {
            Ok(Some(u32::from_le_bytes(opt_bytes.map(|v| v.unwrap()))))
        }
    }

    pub(crate) fn get_u128(&self, index: u32) -> Result<u128, InterpreterError> {
        if index % 16 != 0 {
            return Err(InterpreterError::VromMisaligned(128, index));
        }
        let mut bytes = [0; 16];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = self.get_u8(index + i as u32)?;
        }

        Ok(u128::from_le_bytes(bytes))
    }

    pub(crate) fn get_u128_move(&self, index: u32) -> Result<Option<u128>, InterpreterError> {
        if index % 16 != 0 {
            return Err(InterpreterError::VromMisaligned(128, index));
        }
        let opt_bytes = from_fn(|i| self.get_u8_call_procedure(index + i as u32));

        if opt_bytes.iter().any(|v| v.is_none()) {
            Ok(None)
        } else {
            Ok(Some(u128::from_le_bytes(opt_bytes.map(|v| v.unwrap()))))
        }
    }

    pub(crate) fn insert_to_set(
        &mut self,
        dst: u32,
        to_set_val: ToSetValue,
    ) -> Result<(), InterpreterError> {
        if self.to_set.insert(dst, to_set_val).is_some() {
            return Err(InterpreterError::VromRewrite(dst));
        };

        Ok(())
    }
}

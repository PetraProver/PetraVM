use std::{array::from_fn, collections::HashMap};

use binius_field::{BinaryField16b, BinaryField32b};

use crate::{
    emulator::InterpreterError,
    event::mv::MVEventOutput,
    opcodes::Opcode,
    vrom_allocator::{self, VromAllocator},
    ZCrayTrace,
};

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
    vrom: Vec<u32>,
    // HashMap used to set values and push MV events during a CALL procedure.
    // When a MV occurs with a value that isn't set within a CALL procedure, we
    // assume it is a return value. Then, we add (addr_next_frame,
    // to_set_value) to `moves_to_set`, where `to_set_value` contains enough information to create
    // a move event later. Whenever an address in the HashMap's keys is finally set, we populate
    // the missing values and remove them from the HashMap.
    to_set: HashMap<u32, ToSetValue>,
    vrom_allocator: VromAllocator,
}

impl ValueRom {
    pub fn new_from_vec(vals: Vec<u32>) -> Self {
        let len = vals.len();
        let mut vrom = ValueRom::default();
        vrom.vrom = vals;
        vrom.vrom_allocator = VromAllocator::default();
        vrom.vrom_allocator.set_pos(len as u32);
        vrom
    }

    pub(crate) fn set_u8(&mut self, index: u32, value: u8) -> Result<(), InterpreterError> {
        // Return error if index is out of bounds
        if index as usize >= self.vrom.len() {
            return Err(InterpreterError::VromMissingValue(index));
        }

        // Check if there's a previous value and if it's different
        // Keep in mind that the prev_value check cannot cover the case where the
        // previous value is 0. The real double wrting check should be done in
        // prover's constraints.
        let prev_val = self.vrom[index as usize];
        if prev_val != 0 && prev_val != value as u32 {
            return Err(InterpreterError::VromRewrite(index));
        }

        // Store u8 as u32 directly
        self.vrom[index as usize] = value as u32;
        Ok(())
    }

    pub(crate) fn set_u16(&mut self, index: u32, value: u16) -> Result<(), InterpreterError> {
        // Return error if index is out of bounds
        if index as usize >= self.vrom.len() {
            return Err(InterpreterError::VromMissingValue(index));
        }

        let prev_val = self.vrom[index as usize];
        if prev_val != 0 && prev_val != value as u32 {
            return Err(InterpreterError::VromRewrite(index));
        }

        // Store u16 as u32 directly
        self.vrom[index as usize] = value as u32;
        Ok(())
    }

    pub(crate) fn set_u32(
        &mut self,
        trace: &mut ZCrayTrace,
        index: u32,
        value: u32,
    ) -> Result<(), InterpreterError> {
        // Return error if index is out of bounds
        if index as usize >= self.vrom.len() {
            return Err(InterpreterError::VromMissingValue(index));
        }

        let prev_val = self.vrom[index as usize];
        if prev_val != 0 && prev_val != value {
            return Err(InterpreterError::VromRewrite(index));
        }

        self.vrom[index as usize] = value;

        if let Some((parent, opcode, field_pc, fp, timestamp, dst, src, offset)) =
            self.to_set.remove(&index)
        {
            self.set_u32(trace, parent, value)?;
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
        if index % 4 != 0 {
            return Err(InterpreterError::VromMisaligned(128, index));
        }

        // For u128, we need to store it across multiple u32 slots (4 slots)
        let bytes = value.to_le_bytes();
        for i in 0..4 {
            let idx = index + i * 4;
            let slice_start = (i * 4) as usize;
            let u32_val = u32::from_le_bytes([
                bytes[slice_start],
                bytes[slice_start + 1],
                bytes[slice_start + 2],
                bytes[slice_start + 3],
            ]);

            // Return error if index is out of bounds
            if idx as usize >= self.vrom.len() {
                return Err(InterpreterError::VromMissingValue(idx));
            }

            let prev_val = self.vrom[idx as usize];
            if prev_val != 0 && prev_val != u32_val {
                return Err(InterpreterError::VromRewrite(idx));
            }

            self.vrom[idx as usize] = u32_val;
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
        if index as usize >= self.vrom.len() {
            return Err(InterpreterError::VromMissingValue(index));
        }

        Ok(self.vrom[index as usize] as u8)
    }

    pub(crate) fn get_u8_call_procedure(&self, index: u32) -> Option<u8> {
        if index as usize >= self.vrom.len() {
            None
        } else {
            Some(self.vrom[index as usize] as u8)
        }
    }

    pub(crate) fn get_u16(&self, index: u32) -> Result<u16, InterpreterError> {
        if index as usize >= self.vrom.len() {
            return Err(InterpreterError::VromMissingValue(index));
        }

        Ok(self.vrom[index as usize] as u16)
    }

    pub(crate) fn get_u32(&self, index: u32) -> Result<u32, InterpreterError> {
        if index as usize >= self.vrom.len() {
            return Err(InterpreterError::VromMissingValue(index));
        }

        Ok(self.vrom[index as usize])
    }

    pub(crate) fn get_u32_move(&self, index: u32) -> Result<Option<u32>, InterpreterError> {
        if index as usize >= self.vrom.len() {
            return Ok(None);
        }

        Ok(Some(self.vrom[index as usize]))
    }

    pub(crate) fn get_u128(&self, index: u32) -> Result<u128, InterpreterError> {
        if index % 4 != 0 {
            return Err(InterpreterError::VromMisaligned(128, index));
        }

        // For u128, we need to read from multiple u32 slots (4 slots)
        let mut result: u128 = 0;
        for i in 0..4 {
            let idx = index + i * 4;
            if idx as usize >= self.vrom.len() {
                return Err(InterpreterError::VromMissingValue(idx));
            }

            let u32_val = self.vrom[idx as usize];
            // Shift the value to its appropriate position and add to result
            result |= (u32_val as u128) << (i * 32);
        }

        Ok(result)
    }

    pub(crate) fn get_u128_move(&self, index: u32) -> Result<Option<u128>, InterpreterError> {
        if index % 4 != 0 {
            return Err(InterpreterError::VromMisaligned(128, index));
        }

        // Check if all required slots are available
        for i in 0..4 {
            let idx = index + i * 4;
            if idx as usize >= self.vrom.len() {
                return Ok(None);
            }
        }

        // Read from multiple u32 slots (4 slots)
        let mut result: u128 = 0;
        for i in 0..4 {
            let idx = index + i * 4;
            let u32_val = self.vrom[idx as usize];
            // Shift the value to its appropriate position and add to result
            result |= (u32_val as u128) << (i * 32);
        }

        Ok(Some(result))
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

    pub(crate) fn allocate_new_frame(&mut self, target: u32) -> u32 {
        self.vrom_allocator.alloc(target)
    }
}

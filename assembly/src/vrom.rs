use std::{array::from_fn, collections::HashMap};

use binius_field::{BinaryField16b, BinaryField32b};

use crate::{
    emulator::InterpreterError, event::mv::MVEventOutput, opcodes::Opcode,
    vrom_allocator::VromAllocator, ZCrayTrace,
};

/// Represents the data needed to create a move event later
type ToSetValue = (
    u32,            // parent addr
    Opcode,         // operation code
    BinaryField32b, // field PC
    u32,            // fp
    u32,            // timestamp
    BinaryField16b, // dst
    BinaryField16b, // src
    BinaryField16b, // offset
);

/// ValueRom represents a memory structure for storing different sized values
#[derive(Debug, Default)]
pub(crate) struct ValueRom {
    /// Storage for values, each slot is a u32
    vrom: Vec<u32>,
    /// HashMap used to set values and push MV events during a CALL procedure.
    /// When a MV occurs with a value that isn't set within a CALL procedure, we
    /// assume it is a return value. Then, we add (addr_next_frame,
    /// to_set_value) to `to_set`, where `to_set_value` contains enough
    /// information to create a move event later. Whenever an address in the
    /// HashMap's keys is finally set, we populate the missing values and
    /// remove them from the HashMap.
    to_set: HashMap<u32, ToSetValue>,
    /// Allocator for new frames
    vrom_allocator: VromAllocator,
    /// Initial values to populate new frames
    init_values: Vec<u32>,
}

impl ValueRom {
    /// Create a new ValueRom with initial values
    pub fn new_with_init_values(vals: Vec<u32>) -> Self {
        Self {
            vrom: Vec::new(),
            vrom_allocator: VromAllocator::default(),
            init_values: vals,
            to_set: HashMap::new(),
        }
    }

    /// Create an empty ValueRom
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a u8 value at the specified index
    pub(crate) fn set_u8(&mut self, index: u32, value: u8) -> Result<(), InterpreterError> {
        self.check_bounds(index)?;

        // Check if there's a previous value and if it's different
        // Note: This check cannot detect overwriting a value of 0.
        // The real double-writing check should be done in prover's constraints.
        let prev_val = self.vrom[index as usize];
        if prev_val != 0 && prev_val != u32::from(value) {
            return Err(InterpreterError::VromRewrite(index));
        }

        // Store u8 as u32 directly
        self.vrom[index as usize] = u32::from(value);
        Ok(())
    }

    /// Set a u16 value at the specified index
    pub(crate) fn set_u16(&mut self, index: u32, value: u16) -> Result<(), InterpreterError> {
        self.check_bounds(index)?;

        let prev_val = self.vrom[index as usize];
        if prev_val != 0 && prev_val != u32::from(value) {
            return Err(InterpreterError::VromRewrite(index));
        }

        // Store u16 as u32 directly
        self.vrom[index as usize] = u32::from(value);
        Ok(())
    }

    /// Set a u32 value at the specified index
    pub(crate) fn set_u32(
        &mut self,
        trace: &mut ZCrayTrace,
        index: u32,
        value: u32,
    ) -> Result<(), InterpreterError> {
        self.check_bounds(index)?;

        let prev_val = self.vrom[index as usize];
        if prev_val != 0 && prev_val != value {
            return Err(InterpreterError::VromRewrite(index));
        }

        self.vrom[index as usize] = value;

        // Handle any pending to_set entries for this index
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
                u128::from(value),
            );
            event_out.push_mv_event(trace);
        }
        Ok(())
    }

    /// Set a u128 value at the specified index
    pub(crate) fn set_u128(
        &mut self,
        trace: &mut ZCrayTrace,
        index: u32,
        value: u128,
    ) -> Result<(), InterpreterError> {
        self.check_alignment(index, 4)?;

        // For u128, we need to store it across multiple u32 slots (4 slots)
        for i in 0..4 {
            let idx = index + i; // Store in consecutive slots
            let u32_val = (value >> (i * 32)) as u32; // Extract 32-bit chunk directly

            self.check_bounds(idx)?;

            let prev_val = self.vrom[idx as usize];
            if prev_val != 0 && prev_val != u32_val {
                return Err(InterpreterError::VromRewrite(idx));
            }

            self.vrom[idx as usize] = u32_val;
        }

        // Handle any pending to_set entries for this index
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

    /// Get a u8 value from the specified index
    pub(crate) fn get_u8(&self, index: u32) -> Result<u8, InterpreterError> {
        self.check_bounds(index)?;
        Ok(self.vrom[index as usize] as u8)
    }

    /// Get a u8 value from the specified index, returning None if not available
    pub(crate) fn get_u8_call_procedure(&self, index: u32) -> Option<u8> {
        if index as usize >= self.vrom.len() {
            None
        } else {
            Some(self.vrom[index as usize] as u8)
        }
    }

    /// Get a u16 value from the specified index
    pub(crate) fn get_u16(&self, index: u32) -> Result<u16, InterpreterError> {
        self.check_bounds(index)?;
        Ok(self.vrom[index as usize] as u16)
    }

    /// Get a u32 value from the specified index
    pub(crate) fn get_u32(&self, index: u32) -> Result<u32, InterpreterError> {
        self.check_bounds(index)?;
        Ok(self.vrom[index as usize])
    }

    /// Get a u32 value from the specified index, returning None if not
    /// available
    pub(crate) fn get_u32_move(&self, index: u32) -> Result<Option<u32>, InterpreterError> {
        if index as usize >= self.vrom.len() {
            return Ok(None);
        }
        Ok(Some(self.vrom[index as usize]))
    }

    /// Get a u128 value from the specified index
    pub(crate) fn get_u128(&self, index: u32) -> Result<u128, InterpreterError> {
        self.check_alignment(index, 4)?;

        // For u128, we need to read from multiple u32 slots (4 slots)
        let mut result: u128 = 0;
        for i in 0..4 {
            let idx = index + i; // Read from consecutive slots
            self.check_bounds(idx)?;

            let u32_val = self.vrom[idx as usize];
            // Shift the value to its appropriate position and add to result
            result |= u128::from(u32_val) << (i * 32);
        }

        Ok(result)
    }

    /// Get a u128 value from the specified index, returning None if not
    /// available
    pub(crate) fn get_u128_move(&self, index: u32) -> Result<Option<u128>, InterpreterError> {
        self.check_alignment(index, 4)?;

        // Check if all required slots are available
        for i in 0..4 {
            let idx = index + i; // Check consecutive slots
            if idx as usize >= self.vrom.len() {
                return Ok(None);
            }
        }

        // Read from multiple u32 slots (4 slots)
        let mut result: u128 = 0;
        for i in 0..4 {
            let idx = index + i; // Read from consecutive slots
            let u32_val = self.vrom[idx as usize];
            // Shift the value to its appropriate position and add to result
            result |= u128::from(u32_val) << (i * 32);
        }

        Ok(Some(result))
    }

    /// Insert a value to be set later
    pub(crate) fn insert_to_set(
        &mut self,
        dst: u32,
        to_set_val: ToSetValue,
    ) -> Result<(), InterpreterError> {
        if self.to_set.insert(dst, to_set_val).is_some() {
            return Err(InterpreterError::VromRewrite(dst));
        }
        Ok(())
    }

    /// Allocate a new frame with the given target
    pub(crate) fn allocate_new_frame(&mut self, requested_size: u32) -> u32 {
        let pos = self.vrom_allocator.alloc(requested_size);

        // Resize the vector to accommodate the new frame
        self.vrom.resize((pos + requested_size) as usize, 0);

        // Copy initial values if available
        if !self.init_values.is_empty() {
            assert!(self.init_values.len() <= requested_size as usize);
            for i in 0..self.init_values.len() {
                self.vrom[i] = self.init_values[i];
            }
            self.init_values.clear();
        }

        pos
    }

    /// Check if the index is within bounds
    fn check_bounds(&self, index: u32) -> Result<(), InterpreterError> {
        if index as usize >= self.vrom.len() {
            Err(InterpreterError::VromMissingValue(index))
        } else {
            Ok(())
        }
    }

    /// Check if the index has proper alignment
    fn check_alignment(&self, index: u32, alignment: u32) -> Result<(), InterpreterError> {
        if index % alignment != 0 {
            Err(InterpreterError::VromMisaligned(
                alignment.try_into().unwrap(),
                index,
            ))
        } else {
            Ok(())
        }
    }
}

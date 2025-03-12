use binius_field::{BinaryField128b, BinaryField16b, BinaryField32b};

use super::Event;
use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterError, InterpreterTables},
    event::BinaryOperation,
    fire_non_jump_event, ZCrayTrace, G,
};

/// Event for B128_ADD.
///
/// Performs a 128-bit binary field addition (XOR) between two target addresses.
///
/// Logic:
///   1. FP[dst] = __b128_add(FP[src1], FP[src2])
#[derive(Debug, Clone)]
pub(crate) struct B128AddEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u128,
    src1: u16,
    src1_val: u128,
    src2: u16,
    src2_val: u128,
}

impl B128AddEvent {
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;

        // Calculate addresses
        let dst_addr = fp ^ dst.val() as u32;
        let src1_addr = fp ^ src1.val() as u32;
        let src2_addr = fp ^ src2.val() as u32;

        // Get source values
        let src1_val = interpreter.vrom.get_u128(src1_addr)?;
        let src2_val = interpreter.vrom.get_u128(src2_addr)?;

        // In binary fields, addition is XOR
        let dst_val = src1_val ^ src2_val;

        // Store result
        interpreter.vrom.set_u128(trace, dst_addr, dst_val)?;

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.incr_pc();

        Ok(Self {
            timestamp,
            pc: field_pc,
            fp,
            dst: dst.val(),
            dst_val,
            src1: src1.val(),
            src1_val,
            src2: src2.val(),
            src2_val,
        })
    }
}

impl BinaryOperation for B128AddEvent {
    fn operation(val1: BinaryField128b, val2: BinaryField128b) -> BinaryField128b {
        // In binary fields, addition is XOR
        val1 + val2
    }
}

impl super::LeftOp for B128AddEvent {
    type Left = BinaryField128b;

    fn left(&self) -> BinaryField128b {
        BinaryField128b::new(self.src1_val)
    }
}

impl super::RigthOp for B128AddEvent {
    type Right = BinaryField128b;

    fn right(&self) -> BinaryField128b {
        BinaryField128b::new(self.src2_val)
    }
}

impl super::OutputOp for B128AddEvent {
    type Output = BinaryField128b;

    fn output(&self) -> BinaryField128b {
        BinaryField128b::new(self.dst_val)
    }
}

impl Event for B128AddEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        use super::{LeftOp, OutputOp, RigthOp};

        // Verify that the result is correct (XOR of inputs)
        assert_eq!(self.output(), Self::operation(self.left(), self.right()));

        // Update state channel
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc * G, self.fp, self.timestamp + 1));
    }
}

/// Event for B128_MUL.
///
/// Performs a 128-bit binary field multiplication between two target addresses.
///
/// Logic:
///   1. FP[dst] = __b128_mul(FP[src1], FP[src2])
#[derive(Debug, Clone)]
pub(crate) struct B128MulEvent {
    timestamp: u32,
    pc: BinaryField32b,
    fp: u32,
    dst: u16,
    dst_val: u128,
    src1: u16,
    src1_val: u128,
    src2: u16,
    src2_val: u128,
}

impl B128MulEvent {
    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;

        // Calculate addresses
        let dst_addr = fp ^ dst.val() as u32;
        let src1_addr = fp ^ src1.val() as u32;
        let src2_addr = fp ^ src2.val() as u32;

        // Get source values
        let src1_val = interpreter.vrom.get_u128(src1_addr)?;
        let src2_val = interpreter.vrom.get_u128(src2_addr)?;

        // Binary field multiplication
        let src1_bf = BinaryField128b::new(src1_val);
        let src2_bf = BinaryField128b::new(src2_val);
        let dst_bf = src1_bf * src2_bf;
        let dst_val = dst_bf.val();

        // Store result
        interpreter.vrom.set_u128(trace, dst_addr, dst_val)?;

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.incr_pc();

        Ok(Self {
            timestamp,
            pc: field_pc,
            fp,
            dst: dst.val(),
            dst_val,
            src1: src1.val(),
            src1_val,
            src2: src2.val(),
            src2_val,
        })
    }
}

impl BinaryOperation for B128MulEvent {
    fn operation(val1: BinaryField128b, val2: BinaryField128b) -> BinaryField128b {
        val1 * val2
    }
}

impl super::LeftOp for B128MulEvent {
    type Left = BinaryField128b;

    fn left(&self) -> BinaryField128b {
        BinaryField128b::new(self.src1_val)
    }
}

impl super::RigthOp for B128MulEvent {
    type Right = BinaryField128b;

    fn right(&self) -> BinaryField128b {
        BinaryField128b::new(self.src2_val)
    }
}

impl super::OutputOp for B128MulEvent {
    type Output = BinaryField128b;

    fn output(&self) -> BinaryField128b {
        BinaryField128b::new(self.dst_val)
    }
}

impl Event for B128MulEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        use super::{LeftOp, OutputOp, RigthOp};

        // Verify that the result is correct
        assert_eq!(self.output(), Self::operation(self.left(), self.right()));

        // Update state channel
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc * G, self.fp, self.timestamp + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_b128_add_operation() {
        // Test the basic operation logic directly
        let val1 = 0x1111111122222222u128 | (0x3333333344444444u128 << 64);
        let val2 = 0x5555555566666666u128 | (0x7777777788888888u128 << 64);

        let bf1 = BinaryField128b::new(val1);
        let bf2 = BinaryField128b::new(val2);

        // The operation should be XOR
        let expected = val1 ^ val2;
        let result = B128AddEvent::operation(bf1, bf2);

        assert_eq!(result.val(), expected);
    }

    #[test]
    fn test_b128_mul_operation() {
        // Test the basic operation logic directly
        let val1 = 0x0000000000000002u128;
        let val2 = 0x0000000000000003u128;

        let bf1 = BinaryField128b::new(val1);
        let bf2 = BinaryField128b::new(val2);

        // Test the multiplication operation
        let result = B128MulEvent::operation(bf1, bf2);
        let expected = bf1 * bf2;

        assert_eq!(result, expected);
    }
}

use std::ops::Add;

use binius_field::{BinaryField, BinaryField16b, BinaryField32b};

use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterTables, G},
    event::Event,
    fire_non_jump_event, impl_event_for_binary_operation,
    impl_event_no_interaction_with_state_channel, impl_immediate_binary_operation,
    impl_left_right_output_for_bin_op,
};

use super::BinaryOperation;

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct Add64Event {
    timestamp: u32,
    output: u64,
    input1: u64,
    input2: u64,
    cout: u64,
}

impl Add64Event {
    pub fn new(timestamp: u32, output: u64, input1: u64, input2: u64, cout: u64) -> Self {
        Self {
            timestamp,
            output,
            input1,
            input2,
            cout,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter, input1: u64, input2: u64) -> Self {
        let (output, carry) = input1.overflowing_add(input2);

        let cout = (output ^ input1 ^ input2) >> 1 + (carry as u64) << 63;

        let timestamp = interpreter.timestamp;

        Self {
            timestamp,
            output,
            input1,
            input2,
            cout,
        }
    }
}

impl_event_no_interaction_with_state_channel!(Add64Event);

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct Add32Event {
    timestamp: u32,
    output: u32,
    input1: u32,
    input2: u32,
    cout: u32,
}

impl Add32Event {
    pub fn new(timestamp: u32, output: u32, input1: u32, input2: u32, cout: u32) -> Self {
        Self {
            timestamp,
            output,
            input1,
            input2,
            cout,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        input1: BinaryField32b,
        input2: BinaryField32b,
    ) -> Self {
        let inp1 = input1.val();
        let inp2 = input2.val();
        let (output, carry) = inp1.overflowing_add(inp2);

        let cout = (output ^ inp1 ^ inp2) >> 1 + (carry as u32) << 31;

        let timestamp = interpreter.timestamp;

        Self {
            timestamp,
            output,
            input1: inp1,
            input2: inp2,
            cout,
        }
    }
}

impl_event_no_interaction_with_state_channel!(Add32Event);

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct AddiEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    pub(crate) src_val: u32,
    imm: u16,
}

impl BinaryOperation for AddiEvent {
    fn operation(val: BinaryField32b, imm: BinaryField16b) -> BinaryField32b {
        BinaryField32b::new(val.val() + imm.val() as u32)
    }
}

impl_immediate_binary_operation!(AddiEvent);

impl_event_for_binary_operation!(AddiEvent);

impl AddiEvent {
    pub fn generate_event(
        interpreter: &mut Interpreter,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Self {
        let fp = interpreter.fp;
        let src_val = interpreter.vrom.get_u32(fp ^ src.val() as u32);
        // The following addition is checked thanks to the ADD32 table.
        let dst_val = src_val + imm.val() as u32;
        interpreter.vrom.set_u32(fp ^ dst.val() as u32, dst_val);

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.incr_pc();

        Self {
            pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_val,
            src: src.val(),
            src_val,
            imm: imm.val(),
        }
    }
}

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct AddEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_val: u32,
    src1: u16,
    pub(crate) src1_val: u32,
    src2: u16,
    pub(crate) src2_val: u32,
}

impl AddEvent {
    pub fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_val: u32,
        src1: u16,
        src1_val: u32,
        src2: u16,
        src2_val: u32,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src1,
            src1_val,
            src2,
            src2_val,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
    ) -> Self {
        let fp = interpreter.fp;
        let src1_val = interpreter.vrom.get_u32(fp ^ src1.val() as u32);

        let src2_val = interpreter.vrom.get_u32(fp ^ src2.val() as u32);
        // The following addition is checked thanks to the ADD32 table.
        let dst_val = src1_val + src1_val as u32;
        interpreter.vrom.set_u32(fp ^ dst.val() as u32, dst_val);

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.incr_pc();

        Self {
            pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_val,
            src1: src1.val(),
            src1_val,
            src2: src2.val(),
            src2_val,
        }
    }
}

impl BinaryOperation for AddEvent {
    fn operation(val1: BinaryField32b, val2: BinaryField32b) -> BinaryField32b {
        BinaryField32b::new(val1.val() + val2.val())
    }
}

impl_left_right_output_for_bin_op!(AddEvent);

impl_event_for_binary_operation!(AddEvent);

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct MuliEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    pub(crate) src_val: u32,
    imm: u16,
    // Auxiliary commitments
    pub(crate) aux: [u32; 4],
    // Stores aux[0] + aux[1] << 8.
    pub(crate) sum0: u64,
    // Stores aux[2] + aux[3] << 8.
    // Note: we don't need the third  sum value (equal to sum0 + sum1 <<8) because it is equal to DST_VAL.
    pub(crate) sum1: u64,
}

impl MuliEvent {
    pub fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_val: u32,
        src: u16,
        src_val: u32,
        imm: u16,
        aux: [u32; 4],
        sum0: u64,
        sum1: u64,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            imm,
            aux,
            sum0,
            sum1,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
    ) -> Self {
        let fp = interpreter.fp;
        let src_val = interpreter.vrom.get_u32(fp ^ src.val() as u32);

        let imm_val = imm.val();
        let dst_val = src_val * imm_val as u32;

        interpreter.vrom.set_u32(fp ^ dst.val() as u32, dst_val);

        let (aux, sum0, sum1) = schoolbook_multiplcation_intermediate_sums(src_val, imm_val, dst_val);

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.incr_pc();
        Self {
            pc,
            fp: fp,
            timestamp,
            dst: dst.val(),
            dst_val,
            src: src.val(),
            src_val,
            imm: imm_val,
            aux,
            sum0,
            sum1,
        }
    }
}


/// This function computes the intermediate sums of the schoolbook multiplication algorithm.
fn schoolbook_multiplication_intermediate_sums(src_val: u32, imm_val: u16, dst_val: u32) -> ([u32; 4], u64, u64) {
    let xs = src_val.to_le_bytes();
    let ys = imm_val.to_le_bytes();

    let mut aux = [0; 4];
    /// Compute ys[i]*(xs[0] + xs[1]*2^8 + 2^16*xs[2] + 2^24 xs[3]) in two u32, each
    /// containing the summands that wont't overlap
    for i in 0..2 {
        aux[2 * i] = ys[i] as u32 * xs[0] as u32 + (1 << 16) * ys[i] as u32 * xs[2] as u32;
        aux[2 * i + 1] = ys[i] as u32 * xs[1] as u32 + (1 << 16) * ys[i] as u32 * xs[3] as u32;
    }

    // We call the ADD64 gadget to check these additions.
    // sum0 = ys[0]*xs[0] + 2^8*ys[0]*xs[1] + 2^16*ys[0]*xs[2] + 2^24*ys[0]*xs[3]
    let sum0 = aux[0] as u64 + ((aux[1] as u64) << 8);
    // sum1 = ys[1]*xs[0] + 2^8*ys[1]*xs[1] + 2^16*ys[1]*xs[2] + 2^24*ys[1]*xs[3]
    let sum1 = aux[2] as u64 + ((aux[3] as u64) << 8);

    // sum = ys[0]*xs[0] + 2^8*(ys[0]*xs[1] + ys[1]*xs[0]) + 2^16*(ys[0]*xs[2] + ys[1]*xs[1]) + 2^24*(ys[0]*xs[3] + ys[1]*xs[2]) + 2^32*ys[1]*xs[3].
    assert_eq!((sum0 + (sum1 << 8)) as u32, dst_val);
    (aux, sum0, sum1)
}

impl Event for MuliEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_eq!(self.dst_val, self.src_val * self.imm as u32);
        fire_non_jump_event!(self, channels);
    }
}

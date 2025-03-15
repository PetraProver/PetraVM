use binius_field::{underlier::UnderlierType, BinaryField16b, BinaryField32b};
use num_traits::{ops::overflowing::OverflowingAdd, FromPrimitive, PrimInt};

use super::BinaryOperation;
use crate::{
    event::Event,
    execution::{
        Interpreter, InterpreterChannels, InterpreterError, InterpreterTables, ZCrayTrace,
    },
    fire_non_jump_event, impl_binary_operation, impl_event_for_binary_operation,
    impl_event_no_interaction_with_state_channel, impl_immediate_binary_operation,
};

/// Event for the Add gadgets over the integers.
#[derive(Debug, Clone)]
pub(crate) struct AddGadgetEvent<T: Copy + PrimInt + FromPrimitive + OverflowingAdd> {
    timestamp: u32,
    output: T,
    input1: T,
    input2: T,
    cout: T,
}

impl<T: Copy + PrimInt + FromPrimitive + OverflowingAdd + UnderlierType> AddGadgetEvent<T> {
    pub const fn new(timestamp: u32, output: T, input1: T, input2: T, cout: T) -> Self {
        Self {
            timestamp,
            output,
            input1,
            input2,
            cout,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter, input1: T, input2: T) -> Self {
        let (output, carry) = input1.overflowing_add(&input2);

        // cin's i-th bit stores the carry which was added to the sum's i-th bit.
        let cin = output ^ input1 ^ input2;
        // cout's i-th bit stores the carry for input1[i] + input2[i].
        let cout = (cin >> 1)
            + (T::from(carry as usize).expect("It should be possible to get T from usize.")
                << (T::BITS - 1));

        // Check cout.
        assert!(((input1 ^ cin) & (input2 ^ cin)) ^ cin == cout);

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

pub(crate) type Add32Event = AddGadgetEvent<u32>;
pub(crate) type Add64Event = AddGadgetEvent<u64>;

impl_event_no_interaction_with_state_channel!(Add32Event);
impl_event_no_interaction_with_state_channel!(Add64Event);

/// Event for ADDI.
///
/// Performs an ADD between a target address and an immediate.
///
/// Logic:
///   1. FP[dst] = FP[src] + imm
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
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;
        let src_val = trace.get_vrom_u32(fp ^ src.val() as u32)?;
        // The following addition is checked thanks to the ADD32 table.
        let dst_val = src_val.wrapping_add(imm.val() as u32);
        trace.set_vrom_u32(fp ^ dst.val() as u32, dst_val)?;

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.incr_pc();

        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_val,
            src: src.val(),
            src_val,
            imm: imm.val(),
        })
    }
}

/// Event for ADD.
///
/// Performs an ADD between two target addresses.
///
/// Logic:
///   1. FP[dst] = FP[src1] + FP[src2]
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

impl BinaryOperation for AddEvent {
    fn operation(val1: BinaryField32b, val2: BinaryField32b) -> BinaryField32b {
        BinaryField32b::new(val1.val().wrapping_add(val2.val()))
    }
}

// Note: The addition is checked thanks to the ADD32 table.
impl_binary_operation!(AddEvent);
impl_event_for_binary_operation!(AddEvent);

/// Event for MULI.
///
/// Performs a MUL between a signed 32-bit integer and a 16-bit immediate.
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
    // Note: we don't need the third  sum value (equal to sum0 + sum1 <<8) because it is equal to
    // DST_VAL.
    pub(crate) sum1: u64,
}

impl MuliEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
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
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src: BinaryField16b,
        imm: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;
        let src_val = trace.get_vrom_u32(fp ^ src.val() as u32)?;

        let imm_val = imm.val();
        let dst_val = src_val * imm_val as u32; // TODO: shouldn't the result be u64, stored over two slots?

        trace.set_vrom_u32(fp ^ dst.val() as u32, dst_val)?;

        let (aux, sum0, sum1) =
            schoolbook_multiplication_intermediate_sums(src_val, imm_val, dst_val);

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.incr_pc();
        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_val,
            src: src.val(),
            src_val,
            imm: imm_val,
            aux,
            sum0,
            sum1,
        })
    }
}

/// This function computes the intermediate sums of the schoolbook
/// multiplication algorithm.
fn schoolbook_multiplication_intermediate_sums(
    src_val: u32,
    imm_val: u16,
    dst_val: u32,
) -> ([u32; 4], u64, u64) {
    let xs = src_val.to_le_bytes();
    let ys = imm_val.to_le_bytes();

    let mut aux = [0; 4];
    // Compute ys[i]*(xs[0] + xs[1]*2^8 + 2^16*xs[2] + 2^24 xs[3]) in two u32, each
    // containing the summands that wont't overlap
    for i in 0..2 {
        aux[2 * i] = ys[i] as u32 * xs[0] as u32 + (1 << 16) * ys[i] as u32 * xs[2] as u32;
        aux[2 * i + 1] = ys[i] as u32 * xs[1] as u32 + (1 << 16) * ys[i] as u32 * xs[3] as u32;
    }

    // We call the ADD64 gadget to check these additions.
    // sum0 = ys[0]*xs[0] + 2^8*ys[0]*xs[1] + 2^16*ys[0]*xs[2] + 2^24*ys[0]*xs[3]
    let sum0 = aux[0] as u64 + ((aux[1] as u64) << 8);
    // sum1 = ys[1]*xs[0] + 2^8*ys[1]*xs[1] + 2^16*ys[1]*xs[2] + 2^24*ys[1]*xs[3]
    let sum1 = aux[2] as u64 + ((aux[3] as u64) << 8);

    // sum = ys[0]*xs[0] + 2^8*(ys[0]*xs[1] + ys[1]*xs[0]) + 2^16*(ys[0]*xs[2] +
    // ys[1]*xs[1]) + 2^24*(ys[0]*xs[3] + ys[1]*xs[2]) + 2^32*ys[1]*xs[3].
    assert_eq!((sum0 + (sum1 << 8)) as u32, dst_val);
    (aux, sum0, sum1)
}

impl Event for MuliEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_eq!(self.dst_val, self.src_val * self.imm as u32);
        fire_non_jump_event!(self, channels);
    }
}

use std::ops::Add;

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

        let (aux, sums, _) =
            schoolbook_multiplication_intermediate_sums::<u16>(src_val, imm_val, dst_val);

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
            aux: aux.try_into().unwrap(),
            sum0: sums[0],
            sum1: sums[1],
        })
    }
}

impl Event for MuliEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_eq!(self.dst_val, self.src_val * self.imm as u32);
        fire_non_jump_event!(self, channels);
    }
}

/// Event for MUL.
///
/// Performs a MUL between two signed 32-bit integers.
#[derive(Debug, Clone)]
pub(crate) struct MulEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_val: u32,
    src1: u16,
    pub(crate) src1_val: u32,
    src2: u16,
    src2_val: u32,
    // Auxiliary commitments
    pub(crate) aux: [u32; 8],
    // Stores all aux[2i] + aux[2i + 1] << 8.
    pub(crate) aux_sums: [u64; 4],
    // Stores the cumulative sums: cum_sum[i] = cum_sum[i-1] + aux_sum[i] << 8*i
    pub(crate) cum_sums: [u64; 2],
}

// TODO: add support for signed values.
impl MulEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_val: u32,
        src1: u16,
        src1_val: u32,
        src2: u16,
        src2_val: u32,
        aux: [u32; 8],
        aux_sums: [u64; 4],
        cum_sums: [u64; 2],
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
            aux,
            aux_sums,
            cum_sums,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;
        let src1_val = trace.get_vrom_u32(fp ^ src1.val() as u32)?;
        let src2_val = trace.get_vrom_u32(fp ^ src2.val() as u32)?;

        let dst_val = src1_val * src2_val; // TODO: shouldn't the result be u64, stored over two slots?

        trace.set_vrom_u32(fp ^ dst.val() as u32, dst_val)?;

        let (aux, aux_sums, cum_sums) =
            schoolbook_multiplication_intermediate_sums::<u32>(src1_val, src2_val, dst_val);

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.incr_pc();
        Ok(Self {
            pc: field_pc,
            fp,
            timestamp,
            dst: dst.val(),
            dst_val,
            src1: src1.val(),
            src1_val,
            src2: src1.val(),
            src2_val,
            aux: aux.try_into().expect("Created an incorrect aux vector."),
            aux_sums: aux_sums
                .try_into()
                .expect("Created an incorrect aux_sums vector."),
            cum_sums: cum_sums
                .try_into()
                .expect("Created an incorrect cum_sums vector."),
        })
    }
}

impl Event for MulEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_eq!(self.dst_val, self.src1_val * self.src2_val);
        fire_non_jump_event!(self, channels);
    }
}

/// This function computes the intermediate sums of the schoolbook
/// multiplication algorithm.
fn schoolbook_multiplication_intermediate_sums<T: Into<u32>>(
    src_val: u32,
    imm_val: T,
    dst_val: u32,
) -> (Vec<u32>, Vec<u64>, Vec<u64>) {
    let xs = src_val.to_le_bytes();
    let num_ys_bytes = std::mem::size_of::<T>();
    let ys = &imm_val.into().to_le_bytes()[..num_ys_bytes];

    let num_aux = num_ys_bytes * 2;
    let mut aux = vec![0; num_ys_bytes * 2];
    // Compute ys[i]*(xs[0] + xs[1]*2^8 + 2^16*xs[2] + 2^24 xs[3]) in two u32, each
    // containing the summands that wont't overlap
    for i in 0..num_ys_bytes {
        aux[2 * i] = ys[i] as u32 * xs[0] as u32 + ((ys[i] as u32 * xs[2] as u32) << 16);
        aux[2 * i + 1] = ys[i] as u32 * xs[1] as u32 + ((ys[i] as u32 * xs[3] as u32) << 16);
    }

    // We call the ADD64 gadget to check these additions.
    // sum[i] = aux[2*i] + aux[2*i+1]
    //        = ys[i]*xs[0] + 2^8*ys[i]*xs[1] + 2^16*ys[i]*xs[2] + 2^24*ys[i]*xs[3]
    let aux_sums: Vec<u64> = (0..num_ys_bytes)
        .map(|i| aux[2 * i] as u64 + ((aux[2 * i + 1] as u64) << 8))
        .collect();

    // We call the ADD64 gadget to check these additions. These compute the
    // cumulative sums of all auxiliary sums. Indeed, the final output corresponds
    // to the sum of all auxiliary sums.
    //
    // Note that we only need to store l-2 values because the last cumulative sum is
    // actually equal to the output. Moreover, the thirst cumulative sum is
    // simply `aux_sums[0]`. If `l` is the number of bytes in `T`, then:
    // - cum_sums[0] = aux_sums[0] + aux_sums[1] << 8
    // - output = cum_sums[l-3] + aux_sums[l-1] << 8*l
    // - cum_sums[i] = cum_sums[i-1] + aux_sum[i] << 8*(i+1)
    let cum_sums = if num_ys_bytes > 2 {
        let mut cum_sums = vec![0; num_ys_bytes - 2];

        cum_sums[0] = aux_sums[0] + (aux_sums[1] << 8);
        (1..num_ys_bytes - 2)
            .map(|i| cum_sums[i] = cum_sums[i - 1] + (aux_sums[i + 1] << (8 * (i + 1))))
            .collect::<Vec<_>>();
        cum_sums
    } else {
        vec![]
    };

    if !cum_sums.is_empty() {
        assert_eq!(
            (cum_sums[num_ys_bytes - 3] + (aux_sums[num_ys_bytes - 1] << (8 * (num_ys_bytes - 1))))
                as u32,
            dst_val,
            "Incorrect cum_sums."
        );
    } else {
        assert_eq!(
            (aux_sums[0] + (aux_sums[1] << 8)) as u32,
            dst_val,
            "Incorrect aux_sums."
        );
    }

    (aux, aux_sums, cum_sums)
}

/// Event for SLTU.
///
/// Performs an SLTU between two target addresses.
///
/// Logic:
///   1. FP[dst] = FP[src1] < FP[src2]
#[derive(Debug, Clone)]
pub(crate) struct SltuEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_val: u32,
    src1: u16,
    src1_val: u32,
    src2: u16,
    src2_val: u32,
}

impl BinaryOperation for SltuEvent {
    fn operation(val1: BinaryField32b, val2: BinaryField32b) -> BinaryField32b {
        // LT is checked using a SUB gadget.
        BinaryField32b::new((val1.val() < val2.val()) as u32)
    }
}

// Note: The addition is checked thanks to the ADD32 table.
impl_binary_operation!(SltuEvent);
impl_event_for_binary_operation!(SltuEvent);

/// Event for SLTIU.
///
/// Performs an SLTIU between two target addresses.
///
/// Logic:
///   1. FP[dst] = FP[src1] < FP[src2]
#[derive(Debug, Clone)]
pub(crate) struct SltiuEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u16,
}

impl BinaryOperation for SltiuEvent {
    fn operation(val1: BinaryField32b, val2: BinaryField16b) -> BinaryField32b {
        // LT is checked using a SUB gadget.
        BinaryField32b::new((val1.val() < val2.val() as u32) as u32)
    }
}

impl_immediate_binary_operation!(SltiuEvent);
impl_event_for_binary_operation!(SltiuEvent);

// Event for SUB.
///
/// Performs a SUB between two target addresses.
///
/// Logic:
///   1. FP[dst] = FP[src1] - FP[src2]
#[derive(Debug, Clone)]
pub(crate) struct SubEvent {
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

// TODO: add support for signed values.
impl BinaryOperation for SubEvent {
    fn operation(val1: BinaryField32b, val2: BinaryField32b) -> BinaryField32b {
        // SUB is checked using a specific gadget, similarly to ADD.
        BinaryField32b::new(val1.val().wrapping_sub(val2.val()))
    }
}

// Note: The addition is checked thanks to the ADD32 table.
impl_binary_operation!(SubEvent);
impl_event_for_binary_operation!(SubEvent);

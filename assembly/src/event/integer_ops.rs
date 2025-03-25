use std::ops::Add;

use binius_field::{underlier::UnderlierType, BinaryField16b, BinaryField32b};
use num_traits::{ops::overflowing::OverflowingAdd, FromPrimitive, PrimInt};

use crate::{
    define_bin32_imm_op_event, define_bin32_op_event,
    event::{binary_ops::BinaryOperation, Event},
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

define_bin32_imm_op_event!(
    /// Event for ADDI.
    ///
    /// Performs an ADD between a target address and an immediate.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src] + imm
    AddiEvent,
    |a: BinaryField32b, imm: BinaryField16b| BinaryField32b::new((a.val() as i32).wrapping_add(imm.val() as i16 as i32) as u32)
);

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
        let dst_val = AddiEvent::operation(src_val.into(), imm).val();
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

// Note: The addition is checked thanks to the ADD32 table.
define_bin32_op_event!(
    /// Event for ADD.
    ///
    /// Performs an ADD between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] + FP[src2]
    AddEvent,
    |a: BinaryField32b, b: BinaryField32b| BinaryField32b::new((a.val() as i32).wrapping_add(b.val() as i32) as u32)
);

/// Event for MULI.
///
/// Performs a MUL between a signed 32-bit integer and a 16-bit immediate.
#[derive(Debug, Clone)]
pub(crate) struct MuliEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_val: u64,
    src: u16,
    pub(crate) src_val: u32,
    imm: u16,
}

impl MuliEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_val: u64,
        src: u16,
        src_val: u32,
        imm: u16,
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
        let dst_val = (src_val as i32 as i64).wrapping_mul(imm_val as i16 as i64) as u64; // TODO: shouldn't the result be u64, stored over two slots?

        trace.set_vrom_u64(fp ^ dst.val() as u32, dst_val)?;

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
        })
    }
}

impl Event for MuliEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_eq!(
            self.dst_val,
            (self.src_val as i32 as i64).wrapping_mul(self.imm as i16 as i64) as u64
        );
        fire_non_jump_event!(self, channels);
    }
}

/// Event for MULU.
///
/// Performs a MULU between two unsigned 32-bit integers. Returns a 64-bit
/// result.
#[derive(Debug, Clone)]
pub(crate) struct MuluEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_val: u64,
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

impl MuluEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_val: u64,
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

        let dst_val = (src1_val as u64).wrapping_mul(src2_val as u64); // TODO: shouldn't the result be u64, stored over two slots?

        trace.set_vrom_u64(fp ^ dst.val() as u32, dst_val)?;

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
            src2: src2.val(),
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

impl Event for MuluEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_eq!(
            self.dst_val,
            (self.src1_val as u64).wrapping_mul(self.src2_val as u64)
        );
        fire_non_jump_event!(self, channels);
    }
}

/// This function computes the intermediate sums of the schoolbook
/// multiplication algorithm.
fn schoolbook_multiplication_intermediate_sums<T: Into<u32>>(
    src_val: u32,
    imm_val: T,
    dst_val: u64,
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
            cum_sums[num_ys_bytes - 3] + (aux_sums[num_ys_bytes - 1] << (8 * (num_ys_bytes - 1))),
            dst_val,
            "Incorrect cum_sums."
        );
    } else {
        assert_eq!(
            aux_sums[0] + (aux_sums[1] << 8),
            dst_val,
            "Incorrect aux_sums."
        );
    }

    (aux, aux_sums, cum_sums)
}

#[derive(Debug, Clone)]
pub(crate) enum SignedMulKind {
    Mulsu,
    Mul,
}
/// Event for MUL or MULSU.
///
/// Performs a MUL between two signed 32-bit integers.
#[derive(Debug, Clone)]
pub(crate) struct SignedMulEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_val: u64,
    src1: u16,
    pub(crate) src1_val: u32,
    src2: u16,
    src2_val: u32,
    kind: SignedMulKind,
}

impl SignedMulEvent {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        pc: BinaryField32b,
        fp: u32,
        timestamp: u32,
        dst: u16,
        dst_val: u64,
        src1: u16,
        src1_val: u32,
        src2: u16,
        src2_val: u32,
        kind: SignedMulKind,
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
            kind,
        }
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        trace: &mut ZCrayTrace,
        dst: BinaryField16b,
        src1: BinaryField16b,
        src2: BinaryField16b,
        field_pc: BinaryField32b,
        kind: SignedMulKind,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;
        let src1_val = trace.get_vrom_u32(fp ^ src1.val() as u32)?;
        let src2_val = trace.get_vrom_u32(fp ^ src2.val() as u32)?;

        let dst_val = Self::multiplication(src1_val, src2_val, &kind);

        trace.set_vrom_u64(fp ^ dst.val() as u32, dst_val)?;

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
            src2: src2.val(),
            src2_val,
            kind,
        })
    }

    pub fn multiplication(input1: u32, input2: u32, kind: &SignedMulKind) -> u64 {
        match kind {
            // If the value is signed, first turn into an i32 to get the sign, then into an i64 to
            // get the 64-bit value. Otherwise, directly cast as an i64 for the multiplication.
            SignedMulKind::Mul => (input1 as i32 as i64).wrapping_mul(input2 as i32 as i64) as u64,
            SignedMulKind::Mulsu => (input1 as i32 as i64).wrapping_mul(input2 as i64) as u64,
        }
    }
}

impl Event for SignedMulEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        assert_eq!(
            self.dst_val,
            SignedMulEvent::multiplication(self.src1_val, self.src2_val, &self.kind)
        );
        fire_non_jump_event!(self, channels);
    }
}

// Note: The addition is checked thanks to the ADD32 table.
define_bin32_op_event!(
    /// Event for SLTU.
    ///
    /// Performs an SLTU between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] < FP[src2]
    SltuEvent,
    // LT is checked using a SUB gadget.
    |a: BinaryField32b, b: BinaryField32b| BinaryField32b::new((a.val() < b.val()) as u32)
);

// Note: The addition is checked thanks to the ADD32 table.
define_bin32_op_event!(
    /// Event for SLT.
    ///
    /// Performs an SLT between two signed target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] < FP[src2]
    SltEvent,
    // LT is checked using a SUB gadget.
    |a: BinaryField32b, b: BinaryField32b| BinaryField32b::new(((a.val() as i32) < (b.val() as i32)) as u32)
);

define_bin32_imm_op_event!(
    /// Event for SLTIU.
    ///
    /// Performs an SLTIU between an unsigned target address and immediate.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] < FP[src2]
    SltiuEvent,
    // LT is checked using a SUB gadget.
    |a: BinaryField32b, imm: BinaryField16b| BinaryField32b::new((a.val() < imm.val() as u32) as u32)
);

define_bin32_imm_op_event!(
    /// Event for SLTI.
    ///
    /// Performs an SLTI between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] < FP[src2]
    SltiEvent,
    // LT is checked using a SUB gadget.
    |a: BinaryField32b, imm: BinaryField16b| BinaryField32b::new(((a.val() as i32) < (imm.val() as i16 as i32)) as u32)
);

define_bin32_op_event!(
    // Event for SUB.
    ///
    /// Performs a SUB between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] - FP[src2]
    SubEvent,
    // SUB is checked using a specific gadget, similarly to ADD.
    // TODO: add support for signed values.
    |a: BinaryField32b, b: BinaryField32b| BinaryField32b::new(((a.val() as i32).wrapping_sub(b.val() as i32)) as u32)
);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use binius_field::{BinaryField16b, BinaryField32b, Field};

    use crate::{
        event::{
            binary_ops::{ImmediateBinaryOperation, NonImmediateBinaryOperation},
            integer_ops::{
                AddEvent, AddiEvent, MuliEvent, MuluEvent, SignedMulEvent, SignedMulKind, SltEvent,
                SltiEvent, SltiuEvent, SltuEvent, SubEvent,
            },
            test_utils::TestEnv,
        },
        execution::{Interpreter, ZCrayTrace},
    };

    /// Tests for Add operations (without immediate)
    #[test]
    fn test_add_operations() {
        // Test cases for ADD
        let test_cases = [
            // (src1_val, src2_val, expected_result, description)
            (10, 20, 30, "simple addition"),
            (0, 0, 0, "zero addition"),
            (u32::MAX, 1, 0, "overflow"),
            (0x7FFFFFFF, 1, 0x80000000, "positive to negative overflow"),
            (
                0x80000000,
                0xFFFFFFFF,
                0x7FFFFFFF,
                "negative to positive underflow",
            ),
            (0x1000, 0x7FFF, 0x8FFF, "add values"),
            (
                0x1000,
                0xFFFF8000,
                0xFFFF9000,
                "add sign-extended negative value",
            ),
            (0x1000, 0xFFFFFFFF, 0xFFF, "add -1 value"),
        ];

        for (src1_val, src2_val, expected, desc) in test_cases {
            let mut env = TestEnv::new();
            let src1_offset = BinaryField16b::new(2);
            let src2_offset = BinaryField16b::new(3);
            let dst_offset = BinaryField16b::new(4);

            // Set values in VROM at the computed addresses (FP ^ offset)
            env.set_vrom(src1_offset.val(), src1_val);
            env.set_vrom(src2_offset.val(), src2_val);

            let event = AddEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst_offset,
                src1_offset,
                src2_offset,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, expected,
                "ADD failed for {}: expected 0x{:x} got 0x{:x} (src1=0x{:x}, src2=0x{:x})",
                desc, expected, event.dst_val, src1_val, src2_val
            );
        }
    }

    /// Tests for Sub operations
    #[test]
    fn test_sub_operations() {
        // Test cases for SUB
        let test_cases = [
            // (src1_val, src2_val, expected_result, description)
            (30, 20, 10, "simple subtraction"),
            (
                20,
                30,
                0xFFFFFFF6,
                "negative result (-10 in two's complement)",
            ),
            (0, 0, 0, "zero subtraction"),
            (0, 1, 0xFFFFFFFF, "0 - 1 = -1 (underflow to max u32)"),
            (
                0x80000000,
                1,
                0x7FFFFFFF,
                "MIN_INT - 1 = MAX_INT (underflow)",
            ),
            (
                0x7FFFFFFF,
                0xFFFFFFFF,
                0x80000000,
                "MAX_INT - (-1) = MIN_INT (overflow)",
            ),
            (0x7FFFFFFF, 0x7FFFFFFF, 0, "MAX_INT - MAX_INT = 0"),
            (0x80000000, 0x80000000, 0, "MIN_INT - MIN_INT = 0"),
            (
                0,
                0x80000000,
                0x80000000,
                "0 - MIN_INT = MIN_INT (positive overflow)",
            ),
            (0xFFFFFFFF, 0x7FFFFFFF, 0x80000000, "-1 - MAX_INT = MIN_INT"),
            (0x80000000, 0x7FFFFFFF, 0x00000001, "MIN_INT - MAX_INT = 1"),
            (0xFFFFFFFF, 0xFFFFFFFF, 0, "-1 - (-1) = 0"),
            (0x12345678, 0x12345678, 0, "arbitrary value - itself = 0"),
        ];

        for (src1_val, src2_val, expected, desc) in test_cases {
            let mut env = TestEnv::new();
            let src1_offset = BinaryField16b::new(2);
            let src2_offset = BinaryField16b::new(3);
            let dst_offset = BinaryField16b::new(4);

            // Set values in VROM at the computed addresses (FP ^ offset)
            env.set_vrom(src1_offset.val(), src1_val);
            env.set_vrom(src2_offset.val(), src2_val);

            let event = SubEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst_offset,
                src1_offset,
                src2_offset,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, expected,
                "SUB failed for {}: expected 0x{:x} got 0x{:x} (src1=0x{:x}, src2=0x{:x})",
                desc, expected, event.dst_val, src1_val, src2_val
            );
        }
    }

    /// Tests for Addi operations
    #[test]
    fn test_addi_operations() {
        // Test cases for ADDI with sign extension
        let test_cases = [
            // (src_val, imm_val, expected_result, description)
            (10, 20, 30, "simple addition"),
            (0, 0, 0, "zero addition"),
            (u32::MAX, 1, 0, "overflow"),
            (0x7FFFFFFF, 1, 0x80000000, "positive to negative overflow"),
            (0x1000, 0x7FFF, 0x8FFF, "add max positive immediate"),
            // Sign extension tests
            (
                0x1000,
                0x8000,
                0xFFFF9000,
                "add min negative immediate (0x8000 -> -32768)",
            ),
            (0x1000, 0xFFFF, 0xFFF, "add -1 immediate (0xFFFF -> -1)"),
            (0xFFFFFFFF, 0xFFFF, 0xFFFFFFFE, "add -1 to -1 (wrapped)"),
            (
                0x80000000,
                0x8000,
                0x7FFF8000,
                "add min negative immediate to min int (overflow)",
            ),
        ];

        for (src_val, imm_val, expected, desc) in test_cases {
            let mut env = TestEnv::new();
            let src_offset = BinaryField16b::new(2);
            let dst_offset = BinaryField16b::new(4);

            // Set value in VROM at the computed address (FP ^ offset)
            env.set_vrom(src_offset.val(), src_val);
            let imm = BinaryField16b::new(imm_val);

            let event = AddiEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst_offset,
                src_offset,
                imm,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, expected,
                "ADDI failed for {}: expected 0x{:x} got 0x{:x} (src=0x{:x}, imm=0x{:x})",
                desc, expected, event.dst_val, src_val, imm_val
            );
        }
    }

    /// Tests for Mul operations (without immediate)
    #[test]
    fn test_mul_operations() {
        // Test cases for MUL, MULU, MULSU
        let test_cases = [
            // (src1_val, src2_val, mul_expected, mulu_expected, mulsu_expected, description)
            (5, 7, 35, 35, 35, "simple multiplication"),
            (0, 0, 0, 0, 0, "zero multiplication"),
            (1, 0, 0, 0, 0, "identity with zero"),
            (0, 1, 0, 0, 0, "zero with identity"),
            (1, 1, 1, 1, 1, "identity"),
            // Negative source values - different behavior between signed and unsigned
            (
                0xFFFFFFFF,
                2,
                (-2i64 as u64),
                0x1FFFFFFFE,
                (-2i64 as u64),
                "negative * positive",
            ),
            (
                0xFFFFFFFF,
                0xFFFFFFFF,
                1,
                0xFFFFFFFE00000001,
                0xFFFFFFFF00000001,
                "negative * negative",
            ),
            (
                0xFFFFFFFF,
                0x80000000,
                0x80000000,
                0x7fffffff80000000,
                0xffffffff80000000,
                "negative * min negative",
            ),
            // Large value edge cases
            (
                0x7FFFFFFF,
                2,
                0xFFFFFFFE,
                0xFFFFFFFE,
                0xFFFFFFFE,
                "max positive * 2",
            ),
            (
                0x7FFFFFFF,
                0x7FFFFFFF,
                0x3FFFFFFF00000001,
                0x3FFFFFFF00000001,
                0x3FFFFFFF00000001,
                "max positive * max positive",
            ),
            (
                0x80000000,
                2,
                0xffffffff00000000,
                0x100000000,
                0xffffffff00000000,
                "min negative * 2",
            ),
            (
                0x80000000,
                0x80000000,
                0x4000000000000000,
                0x4000000000000000,
                0xc000000000000000,
                "min negative * min negative",
            ),
        ];

        for (src1_val, src2_val, mul_expected, mulu_expected, mulsu_expected, desc) in test_cases {
            // Test MUL (sign * sign)
            let mut env = TestEnv::new();
            let src1_offset = BinaryField16b::new(2);
            let src2_offset = BinaryField16b::new(3);
            let dst_offset = BinaryField16b::new(4);

            // Set values in VROM at the computed addresses (FP ^ offset)
            env.set_vrom(src1_offset.val(), src1_val);
            env.set_vrom(src2_offset.val(), src2_val);

            let event = SignedMulEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst_offset,
                src1_offset,
                src2_offset,
                env.field_pc,
                SignedMulKind::Mul,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, mul_expected,
                "MUL failed for {}: expected 0x{:x} got 0x{:x} (src1=0x{:x}, src2=0x{:x})",
                desc, mul_expected, event.dst_val, src1_val, src2_val
            );

            // Test MULU (unsigned * unsigned)
            let mut env = TestEnv::new();
            env.set_vrom(src1_offset.val(), src1_val);
            env.set_vrom(src2_offset.val(), src2_val);

            let event = MuluEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst_offset,
                src1_offset,
                src2_offset,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, mulu_expected,
                "MULU failed for {}: expected 0x{:x} got 0x{:x} (src1=0x{:x}, src2=0x{:x})",
                desc, mulu_expected, event.dst_val, src1_val, src2_val
            );

            // Test MULSU (sign * unsigned)
            let mut env = TestEnv::new();
            env.set_vrom(src1_offset.val(), src1_val);
            env.set_vrom(src2_offset.val(), src2_val);

            let event = SignedMulEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst_offset,
                src1_offset,
                src2_offset,
                env.field_pc,
                SignedMulKind::Mulsu,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, mulsu_expected,
                "MULSU failed for {}: expected 0x{:x} got 0x{:x} (src1=0x{:x}, src2=0x{:x})",
                desc, mulsu_expected, event.dst_val, src1_val, src2_val
            );
        }
    }

    /// Tests for Mul operations (with immediate)
    #[test]
    fn test_muli_operations() {
        // Test cases for MULI (sign-extends the immediate)
        let test_cases = [
            // (src_val, imm_val, expected_result, description)
            (5, 7, 35, "simple multiplication"),
            (0, 0, 0, "zero multiplication"),
            (1, 0, 0, "identity with zero"),
            (0, 1, 0, "zero with identity"),
            (1, 1, 1, "identity"),
            // Immediate sign extension tests
            (5, 0x7FFF, 163835, "multiply by max positive 16-bit"),
            (
                5,
                0x8000,
                0xfffffffffffd8000,
                "multiply by 0x8000 (sign extends to -32768)",
            ),
            (
                5,
                0xFFFF,
                (-5i64 as u64),
                "multiply by 0xFFFF (sign extends to -1)",
            ),
            // Tests with negative source values
            (0xFFFFFFFF, 2, (-2i64 as u64), "negative * positive"),
            (0x80000000, 2, 0xffffffff00000000, "min negative * 2"),
            // Edge cases with 16-bit immediate
            (
                0x10000,
                0x7FFF,
                0x7FFF0000,
                "multiply by max positive 16-bit",
            ),
            (
                0x10000,
                0x8000,
                0xffffffff80000000,
                "multiply by min negative 16-bit",
            ),
            (0x10000, 0xFFFF, 0xffffffffffff0000, "multiply by -1"),
        ];

        for (src_val, imm_val, expected, desc) in test_cases {
            let mut env = TestEnv::new();
            let src_offset = BinaryField16b::new(2);
            let dst_offset = BinaryField16b::new(4);

            // Set value in VROM at the computed address (FP ^ offset)
            env.set_vrom(src_offset.val(), src_val);
            let imm = BinaryField16b::new(imm_val);

            let event = MuliEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst_offset,
                src_offset,
                imm,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, expected,
                "MULI failed for {}: expected 0x{:x} got 0x{:x} (src=0x{:x}, imm=0x{:x})",
                desc, expected, event.dst_val, src_val, imm_val
            );
        }
    }

    /// Tests for Comparison operations (without immediate)
    #[test]
    fn test_comparison_operations() {
        // Test cases for SLT, SLTU
        let test_cases = [
            // (src1_val, src2_val, slt_expected, sltu_expected, description)
            (5, 10, 1, 1, "simple less than"),
            (10, 5, 0, 0, "simple greater than"),
            (5, 5, 0, 0, "equal values"),
            (0, 0, 0, 0, "zero comparison"),
            (0xFFFFFFFF, 5, 1, 0, "signed -1 < 5, unsigned MAX > 5"),
            (5, 0xFFFFFFFF, 0, 1, "signed 5 > -1, unsigned 5 < MAX"),
            (
                0x80000000,
                0x7FFFFFFF,
                1,
                0,
                "signed MIN < MAX, unsigned MIN > MAX",
            ),
            (
                0x7FFFFFFF,
                0x80000000,
                0,
                1,
                "signed MAX > MIN, unsigned MAX < MIN",
            ),
            (0, 0x80000000, 0, 1, "signed 0 > MIN, unsigned 0 < MIN"),
            (0x80000000, 0, 1, 0, "signed MIN < 0, unsigned MIN > 0"),
        ];

        for (src1_val, src2_val, slt_expected, sltu_expected, desc) in test_cases {
            // Test SLT (Signed Less Than)
            let mut env = TestEnv::new();
            let src1_offset = BinaryField16b::new(2);
            let src2_offset = BinaryField16b::new(3);
            let dst_offset = BinaryField16b::new(4);

            // Set values in VROM at the computed addresses (FP ^ offset)
            env.set_vrom(src1_offset.val(), src1_val);
            env.set_vrom(src2_offset.val(), src2_val);

            let event = SltEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst_offset,
                src1_offset,
                src2_offset,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, slt_expected,
                "SLT failed for {}: expected {} got {} (src1=0x{:x}, src2=0x{:x})",
                desc, slt_expected, event.dst_val, src1_val, src2_val
            );

            // Test SLTU (Unsigned Less Than)
            let mut env = TestEnv::new();
            // Set values in VROM at the computed addresses (FP ^ offset)
            env.set_vrom(src1_offset.val(), src1_val);
            env.set_vrom(src2_offset.val(), src2_val);

            let event = SltuEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst_offset,
                src1_offset,
                src2_offset,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, sltu_expected,
                "SLTU failed for {}: expected {} got {} (src1=0x{:x}, src2=0x{:x})",
                desc, sltu_expected, event.dst_val, src1_val, src2_val
            );
        }
    }

    /// Tests for Comparison operations (with immediate)
    #[test]
    fn test_comparison_immediate_operations() {
        // Test cases for SLTI, SLTIU
        let test_cases = [
            // (src_val, imm_val, slti_expected, sltiu_expected, description)
            (5, 10, 1, 1, "simple less than"),
            (10, 5, 0, 0, "simple greater than"),
            (5, 5, 0, 0, "equal values"),
            (0, 0, 0, 0, "zero comparison"),
            // Tests with max positive and min negative 16-bit immediates
            (0x7FFF, 0x7FFF, 0, 0, "equal to max positive immediate"),
            (0x7FFE, 0x7FFF, 1, 1, "just below max positive immediate"),
            (0x8000, 0x7FFF, 0, 0, "just above max positive immediate"),
            (0x0000, 0x7FFF, 1, 1, "0 vs 0x7FFF (max positive immediate)"),
            // Sign extension tests for immediates
            (
                0x0000,
                0x8000,
                0,
                1,
                "0 vs 0x8000 (sign extended to -32768)",
            ),
            (
                0x7FFF,
                0x8000,
                0,
                1,
                "0x7FFF vs 0x8000 (sign extended to -32768)",
            ),
            (0x8000, 0x8000, 0, 0, "0x8000 vs 0x8000 (equal values)"),
            (
                0x8001,
                0x8000,
                0,
                0,
                "0x8001 vs 0x8000 (both negative, first more negative)",
            ),
            // Tests with 0xFFFF (-1 signed, max unsigned)
            (0x0000, 0xFFFF, 0, 1, "0 vs 0xFFFF (sign extended to -1)"),
            (0xFFFE, 0xFFFF, 0, 1, "0xFFFE vs 0xFFFF (test just below)"),
            (0xFFFF, 0xFFFF, 0, 0, "0xFFFF vs 0xFFFF (equal values)"),
            (0x10000, 0xFFFF, 0, 0, "0x10000 vs 0xFFFF (test just above)"),
            (
                0xFFFFFFFF,
                0xFFFF,
                0,
                0,
                "-1 vs -1 (both sign extended to -1)",
            ),
            // Additional tests with signed vs unsigned interpretation
            (0xFFFFFFFF, 0x0005, 1, 0, "-1 vs 5 (signed vs unsigned)"),
            (0x00000005, 0xFFFF, 0, 1, "5 vs -1 (signed vs unsigned)"),
        ];

        for (src_val, imm_val, slti_expected, sltiu_expected, desc) in test_cases {
            // Test SLTI (Signed Less Than Immediate)
            let mut env = TestEnv::new();
            let src_offset = BinaryField16b::new(2);
            let dst_offset = BinaryField16b::new(4);
            let imm = BinaryField16b::new(imm_val);

            // Set value in VROM at the computed address (FP ^ offset)
            env.set_vrom(src_offset.val(), src_val);

            let event = SltiEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst_offset,
                src_offset,
                imm,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, slti_expected,
                "SLTI failed for {}: expected {} got {} (src=0x{:x}, imm=0x{:x})",
                desc, slti_expected, event.dst_val, src_val, imm_val
            );

            // Test SLTIU (Unsigned Less Than Immediate)
            let mut env = TestEnv::new();
            // Set value in VROM at the computed address (FP ^ offset)
            env.set_vrom(src_offset.val(), src_val);

            let event = SltiuEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst_offset,
                src_offset,
                imm,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, sltiu_expected,
                "SLTIU failed for {}: expected {} got {} (src=0x{:x}, imm=0x{:x})",
                desc, sltiu_expected, event.dst_val, src_val, imm_val
            );
        }
    }
}

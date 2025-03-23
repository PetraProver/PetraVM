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
        BinaryField32b::new((val.val() as i32).wrapping_add(imm.val() as i32) as u32)
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
        BinaryField32b::new((val1.val() as i32).wrapping_add(val2.val() as i32) as u32)
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
            src2: src1.val(),
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

/// Event for SLT.
///
/// Performs an SLT between two signed target addresses.
///
/// Logic:
///   1. FP[dst] = FP[src1] < FP[src2]
#[derive(Debug, Clone)]
pub(crate) struct SltEvent {
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

impl BinaryOperation for SltEvent {
    fn operation(val1: BinaryField32b, val2: BinaryField32b) -> BinaryField32b {
        // LT is checked using a SUB gadget.
        BinaryField32b::new(((val1.val() as i32) < (val2.val() as i32)) as u32)
    }
}

// Note: The addition is checked thanks to the ADD32 table.
impl_binary_operation!(SltEvent);
impl_event_for_binary_operation!(SltEvent);

/// Event for SLTIU.
///
/// Performs an SLTIU between an unsigned target address and immediate.
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

/// Event for SLTI.
///
/// Performs an SLTI between two target addresses.
///
/// Logic:
///   1. FP[dst] = FP[src1] < FP[src2]
#[derive(Debug, Clone)]
pub(crate) struct SltiEvent {
    pc: BinaryField32b,
    fp: u32,
    timestamp: u32,
    dst: u16,
    dst_val: u32,
    src: u16,
    src_val: u32,
    imm: u16,
}

impl BinaryOperation for SltiEvent {
    fn operation(val1: BinaryField32b, val2: BinaryField16b) -> BinaryField32b {
        // LT is checked using a SUB gadget.
        BinaryField32b::new(((val1.val() as i32) < (val2.val() as i32)) as u32)
    }
}

impl_immediate_binary_operation!(SltiEvent);
impl_event_for_binary_operation!(SltiEvent);

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
        BinaryField32b::new(((val1.val() as i32).wrapping_sub(val2.val() as i32)) as u32)
    }
}

// Note: The addition is checked thanks to the ADD32 table.
impl_binary_operation!(SubEvent);
impl_event_for_binary_operation!(SubEvent);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use binius_field::{BinaryField16b, BinaryField32b, Field};

    use crate::{
        event::{
            integer_ops::{
                AddEvent, AddiEvent, MuliEvent, MuluEvent, SignedMulEvent, SignedMulKind, SltEvent,
                SltiEvent, SltiuEvent, SltuEvent, SubEvent,
            },
            ImmediateBinaryOperation, NonImmediateBinaryOperation,
        },
        execution::{Interpreter, ZCrayTrace},
        memory::{Memory, ProgramRom, ValueRom},
    };

    // Helper struct to simplify test setup
    struct TestEnv {
        interpreter: Interpreter,
        trace: ZCrayTrace,
        field_pc: BinaryField32b,
    }

    impl TestEnv {
        fn new() -> Self {
            let mut interpreter = Interpreter::new(HashMap::new(), HashMap::new());
            interpreter.timestamp = 0;
            interpreter.pc = 1; // Start at PC = 1

            let memory = Memory::new(ProgramRom::new(), ValueRom::default());
            let trace = ZCrayTrace::new(memory);

            Self {
                interpreter,
                trace,
                field_pc: BinaryField32b::ONE,
            }
        }

        // Helper to set a value in VROM
        fn set_value(&mut self, slot: u16, value: u32) {
            self.trace
                .set_vrom_u32(self.interpreter.fp ^ slot as u32, value)
                .unwrap();
        }

        // Helper to get a value from VROM
        fn get_value(&self, slot: u16) -> u32 {
            self.trace
                .get_vrom_u32(self.interpreter.fp ^ slot as u32)
                .unwrap()
        }

        // Helper to get a u64 value from VROM (for multiplication results)
        fn get_value_u64(&self, slot: u16) -> u64 {
            self.trace
                .get_vrom_u64(self.interpreter.fp ^ slot as u32)
                .unwrap()
        }
    }

    #[test]
    fn test_add_operations() {
        // Test cases for ADD and ADDI
        let test_cases = [
            // (src1, src2, expected_result, description)
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
            (1, u32::MAX, 0, "commutative overflow"),
        ];

        for (src1_val, src2_val, expected, desc) in test_cases {
            let mut env = TestEnv::new();

            // Test ADD
            let src1 = BinaryField16b::new(2);
            let src2 = BinaryField16b::new(3);
            let dst = BinaryField16b::new(4);

            env.set_value(src1.val(), src1_val);
            env.set_value(src2.val(), src2_val);

            let event = AddEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst,
                src1,
                src2,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, expected,
                "ADD failed for {}: {}",
                desc, expected
            );
            assert_eq!(
                env.get_value(dst.val()),
                expected,
                "VROM update failed for ADD"
            );

            // Test ADDI using src1 and immediate
            let mut env = TestEnv::new();
            env.set_value(src1.val(), src1_val);

            let imm = if src2_val <= u16::MAX as u32 {
                BinaryField16b::new(src2_val as u16)
            } else {
                continue; // Skip test if immediate is too large
            };

            let event = AddiEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst,
                src1,
                imm,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, expected,
                "ADDI failed for {}: {}",
                desc, expected
            );
            assert_eq!(
                env.get_value(dst.val()),
                expected,
                "VROM update failed for ADDI"
            );
        }
    }

    #[test]
    fn test_sub_operation() {
        // Test cases for SUB
        let test_cases = [
            // (src1, src2, expected_result, description)
            (30, 20, 10, "simple subtraction"),
            (20, 30, 0xFFFFFFF6, "negative result"), // -10 as two's complement
            (0, 0, 0, "zero subtraction"),
            (0, 1, 0xFFFFFFFF, "underflow to -1"),
            (0x80000000, 1, 0x7FFFFFFF, "negative to positive underflow"),
            (
                0x7FFFFFFF,
                0xFFFFFFFF,
                0x80000000,
                "positive to negative overflow",
            ),
        ];

        for (src1_val, src2_val, expected, desc) in test_cases {
            let mut env = TestEnv::new();

            let src1 = BinaryField16b::new(2);
            let src2 = BinaryField16b::new(3);
            let dst = BinaryField16b::new(4);

            env.set_value(src1.val(), src1_val);
            env.set_value(src2.val(), src2_val);

            let event = SubEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst,
                src1,
                src2,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, expected,
                "SUB failed for {}: {}",
                desc, expected
            );
            assert_eq!(
                env.get_value(dst.val()),
                expected,
                "VROM update failed for SUB"
            );
        }
    }

    #[test]
    fn test_comparison_operations() {
        // Test cases for SLT, SLTU, SLTI, SLTIU
        let test_cases = [
            // (src1, src2, signed_result, unsigned_result, description)
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
            (0, 0x80000000, 0, 1, "signed 0 > MIN, unsigned 0 < MIN"),
        ];

        for (src1_val, src2_val, signed_expected, unsigned_expected, desc) in test_cases {
            // Test SLT
            let mut env = TestEnv::new();
            let src1 = BinaryField16b::new(2);
            let src2 = BinaryField16b::new(3);
            let dst = BinaryField16b::new(4);

            env.set_value(src1.val(), src1_val);
            env.set_value(src2.val(), src2_val);

            let event = SltEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst,
                src1,
                src2,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, signed_expected,
                "SLT failed for {}: {}",
                desc, signed_expected
            );

            // Test SLTU
            let mut env = TestEnv::new();
            env.set_value(src1.val(), src1_val);
            env.set_value(src2.val(), src2_val);

            let event = SltuEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst,
                src1,
                src2,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, unsigned_expected,
                "SLTU failed for {}: {}",
                desc, unsigned_expected
            );

            // Test SLTI (if immediate fits in u16)
            if src2_val <= u16::MAX as u32 {
                let mut env = TestEnv::new();
                env.set_value(src1.val(), src1_val);
                let imm = BinaryField16b::new(src2_val as u16);

                let event = SltiEvent::generate_event(
                    &mut env.interpreter,
                    &mut env.trace,
                    dst,
                    src1,
                    imm,
                    env.field_pc,
                )
                .unwrap();

                assert_eq!(
                    event.dst_val, signed_expected,
                    "SLTI failed for {}: {}",
                    desc, signed_expected
                );

                // Test SLTIU
                let mut env = TestEnv::new();
                env.set_value(src1.val(), src1_val);

                let event = SltiuEvent::generate_event(
                    &mut env.interpreter,
                    &mut env.trace,
                    dst,
                    src1,
                    imm,
                    env.field_pc,
                )
                .unwrap();

                assert_eq!(
                    event.dst_val, unsigned_expected,
                    "SLTIU failed for {}: {}",
                    desc, unsigned_expected
                );
            }
        }
    }

    #[test]
    fn test_multiplication_operations() {
        // Test cases for MUL, MULU, MULSU, MULI
        let test_cases = [
            // (src1, src2, signed_result, unsigned_result, signed_unsigned_result, description)
            (5, 7, 35, 35, 35, "simple multiplication"),
            (0, 0, 0, 0, 0, "zero multiplication"),
            (1, 0, 0, 0, 0, "identity with zero"),
            (0, 1, 0, 0, 0, "zero with identity"),
            (1, 1, 1, 1, 1, "identity"),
            (
                0xFFFFFFFF,
                2,
                -2i64 as u64,
                0x1FFFFFFFE,
                -2i64 as u64,
                "MAX*2 signed/unsigned",
            ),
            (
                0x80000000,
                2,
                -4294967296i64 as u64,
                0x100000000,
                -4294967296i64 as u64,
                "MIN*2",
            ),
            (
                -5i32 as u32,
                7,
                -35i64 as u64,
                30064771037,
                -35i64 as u64,
                "negative * positive",
            ),
            (
                5,
                -7i32 as u32,
                -35i64 as u64,
                21474836445,
                21474836445,
                "positive * negative",
            ),
            (
                -5i32 as u32,
                -7i32 as u32,
                35,
                18446744022169944099,
                18446744052234715171,
                "negative * negative",
            ),
            (
                0x10000000,
                0x10,
                0x100000000,
                0x100000000,
                0x100000000,
                "overflow test",
            ),
            (
                0x80000000,
                0x80000000,
                0x4000000000000000,
                0x4000000000000000,
                13835058055282163712,
                "large values",
            ),
        ];

        for (
            src1_val,
            src2_val,
            signed_expected,
            unsigned_expected,
            signed_unsigned_expected,
            desc,
        ) in test_cases
        {
            // Test MUL
            let mut env = TestEnv::new();
            let src1 = BinaryField16b::new(2);
            let src2 = BinaryField16b::new(3);
            let dst = BinaryField16b::new(4);

            env.set_value(src1.val(), src1_val);
            env.set_value(src2.val(), src2_val);

            let event = SignedMulEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst,
                src1,
                src2,
                env.field_pc,
                SignedMulKind::Mul,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, signed_expected,
                "MUL failed for {}: {}",
                desc, signed_expected
            );
            assert_eq!(
                env.get_value_u64(dst.val()),
                signed_expected,
                "VROM update failed for MUL"
            );

            // Test MULU
            let mut env = TestEnv::new();
            env.set_value(src1.val(), src1_val);
            env.set_value(src2.val(), src2_val);

            let event = MuluEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst,
                src1,
                src2,
                env.field_pc,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, unsigned_expected,
                "MULU failed for {}: {}",
                desc, unsigned_expected
            );
            assert_eq!(
                env.get_value_u64(dst.val()),
                unsigned_expected,
                "VROM update failed for MULU"
            );

            // Test MULSU
            let mut env = TestEnv::new();
            env.set_value(src1.val(), src1_val);
            env.set_value(src2.val(), src2_val);

            let event = SignedMulEvent::generate_event(
                &mut env.interpreter,
                &mut env.trace,
                dst,
                src1,
                src2,
                env.field_pc,
                SignedMulKind::Mulsu,
            )
            .unwrap();

            assert_eq!(
                event.dst_val, signed_unsigned_expected,
                "MULSU failed for {}: expected {} but got {}",
                desc, signed_unsigned_expected, event.dst_val
            );

            // Test MULI (if src2 fits in i16)
            if src2_val < 0x8000 || src2_val >= 0xFFFF8000 {
                let mut env = TestEnv::new();
                env.set_value(src1.val(), src1_val);

                let imm = BinaryField16b::new(src2_val as u16);

                let event = MuliEvent::generate_event(
                    &mut env.interpreter,
                    &mut env.trace,
                    dst,
                    src1,
                    imm,
                    env.field_pc,
                )
                .unwrap();

                assert_eq!(
                    event.dst_val, signed_expected,
                    "MULI failed for {}: {}",
                    desc, signed_expected
                );
                assert_eq!(
                    env.get_value_u64(dst.val()),
                    signed_expected,
                    "VROM update failed for MULI"
                );
            }
        }
    }

    #[test]
    fn test_mulu_schoolbook_computation() {
        // Test the schoolbook multiplication intermediate values
        let mut env = TestEnv::new();
        let src1 = BinaryField16b::new(2);
        let src2 = BinaryField16b::new(3);
        let dst = BinaryField16b::new(4);

        // Use values that will produce interesting intermediate results
        env.set_value(src1.val(), 0x12345678);
        env.set_value(src2.val(), 0xABCDEF00);

        let event = MuluEvent::generate_event(
            &mut env.interpreter,
            &mut env.trace,
            dst,
            src1,
            src2,
            env.field_pc,
        )
        .unwrap();

        // Check the final result
        let expected_result = (0x12345678_u64).wrapping_mul(0xABCDEF00_u64);
        assert_eq!(event.dst_val, expected_result);

        // Validate intermediate arrays have proper sizes
        assert_eq!(event.aux.len(), 8);
        assert_eq!(event.aux_sums.len(), 4);
        assert_eq!(event.cum_sums.len(), 2);

        // Check that auxiliary values are non-zero
        assert!(event.aux.iter().any(|&x| x > 0));
        assert!(event.aux_sums.iter().any(|&x| x > 0));
        assert!(event.cum_sums.iter().any(|&x| x > 0));

        // Additional check: Check that the additions of auxiliary sums yield final
        // result
        assert_eq!(
            expected_result,
            event.cum_sums[1] + (event.aux_sums[3] << 24),
            "The auxiliary sums don't add up to the final result"
        );
    }
}

pub mod model {
    use binius_field::{BinaryField16b, BinaryField32b};

    use crate::{
        emulator::{Interpreter, InterpreterChannels, InterpreterTables},
        event::{BinaryOperation, Event},
        fire_non_jump_event, impl_binary_operation, impl_event_for_binary_operation,
        impl_event_no_interaction_with_state_channel, impl_immediate_binary_operation,
    };
    /// Event for the Add64 gadget.
    #[derive(Debug, Clone)]
    pub(crate) struct Add64Event {
        timestamp: u32,
        output: u64,
        input1: u64,
        input2: u64,
        cout: u64,
    }

    impl Add64Event {
        pub const fn new(timestamp: u32, output: u64, input1: u64, input2: u64, cout: u64) -> Self {
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

            let cout = (output ^ input1 ^ input2) >> (1 + (carry as u64)) << 63;

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

    /// Event for the Add32 gadget.
    #[derive(Debug, Clone)]
    pub struct Add32Event {
        pub(crate) timestamp: u32,
        pub(crate) output: u32,
        pub(crate) input1: u32,
        pub(crate) input2: u32,
        pub(crate) cout: u32,
    }

    impl Add32Event {
        pub const fn new(timestamp: u32, output: u32, input1: u32, input2: u32, cout: u32) -> Self {
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

            let cout = (output ^ inp1 ^ inp2) >> (1 + (carry as u32)) << 31;

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

    /// Event for ADDI.
    ///
    /// Performs an ADD between a target address and an immediate.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src] + imm
    #[derive(Debug, Clone)]
    pub(crate) struct AddiEvent {
        pub(crate) pc: BinaryField32b,
        pub(crate) fp: u32,
        pub(crate) timestamp: u32,
        pub(crate) dst: u16,
        pub(crate) dst_val: u32,
        pub(crate) src: u16,
        pub(crate) src_val: u32,
        pub(crate) imm: u16,
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

    /// Event for ADD.
    ///
    /// Performs an ADD between two target addresses.
    ///
    /// Logic:
    ///   1. FP[dst] = FP[src1] + FP[src2]
    #[derive(Debug, Clone)]
    pub struct AddEvent {
        pub(crate) pc: BinaryField32b,
        pub(crate) fp: u32,
        pub(crate) timestamp: u32,
        pub(crate) dst: u16,
        pub(crate) dst_val: u32,
        pub(crate) src1: u16,
        pub(crate) src1_val: u32,
        pub(crate) src2: u16,
        pub(crate) src2_val: u32,
    }

    impl BinaryOperation for AddEvent {
        fn operation(val1: BinaryField32b, val2: BinaryField32b) -> BinaryField32b {
            BinaryField32b::new(val1.val() + val2.val())
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
        // Note: we don't need the third  sum value (equal to sum0 + sum1 <<8) because it is equal
        // to DST_VAL.
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
            dst: BinaryField16b,
            src: BinaryField16b,
            imm: BinaryField16b,
        ) -> Self {
            let fp = interpreter.fp;
            let src_val = interpreter.vrom.get_u32(fp ^ src.val() as u32);

            let imm_val = imm.val();
            let dst_val = src_val * imm_val as u32; // TODO: shouldn't the result be u64, stored over two slots?

            interpreter.vrom.set_u32(fp ^ dst.val() as u32, dst_val);

            let (aux, sum0, sum1) =
                schoolbook_multiplication_intermediate_sums(src_val, imm_val, dst_val);

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
                imm: imm_val,
                aux,
                sum0,
                sum1,
            }
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
}

pub mod arithmetization {
    use std::time;

    use binius_core::{
        constraint_system::channel::{Boundary, ChannelId, FlushDirection},
        fiat_shamir::HasherChallenger,
        oracle::ShiftVariant,
        tower::CanonicalTowerFamily,
        witness::MultilinearExtensionIndex,
    };
    use binius_field::{
        arch::OptimalUnderlier128b,
        as_packed_field::{PackScalar, PackedType},
        underlier::UnderlierType,
        BinaryField, Field, PackedField,
    };
    use binius_m3::{
        builder::{
            upcast_col, Col, ConstraintSystem, Statement, TableFiller, TableId,
            TableWitnessIndexSegment, B1, B128, B16, B32,
        },
        gadgets::u32::{U32Add, U32AddFlags},
    };
    use bytemuck::Pod;

    use super::model::{self, AddEvent};
    use crate::emulator::InterpreterChannels;

    pub struct Add32Table {
        id: TableId,
        xin: Col<B1, 32>,
        yin: Col<B1, 32>,
        xin_packed: Col<B32>,
        yin_packed: Col<B32>,
        zout_packed: Col<B32>,
        timestamp: Col<B32>,
        inner_add: U32Add,
    }

    impl Add32Table {
        pub fn new(cs: &mut ConstraintSystem, channel: ChannelId) -> Self {
            let mut table = cs.add_table("add32");
            let xin = table.add_committed("x1");
            let xin_packed = table.add_packed("x1_packed", xin);
            let yin = table.add_committed("yin");
            let yin_packed = table.add_packed("yin_packed", yin);

            let timestamp = table.add_committed("timestamp");
            let inner_add = U32Add::new(&mut table, xin, yin, U32AddFlags::default());

            let zout_packed = table.add_packed("zout_packed", inner_add.zout);

            table.pull(
                channel,
                [
                    upcast_col(timestamp),
                    upcast_col(xin_packed),
                    upcast_col(yin_packed),
                ],
            );
            table.push(channel, [upcast_col(timestamp), upcast_col(zout_packed)]);

            Self {
                id: table.id(),
                xin,
                yin,
                xin_packed,
                yin_packed,
                zout_packed,
                timestamp,
                inner_add,
            }
        }
    }

    impl<U: UnderlierType> TableFiller<U> for Add32Table
    where
        U: Pod + PackScalar<B1>,
    {
        type Event = model::Add32Event;

        fn id(&self) -> TableId {
            self.id
        }

        fn fill<'a>(
            &'a self,
            rows: impl Iterator<Item = &'a Self::Event>,
            witness: &'a mut TableWitnessIndexSegment<U, binius_field::BinaryField128b>,
        ) -> anyhow::Result<()> {
            {
                let mut xin = witness.get_mut_as(self.inner_add.xin)?;
                let mut yin = witness.get_mut_as(self.inner_add.yin)?;
                let mut xin_packed = witness.get_mut_as(self.xin_packed)?;
                let mut yin_packed = witness.get_mut_as(self.yin_packed)?;
                let mut zout_packed = witness.get_mut_as(self.zout_packed)?;
                let mut timestamp = witness.get_mut_as(self.timestamp)?;
                for (i, event) in rows.enumerate() {
                    xin[i] = event.input1;
                    yin[i] = event.input2;
                    xin_packed[i] = event.input1;
                    yin_packed[i] = event.input2;
                    timestamp[i] = event.timestamp;
                    zout_packed[i] = event.output;
                }
            }
            self.inner_add.populate(witness)?;
            Ok(())
        }
    }
    pub struct AddTable {
        id: TableId,
        pc: Col<B32>,
        fp: Col<B32>,
        timestamp: Col<B32>,
        next_timestamp: Col<B32>, // TODO: This is currently unconstrained
        dst: Col<B16>,
        dst_val: Col<B32>,
        src1: Col<B16>,
        src2: Col<B16>,
        src1_val: Col<B32>,
        src2_val: Col<B32>,
    }

    impl AddTable {
        pub fn new(
            cs: &mut ConstraintSystem,
            state_channel: ChannelId,
            add32_channel: ChannelId,
        ) -> Self {
            let mut table = cs.add_table("add");

            let pc = table.add_committed("pc");
            let fp = table.add_committed("fp");
            let timestamp = table.add_committed("timestamp");

            let src1 = table.add_committed("src1");
            let src2 = table.add_committed("src2");

            let src1_val = table.add_committed("src1_val");
            let src2_val = table.add_committed("src2_val");

            let dst = table.add_committed("dst");
            let dst_val = table.add_committed("dst_val");

            let next_pc =
                table.add_linear_combination("next_pc", pc * B32::MULTIPLICATIVE_GENERATOR);

            // TODO: Next timestamp should be either timestamp + 1 or timestamp*G.
            let next_timestamp = table.add_committed("next_timestamp");

            table.push(
                state_channel,
                [upcast_col(pc), upcast_col(fp), upcast_col(timestamp)],
            );
            table.pull(
                state_channel,
                [
                    upcast_col(next_pc),
                    upcast_col(fp),
                    upcast_col(next_timestamp),
                ],
            );

            table.push(
                add32_channel,
                [
                    upcast_col(timestamp),
                    upcast_col(src1_val),
                    upcast_col(src2_val),
                ],
            );
            table.pull(add32_channel, [upcast_col(timestamp), upcast_col(dst_val)]);

            Self {
                id: table.id(),
                pc,
                fp,
                timestamp,
                next_timestamp,
                src1,
                src2,
                src1_val,
                src2_val,
                dst,
                dst_val,
            }
        }
    }

    impl<U: UnderlierType> TableFiller<U> for AddTable
    where
        U: Pod + PackScalar<B1>,
    {
        type Event = model::AddEvent;

        fn id(&self) -> TableId {
            self.id
        }

        fn fill<'a>(
            &self,
            rows: impl Iterator<Item = &'a Self::Event>,
            witness: &'a mut TableWitnessIndexSegment<U>,
        ) -> Result<(), anyhow::Error> {
            {
                let mut pc: std::cell::RefMut<'_, [u32]> = witness.get_mut_as(self.pc)?;
                let mut fp = witness.get_mut_as(self.fp)?;
                let mut timestamp = witness.get_mut_as(self.timestamp)?;

                let mut next_timestamp = witness.get_mut_as(self.next_timestamp)?;

                let mut src1 = witness.get_mut_as(self.src1)?;
                let mut src2 = witness.get_mut_as(self.src2)?;
                let mut src1_val = witness.get_mut_as(self.src1_val)?;
                let mut src2_val = witness.get_mut_as(self.src2_val)?;
                let mut dst = witness.get_mut_as(self.dst)?;
                let mut dst_val = witness.get_mut_as(self.dst_val)?;

                for (i, event) in rows.enumerate() {
                    pc[i] = event.pc.into();
                    fp[i] = event.fp;
                    timestamp[i] = event.timestamp;
                    next_timestamp[i] = event.timestamp + 1u32;

                    src1[i] = event.src1;
                    src2[i] = event.src2;
                    src1_val[i] = event.src1_val;
                    src2_val[i] = event.src2_val;
                    dst[i] = event.dst;
                    dst_val[i] = event.dst_val;
                }
            }
            Ok(())
        }
    }

    struct AddiTable {
        id: TableId,
        pc: Col<B32>,
        fp: Col<B32>,
        timestamp: Col<B32>,
        next_timestamp: Col<B32>, // TODO: This is currently unconstrained
        dst: Col<B16>,
        dst_val: Col<B32>,
        src: Col<B16>,
        src_val: Col<B32>,
        imm: Col<B16>,
    }

    impl AddiTable {
        pub fn new(
            cs: &mut ConstraintSystem,
            state_channel: ChannelId,
            add32_channel: ChannelId,
        ) -> Self {
            let mut table = cs.add_table("addi");

            let pc = table.add_committed("pc");
            let fp = table.add_committed("fp");
            let timestamp = table.add_committed("timestamp");

            let src = table.add_committed("src1");
            let src_val = table.add_committed("src1_val");
            let imm =   table.add_committed("imm");

            let dst = table.add_committed("dst");
            let dst_val = table.add_committed("dst_val");

            let next_pc =
                table.add_linear_combination("next_pc", pc * B32::MULTIPLICATIVE_GENERATOR);

            // TODO: Next timestamp should be either timestamp + 1 or timestamp*G.
            let next_timestamp = table.add_committed("next_timestamp");

            table.push(
                state_channel,
                [upcast_col(pc), upcast_col(fp), upcast_col(timestamp)],
            );
            table.pull(
                state_channel,
                [
                    upcast_col(next_pc),
                    upcast_col(fp),
                    upcast_col(next_timestamp),
                ],
            );

            table.push(
                add32_channel,
                [
                    upcast_col(timestamp),
                    upcast_col(src_val),
                    upcast_col(imm),
                ],
            );
            table.pull(add32_channel, [upcast_col(timestamp), upcast_col(dst_val)]);

            Self {
                id: table.id(),
                pc,
                fp,
                timestamp,
                next_timestamp,
                src,
                src_val,
                dst,
                dst_val,
                imm,
            }
        }
    }

    impl<U: UnderlierType> TableFiller<U> for AddiTable
    where
        U: Pod + PackScalar<B1>,
    {
        type Event = model::AddiEvent;

        fn id(&self) -> TableId {
            self.id
        }

        fn fill<'a>(
            &self,
            rows: impl Iterator<Item = &'a Self::Event>,
            witness: &'a mut TableWitnessIndexSegment<U>,
        ) -> Result<(), anyhow::Error> {
            {
                let mut pc: std::cell::RefMut<'_, [u32]> = witness.get_mut_as(self.pc)?;
                let mut fp = witness.get_mut_as(self.fp)?;
                let mut timestamp = witness.get_mut_as(self.timestamp)?;

                let mut next_timestamp = witness.get_mut_as(self.next_timestamp)?;

                let mut src1 = witness.get_mut_as(self.src)?;
                let mut src1_val = witness.get_mut_as(self.src_val)?;
                let mut dst = witness.get_mut_as(self.dst)?;
                let mut dst_val = witness.get_mut_as(self.dst_val)?;
                let mut imm = witness.get_mut_as(self.imm)?;

                for (i, event) in rows.enumerate() {
                    pc[i] = event.pc.into();
                    fp[i] = event.fp;
                    timestamp[i] = event.timestamp;
                    next_timestamp[i] = event.timestamp + 1u32;

                    src1[i] = event.src;
                    src1_val[i] = event.src_val;
                    dst[i] = event.dst;
                    dst_val[i] = event.dst_val;
                    imm[i] = event.imm;
                }
            }
            Ok(())
        }
    }
}

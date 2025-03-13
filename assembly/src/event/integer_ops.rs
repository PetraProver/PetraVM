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
            let src_val = interpreter.get_u32(fp ^ src.val() as u32);
            // The following addition is checked thanks to the ADD32 table.
            let dst_val = src_val + imm.val() as u32;
            interpreter.set_vrom_u32(fp ^ dst.val() as u32, dst_val);

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
            let src_val = interpreter.get_vrom_u32(fp ^ src.val() as u32);

            let imm_val = imm.val();
            let dst_val = src_val * imm_val as u32; // TODO: shouldn't the result be u64, stored over two slots?

            interpreter.set_vrom_u32(fp ^ dst.val() as u32, dst_val);

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
            upcast_col, Col, ConstraintSystem, Expr, Statement, TableFiller, TableId, TableWitnessIndexSegment, B1, B128, B16, B32, B64
        },
        gadgets::u32::{U32Add, U32AddFlags},
    };
    use bytemuck::Pod;

    use super::model::{self, AddEvent};
    use crate::{emulator::InterpreterChannels, opcodes::Opcode};

    pub struct AddTable {
        id: TableId,
        pc: Col<B32>,
        fp: Col<B32>,
        opcode: Col<B16>, // This should be a transparent column
        timestamp: Col<B32>,
        next_timestamp: Col<B32>, // TODO: This is currently unconstrained
        src1: Col<B16>,
        src2: Col<B16>,
        dst: Col<B16>,
        dst_val_packed: Col<B32>,
        src1_val: Col<B1, 32>,
        src1_val_packed: Col<B32>,
        src2_val: Col<B1, 32>,
        src2_val_packed: Col<B32>,
        u32_add: U32Add,
    }

    impl AddTable {
        pub fn new(
            cs: &mut ConstraintSystem,
            state_channel: ChannelId,
            vrom_channel: ChannelId,
            prom_channel: ChannelId,
        ) -> Self {
            let mut table = cs.add_table("axdd");

            

            let pc = table.add_committed("pc");
            let fp = table.add_committed("fp");
            let timestamp = table.add_committed("timestamp");
            let opcode = table.add_committed("opcode"); //TODO: opcode must be transparent
            // let a = table.add_linear_combination("opcode", B16::new(Opcode::Add as u16));
            table.assert_zero("opcode_is_correct", opcode - B16::new(Opcode::Add as u16));

            let src1 = table.add_committed("src1");
            let src2 = table.add_committed("src2");
            let dst = table.add_committed("dst");

            let src1_val = table.add_committed("src1_val");
            let src2_val = table.add_committed("src2_val");

            let src1_val_packed = table.add_packed("src1_val_packed", src1_val);
            let src2_val_packed = table.add_packed("src2_val_packed", src1_val);

            let next_pc =
                table.add_linear_combination("next_pc", pc * B32::MULTIPLICATIVE_GENERATOR);

            // TODO: Next timestamp should be either timestamp + 1 or timestamp*G.
            let next_timestamp = table.add_committed("next_timestamp");

            let u32_add = U32Add::new(&mut table, src1_val, src2_val, U32AddFlags::default());
            let dst_val_packed = table.add_packed("dst_val_packed", u32_add.zout);

            // Reado opcode
            table.push(prom_channel, [upcast_col(pc), upcast_col(opcode), upcast_col(dst), upcast_col(src1), upcast_col(src2)]);

            // Read src1
            table.push(vrom_channel, [upcast_col(timestamp), upcast_col(src1), upcast_col(src1_val_packed)]);
            // Read src2
            table.push(vrom_channel, [upcast_col(timestamp), upcast_col(src2), upcast_col(src2_val_packed)]);
            // Write dst
            table.push(vrom_channel, [upcast_col(timestamp), upcast_col(dst), upcast_col(dst_val_packed)]);


            // Flushing rules for the state channel
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

            Self {
                id: table.id(),
                pc,
                fp,
                opcode,
                timestamp,
                next_timestamp,
                src1,
                src2,
                dst,
                src1_val,
                src2_val,
                src1_val_packed,
                src2_val_packed,
                u32_add,
                dst_val_packed,
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
                let mut dst = witness.get_mut_as(self.dst)?;
                let mut src1_val = witness.get_mut_as(self.src1_val)?;
                let mut src2_val = witness.get_mut_as(self.src2_val)?;
                let mut src1_val_packed = witness.get_mut_as(self.src1_val_packed)?;
                let mut src2_val_packed = witness.get_mut_as(self.src2_val_packed)?;

                for (i, event) in rows.enumerate() {
                    pc[i] = event.pc.into();
                    fp[i] = event.fp;
                    timestamp[i] = event.timestamp;
                    next_timestamp[i] = event.timestamp + 1u32;

                    src1[i] = event.src1;
                    src2[i] = event.src2;
                    dst[i] = event.dst;
                    src1_val[i] = event.src1_val;
                    src2_val[i] = event.src2_val;
                    src1_val_packed[i] = event.src1_val;
                    src2_val_packed[i] = event.src2_val;
                }
            }
            self.u32_add.populate(witness);
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
        src: Col<B16>,
        src_val: Col<B1, 32>,
        src_val_packed: Col<B32>,
        imm: Col<B1, 32>, // TODO: Should only use 16 columns
        imm_packed: Col<B16>,
        u32_add: U32Add,
    }

    impl AddiTable {
        pub fn new(
            cs: &mut ConstraintSystem,
            state_channel: ChannelId,
        ) -> Self {
            let mut table = cs.add_table("addi");

            let pc = table.add_committed("pc");
            let fp = table.add_committed("fp");
            let timestamp = table.add_committed("timestamp");

            let src = table.add_committed("src1");
            let src_val = table.add_committed("src1_val");
            let src_val_packed = table.add_packed("src_val_packed", src_val);
            let imm =   table.add_committed("imm");
            let imm_packed = table.add_packed("imm_packed", imm);

            let dst = table.add_committed("dst");

            let next_pc =
                table.add_linear_combination("next_pc", pc * B32::MULTIPLICATIVE_GENERATOR);

            // TODO: Next timestamp should be either timestamp + 1 or timestamp*G.
            let next_timestamp = table.add_committed("next_timestamp");

            let u32_add = U32Add::new(&mut table, src_val, imm, U32AddFlags::default());

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

            Self {
                id: table.id(),
                pc,
                fp,
                timestamp,
                next_timestamp,
                src,
                src_val,
                src_val_packed,
                dst,
                imm,
                imm_packed,
                u32_add,
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
                let mut imm = witness.get_mut_as(self.imm)?;

                for (i, event) in rows.enumerate() {
                    pc[i] = event.pc.into();
                    fp[i] = event.fp;
                    timestamp[i] = event.timestamp;
                    next_timestamp[i] = event.timestamp + 1u32;

                    src1[i] = event.src;
                    src1_val[i] = event.src_val;
                    dst[i] = event.dst;
                    imm[i] = event.imm;
                }
            }
            Ok(())
        }
    }

    // struct MuliTable {
    //     id: TableId,
    //     pc: Col<B32>,
    //     fp: Col<B32>,
    //     timestamp: Col<B32>,
    //     next_timestamp: Col<B32>, // TODO: This is currently unconstrained
    //     dst: Col<B16>,
    //     src: Col<B16>,
    //     src_val_packed: Col<B32>,
    //     imm: Col<B1, 32>, // TODO: Should only use 16 cols
    //     imm_packed: Col<B16>,
    //     aux: Col<B32>,
    //     sum0: Col<B64>,
    //     sum1: Col<B64>,
    //     add1: U64Add,
    // }

    // impl MuliTable {
    //     pub fn new(
    //         cs: &mut ConstraintSystem,
    //         state_channel: ChannelId,
    //         add64_channel: ChannelId,
    //         mul8_channel: ChannelId,
    //     ) -> Self {
    //         let mut table = cs.add_table("muli");

    //         let pc = table.add_committed("pc");
    //         let fp = table.add_committed("fp");
    //         let timestamp = table.add_committed("timestamp");

    //         let src = table.add_committed("src");
    //         let src_val = table.add_committed("src_val");
    //         let src_val_packed = table.add_packed(table, "src_val_packed");
    //         let imm =   table.add_committed("imm");

    //         let dst = table.add_committed("dst");
    //         let dst_val = table.add_committed("dst_val");

    //         let aux = table.add_committed("aux");
    //         let sum0 = table.add_committed("sum0");
    //         let sum1 = table.add_committed("sum1");

    //         let next_pc =
    //             table.add_linear_combination("next_pc", pc * B32::MULTIPLICATIVE_GENERATOR);

    //         // TODO: Next timestamp should be either timestamp + 1 or timestamp*G.
    //         let next_timestamp = table.add_committed("next_timestamp");

    //         table.push(
    //             state_channel,
    //             [upcast_col(pc), upcast_col(fp), upcast_col(timestamp)],
    //         );
    //         table.pull(
    //             state_channel,
    //             [
    //                 upcast_col(next_pc),
    //                 upcast_col(fp),
    //                 upcast_col(next_timestamp),
    //             ],
    //         );

    //         table.push(
    //             add32_channel,
    //             [
    //                 upcast_col(timestamp),
    //                 upcast_col(src_val),
    //                 upcast_col(imm),
    //             ],
    //         );
    //         table.pull(add32_channel, [upcast_col(timestamp), upcast_col(dst_val)]);

    //         Self {
    //             id: table.id(),
    //             pc,
    //             fp,
    //             timestamp,
    //             next_timestamp,
    //             src,
    //             dst,
    //             imm,
    //             src_val_packed,
    //             imm_packed: todo!(),
    //             aux,
    //             sum0,
    //             sum1,
    //             add1: todo!(),
    //         }
    //     }
    // }

    #[test]
	fn test_addi() {
		let mut cs = ConstraintSystem::new();
		let state_channel = cs.add_channel("state_channel");
        let prom_channel = cs.add_channel("prom_channel");
        let vrom_channel = cs.add_channel("vrom_channel");
		let fibonacci_table = FibonacciTable::new(&mut cs, fibonacci_pairs);
		let trace = FibonacciTrace::generate((0, 1), 40);
		let statement = Statement {
			boundaries: vec![
				Boundary {
					values: vec![B128::new(0), B128::new(1)],
					channel_id: fibonacci_pairs,
					direction: FlushDirection::Push,
					multiplicity: 1,
				},
				Boundary {
					values: vec![B128::new(165580141), B128::new(267914296)],
					channel_id: fibonacci_pairs,
					direction: FlushDirection::Pull,
					multiplicity: 1,
				},
			],
			table_sizes: vec![trace.rows.len()],
		};
		let allocator = Bump::new();
		let mut witness = cs
			.build_witness::<OptimalUnderlier128b>(&allocator, &statement)
			.unwrap();

		witness
			.fill_table_sequential(&fibonacci_table, &trace.rows)
			.unwrap();

		let compiled_cs = cs.compile(&statement).unwrap();
		let witness = witness.into_multilinear_extension_index::<B128>(&statement);

		binius_core::constraint_system::validate::validate_witness(
			&compiled_cs,
			&statement.boundaries,
			&witness,
		)
		.unwrap();
	}
}

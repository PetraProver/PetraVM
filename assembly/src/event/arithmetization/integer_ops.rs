
    use std::{collections::HashMap, time};

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
            upcast_col, Col, ConstraintSystem, Expr, Statement, TableFiller, TableId,
            TableWitnessIndexSegment, B1, B128, B16, B32, B64,
        },
        gadgets::u32::{U32Add, U32AddFlags},
    };
    use bumpalo::Bump;
    use bytemuck::Pod;

    use crate::{
        code_to_prom,
        emulator::{BoundaryValues, InterpreterChannels},
        emulator_arithmetization::arithmetization::ZCrayTable,
        opcodes::Opcode,
        ValueRom, ZCrayTrace,
    };

    pub struct AddTable {
        id: TableId,
        // TODO: Use the cpu gadget
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
            let mut table = cs.add_table("add");

            let pc = table.add_committed("pc");
            let fp = table.add_committed("fp");
            let timestamp = table.add_committed("timestamp");
            let opcode = table.add_committed("opcode"); //TODO: opcode must be transparent
                                                        // let a = table.add_linear_combination("opcode", B16::new(Opcode::Add as u16));
            // TODO: Why opcode - B16::new(Opcode::Add as u16) doesn't work?
            table.assert_zero("opcode_is_correcto", opcode.into());

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
            table.push(
                prom_channel,
                [
                    upcast_col(pc),
                    upcast_col(opcode),
                    upcast_col(dst),
                    upcast_col(src1),
                    upcast_col(src2),
                ],
            );

            // Read src1
            table.push(
                vrom_channel,
                [
                    upcast_col(timestamp),
                    upcast_col(src1),
                    upcast_col(src1_val_packed),
                ],
            );
            // Read src2
            table.push(
                vrom_channel,
                [
                    upcast_col(timestamp),
                    upcast_col(src2),
                    upcast_col(src2_val_packed),
                ],
            );
            // Write dst
            table.push(
                vrom_channel,
                [
                    upcast_col(timestamp),
                    upcast_col(dst),
                    upcast_col(dst_val_packed),
                ],
            );

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
        type Event = crate::event::model::integer_ops::AddEvent;

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

                let mut opcode = witness.get_mut_as(self.opcode)?;
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

                    opcode[i] = Opcode::Add as u16;
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

    pub struct AddiTable {
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
        pub fn new(cs: &mut ConstraintSystem, state_channel: ChannelId) -> Self {
            let mut table = cs.add_table("addi");

            let pc = table.add_committed("pc");
            let fp = table.add_committed("fp");
            let timestamp = table.add_committed("timestamp");

            let src = table.add_committed("src1");
            let src_val = table.add_committed("src1_val");
            let src_val_packed = table.add_packed("src_val_packed", src_val);
            let imm = table.add_committed("imm");
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
        type Event = crate::event::model::integer_ops::AddiEvent;

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
    //             table.add_linear_combination("next_pc", pc *
    // B32::MULTIPLICATIVE_GENERATOR);

    //         // TODO: Next timestamp should be either timestamp + 1 or
    // timestamp*G.         let next_timestamp =
    // table.add_committed("next_timestamp");

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
    //         table.pull(add32_channel, [upcast_col(timestamp),
    // upcast_col(dst_val)]);

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
        let zcray_table = ZCrayTable::new(&mut cs);

        let zero = B16::zero();
        // TODO: This is a Ret!!!
        let code = vec![
            [Opcode::Add.get_field_elt(), zero, zero, zero],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];
        let prom = code_to_prom(&code, &[false; 2]);
        let vrom = ValueRom::new();
        let mut frames = HashMap::new();
        frames.insert(B32::ONE, 12);

        let (
            trace,
            BoundaryValues {
                final_pc,
                final_fp,
                timestamp: final_timestamp,
            },
        ) = ZCrayTrace::generate_with_vrom(prom, vrom, frames, HashMap::new()).expect("Ouch!");
        let statement = Statement {
            boundaries: vec![
                Boundary {
                    values: vec![B128::new(0), B128::new(0), B128::new(0)], /* inital_pc = 0,
                                                                             * inital_fp = 0,
                                                                             * initial_timestamp
                                                                             * = 0 */
                    channel_id: zcray_table.state_channel,
                    direction: FlushDirection::Push,
                    multiplicity: 1,
                },
                Boundary {
                    values: vec![
                        B128::new(final_pc.val() as u128),
                        B128::new(final_fp as u128),
                        B128::new(final_timestamp as u128),
                    ],
                    channel_id: zcray_table.state_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
            ],
            table_sizes: vec![trace.add.len(), trace.ret.len()], // TODO: What should be here?
        };
        let allocator = Bump::new();
        let mut witness = cs
            .build_witness::<OptimalUnderlier128b>(&allocator, &statement)
            .unwrap();

        zcray_table.populate(trace, &mut witness);

        let compiled_cs = cs.compile(&statement).unwrap();
        let witness = witness.into_multilinear_extension_index(&statement);

        binius_core::constraint_system::validate::validate_witness(
            &compiled_cs,
            &statement.boundaries,
            &witness,
        )
        .unwrap();
    }
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
    BinaryField, ExtensionField, Field, PackedField,
};
use binius_m3::{
    builder::{
        upcast_col, upcast_expr, Col, ConstraintSystem, Expr, Statement, TableFiller, TableId,
        TableWitnessIndexSegment, B1, B128, B16, B32, B64,
    },
    gadgets::u32::{U32Add, U32AddFlags},
};
use binius_math::DefaultEvaluationDomainFactory;
use bumpalo::Bump;
use bytemuck::Pod;
use groestl_crypto::Groestl256;

use super::cpu::{CpuColumns, CpuColumnsOptions, CpuRow, Instruction, NextPc};
use crate::{
    execution::{
        emulator::InterpreterChannels,
        emulator_arithmetization::arithmetization::ZCrayTable,
        trace::{BoundaryValues, ZCrayTrace},
    },
    opcodes::Opcode,
    ValueRom,
};

pub struct AddTable {
    id: TableId,
    // TODO: Use the cpu gadget
    cpu_cols: CpuColumns,
    dst_val_packed: Col<B32>,
    src1_val: Col<B1, 32>,
    src1_val_packed: Col<B32>,
    src2_val: Col<B1, 32>,
    src2_val_packed: Col<B32>,
    u32_add: U32Add,

    vrom_src1: Col<B64>,
    vrom_src2: Col<B64>,
    vrom_dst: Col<B64>,
}

impl AddTable {
    pub fn new(
        cs: &mut ConstraintSystem,
        state_channel: ChannelId,
        vrom_channel: ChannelId,
        prom_channel: ChannelId,
    ) -> Self {
        let mut table = cs.add_table("add");

        let cpu = CpuColumns::new::<{ Opcode::Add as u16 }>(
            &mut table,
            state_channel,
            prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let dst = cpu.arg0;
        let src1 = cpu.arg1;
        let src2 = cpu.arg2;

        let src1_val = table.add_committed("src1_val");
        let src2_val = table.add_committed("src2_val");

        let src1_val_packed = table.add_packed("src1_val_packed", src1_val);
        let src2_val_packed = table.add_packed("src2_val_packed", src1_val);

        let u32_add = U32Add::new(&mut table, src1_val, src2_val, U32AddFlags::default());
        let dst_val_packed = table.add_packed("dst_val_packed", u32_add.zout);

        // TODO: Load this from some utility module
        let b64_basis: [_; 2] = std::array::from_fn(|i| {
            <B64 as ExtensionField<B32>>::basis(i).expect("i in range 0..2; extension degree is 2")
        });
        let pack_b32_into_b64 = move |limbs: [Expr<B32, 1>; 2]| {
            limbs
                .into_iter()
                .enumerate()
                .map(|(i, limb)| upcast_expr(limb) * b64_basis[i])
                .reduce(|a, b| a + b)
                .expect("limbs has length 2")
        };

        // Read src1
        let vrom_src1 = table.add_computed(
            "src1_src1_val",
            pack_b32_into_b64([upcast_col(src1).into(), src1_val_packed.into()]),
        );
        table.push(vrom_channel, [vrom_src1]);
        // Read src2
        let vrom_src2 = table.add_computed(
            "src2_src2_val",
            pack_b32_into_b64([upcast_col(src2).into(), src2_val_packed.into()]),
        );
        table.push(vrom_channel, [vrom_src2]);
        // Write dst
        let vrom_dst = table.add_computed(
            "dst_dst_val",
            pack_b32_into_b64([upcast_col(dst).into(), dst_val_packed.into()]),
        );
        table.push(vrom_channel, [vrom_dst]);

        Self {
            id: table.id(),
            cpu_cols: cpu,
            src1_val,
            src2_val,
            src1_val_packed,
            src2_val_packed,
            u32_add,
            dst_val_packed,
            vrom_src1,
            vrom_src2,
            vrom_dst,
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
            for (i, event) in rows.enumerate() {
                println!("Add");
                self.cpu_cols.fill_row(
                    witness,
                    CpuRow {
                        index: i,
                        pc: event.pc.into(),
                        next_pc: None,
                        fp: event.fp,
                        instruction: Instruction {
                            opcode: Opcode::Add,
                            arg0: event.dst,
                            arg1: event.src1,
                            arg2: event.src2,
                        },
                    },
                );
                // TODO: Move this outside the loop
                let mut src1_val = witness.get_mut_as(self.src1_val)?;
                let mut src2_val = witness.get_mut_as(self.src2_val)?;
                let mut vrom_src1 = witness.get_mut_as(self.vrom_src1)?;
                let mut vrom_src2 = witness.get_mut_as(self.vrom_src2)?;
                let mut vrom_dst = witness.get_mut_as(self.vrom_dst)?;
                src1_val[i] = event.src1_val;
                src2_val[i] = event.src2_val;
                vrom_src1[i] = (event.src1 as u64) << 32 | event.src1_val as u64;
                vrom_src2[i] = (event.src2 as u64) << 32 | event.src2_val as u64;
                vrom_dst[i] = (event.dst as u64) << 32 | event.dst_val as u64;
            }
        }
        self.u32_add.populate(witness);
        Ok(())
    }
}

pub struct AddiTable {
    id: TableId,
    cpu_cols: CpuColumns,
    src_val: Col<B1, 32>,
    src_val_packed: Col<B32>,
    imm_packed: Col<B16>,
    u32_add: U32Add,
}

impl AddiTable {
    pub fn new(
        cs: &mut ConstraintSystem,
        state_channel: ChannelId,
        prom_channel: ChannelId,
        vrom_channel: ChannelId,
    ) -> Self {
        let mut table = cs.add_table("addi");

        let cpu_cols = CpuColumns::new::<{ Opcode::Addi as u16 }>(
            &mut table,
            state_channel,
            prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        // TODO: We need a U32AddU16 gadget or otherwise we will be wasting cols with
        // only 0s

        // let src_val = table.add_committed("src1_val");
        // let src_val_packed = table.add_packed("src_val_packed", src_val);
        // let imm_unpacked = cpu_cols.arg2_unpacked;
        // let imm_packed = table.add_packed("imm_packed", imm_unpacked);

        unimplemented!()
        // let u32_add = U32Add::new(&mut table, src_val,
        // upcast_col(imm_unpacked), U32AddFlags::default());

        // Self {
        //     id: table.id(),
        //     cpu_cols,
        //     src_val,
        //     src_val_packed,
        //     imm_packed,
        //     u32_add,
        // }
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
            for (i, event) in rows.enumerate() {
                self.cpu_cols.fill_row(
                    witness,
                    CpuRow {
                        index: i,
                        pc: event.pc.into(),
                        next_pc: None,
                        fp: event.fp,
                        instruction: Instruction {
                            opcode: Opcode::Addi,
                            arg0: event.dst,
                            arg1: event.src,
                            arg2: event.imm,
                        },
                    },
                );
                let mut src1_val = witness.get_mut_as(self.src_val)?;
                // let mut imm = witness.get_mut_as(self.imm)?;
                src1_val[i] = event.src_val;
                // imm[i] = event.imm;
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
//             [$pc, upcast_col(fp), upcast_col(timestamp)],
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

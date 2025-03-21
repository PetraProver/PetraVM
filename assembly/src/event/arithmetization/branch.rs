use core::time;

use binius_core::constraint_system::channel::ChannelId;
use binius_field::{
    as_packed_field::PackScalar, underlier::UnderlierType, BinaryField16b, BinaryField32b,
    ExtensionField, Field,
};
use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessIndexSegment, B1, B32,
};
use bytemuck::Pod;
use env_logger::fmt::Timestamp;

use super::cpu::{CpuColumns, CpuColumnsOptions, CpuRow, Instruction, NextPc};
use crate::opcodes::Opcode;

/// Table for BNZ.
///
/// Performs a branching to the target address if the argument is not zero.
///
/// Logic:
///   1. if FP[cond] <> 0, then PC = target
///   2. if FP[cond] == 0, then increment PC
pub(crate) struct BnzTable {
    id: TableId,
    cpu_cols: CpuColumns,
    cond_val: Col<B32>, // Constant
}

impl BnzTable {
    pub fn new(
        cs: &mut ConstraintSystem,
        state_channel: ChannelId,
        vrom_channel: ChannelId,
        prom_channel: ChannelId,
    ) -> Self {
        let mut table = cs.add_table("bnz");
        let cond_val = table.add_committed("cond_val");

        // TODO: Assert cond_val is != 0

        let cpu_cols = CpuColumns::new::<{ Opcode::Bnz as u32 }>(
            &mut table,
            state_channel,
            prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Immediate,
                next_fp: None,
            },
        );

        let cond = cpu_cols.arg0;
        let timestamp = cpu_cols.timestamp;

        // Read cond_val
        table.push(vrom_channel, [timestamp, upcast_col(cond), cond_val]);

        Self {
            id: table.id(),
            cpu_cols,
            cond_val,
        }
    }
}

impl<U: UnderlierType> TableFiller<U> for BnzTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = crate::event::model::branch::BnzEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessIndexSegment<U>,
    ) -> Result<(), anyhow::Error> {
        for (i, event) in rows.enumerate() {
            let row = CpuRow {
                index: i,
                pc: event.pc.val(),
                next_pc: None,
                fp: event.fp,
                timestamp: event.timestamp,
                instruction: Instruction {
                    opcode: Opcode::Bnz,
                    arg0: event.cond,
                    arg1: event.target_low.val(),
                    arg2: event.target_high.val(),
                },
            };
            self.cpu_cols.fill_row(witness, row)?;
            let mut cond_val = witness.get_mut_as(self.cond_val)?;
            cond_val[i] = event.cond_val;
        }
        Ok(())
    }
}

struct BzTable {
    id: TableId,
    cpu_cols: CpuColumns,
}

impl BzTable {
    pub fn new(
        cs: &mut ConstraintSystem,
        state_channel: ChannelId,
        vrom_channel: ChannelId,
        prom_channel: ChannelId,
    ) -> Self {
        let mut table = cs.add_table("bz");

        let cpu_cols = CpuColumns::new::<{ Opcode::Bnz as u32 }>(
            &mut table,
            state_channel,
            prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let cond = cpu_cols.arg0;
        let timestamp = cpu_cols.timestamp;
        // TODO: We should have a single zero?
        let zero = table.add_constant("zero", [B32::ZERO]);

        // cond_val must be zero
        table.push(prom_channel, [timestamp, upcast_col(cond), zero]);

        Self {
            id: table.id(),
            cpu_cols,
        }
    }
}

impl<U: UnderlierType> TableFiller<U> for BzTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = crate::event::model::branch::BzEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessIndexSegment<U>,
    ) -> Result<(), anyhow::Error> {
        for (i, event) in rows.enumerate() {
            let row = CpuRow {
                index: i,
                pc: event.pc.val(),
                next_pc: None,
                fp: event.fp,
                timestamp: event.timestamp,
                instruction: Instruction {
                    opcode: Opcode::Bnz,
                    arg0: 0,
                    arg1: 0,
                    arg2: 0,
                },
            };
            self.cpu_cols.fill_row(witness, row)?;
        }
        Ok(())
    }
}

use binius_core::constraint_system::channel::ChannelId;
use binius_field::{as_packed_field::PackScalar, BinaryField};
use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableBuilder, TableWitnessIndexSegment, B1, B16, B32,
};
use bytemuck::Pod;

use crate::opcodes::Opcode;

/// A gadget for reading the instruction from the prom and
/// setting the next program counter
pub(crate) struct CpuColumns {
    pub(crate) pc: Col<B32>,
    pub(crate) next_pc: Col<B32>, // Virtual
    pub(crate) fp: Col<B32>,
    pub(crate) timestamp: Col<B32>,
    next_timestamp: Col<B32>, // Virtual?
    opcode: Col<B16>,         // Should be virtual
    pub(crate) arg0: Col<B16>,
    pub(crate) arg1: Col<B16>,
    pub(crate) arg2: Col<B16>,
}

pub(crate) struct CpuColumnsOptions {
    pub(crate) jumps: Option<Col<B32>>,
    pub(crate) next_fp: Option<Col<B32>>,
    pub(crate) opcode: Opcode,
    // TODO: Maybe add options for reading/writng from/to to the args
}

pub(crate) struct CpuRow {
    pub(crate) index: usize,
    pub(crate) pc: u32,
    pub(crate) fp: u32,
    pub(crate) timestamp: u32,
    pub(crate) instruction: Instruction,
}

pub(crate) struct Instruction {
    pub(crate) opcode: Opcode,
    pub(crate) arg0: u16,
    pub(crate) arg1: u16,
    pub(crate) arg2: u16,
}

impl CpuColumns {
    pub fn new(
        table: &mut TableBuilder,
        state_channel: ChannelId,
        prom_channel: ChannelId,
        options: CpuColumnsOptions,
    ) -> Self {
        let pc = table.add_committed("pc");
        let fp = table.add_committed("fp");
        let timestamp = table.add_committed("timestamp");
        let opcode = table.add_committed("opcode"); //TODO: opcode is a constant
        let arg0 = table.add_committed("arg0");
        let arg1 = table.add_committed("arg1");
        let arg2 = table.add_committed("arg2");
        // let a = table.add_linear_combination("opcode", B16::new(Opcode::Add as u16));
        // TODO: Whye opcode - options.opcode doesn't work?
        table.assert_zero(
            "opcode_is_correct",
            opcode - B16::new(options.opcode as u16),
        );

        // TODO: Next timestamp should be either timestamp + 1 or timestamp*G.
        // For now it's unconstrained.
        let next_timestamp = table.add_committed("next_timestamp");

        let next_pc;
        if let Some(target) = options.jumps {
            // TODO: Add jumps
            next_pc = target;
        } else {
            next_pc = table
                .add_computed("next_pc", (pc * B32::MULTIPLICATIVE_GENERATOR).into());
        }

        // Read instruction
        table.push(
            prom_channel,
            [
                pc,
                upcast_col(opcode),
                upcast_col(arg0),
                upcast_col(arg1),
                upcast_col(arg2),
            ],
        );

        // Flushing rules for the state channel
        table.push(
            state_channel,
            [pc, fp, timestamp],
        );
        if let Some(next_fp) = options.next_fp {
            table.pull(
                state_channel,
                [
                    next_pc,
                    next_fp,
                    next_timestamp,
                ],
            );
        } else {
            table.pull(
                state_channel,
                [
                    next_pc,
                    fp,
                    next_timestamp,
                ],
            );
        }
        Self {
            pc,
            next_pc,
            fp,
            timestamp,
            next_timestamp,
            opcode,
            arg0,
            arg1,
            arg2,
        }
    }

    pub fn fill_row<U>(
        &self,
        index: &mut TableWitnessIndexSegment<U>,
        row: CpuRow,
    ) -> Result<(), anyhow::Error>
    where
        U: Pod + PackScalar<B1>,
    {
        let mut pc_col = index.get_mut_as(self.pc)?;
        let mut fp_col = index.get_mut_as(self.fp)?;
        let mut timestamp_col = index.get_mut_as(self.timestamp)?;
        let mut next_pc_col = index.get_mut_as(self.next_pc)?;
        let mut next_timestamp_col = index.get_mut_as(self.next_timestamp)?;
        let mut opcode_col = index.get_mut_as(self.opcode)?;

        let mut arg0_col = index.get_mut_as(self.arg0)?;
        let mut arg1_col = index.get_mut_as(self.arg1)?;
        let mut arg2_col = index.get_mut_as(self.arg2)?;

        let CpuRow {
            index,
            pc,
            fp,
            timestamp,
            instruction:
                Instruction {
                    opcode,
                    arg0,
                    arg1,
                    arg2,
                },
        } = row;
        pc_col[index] = pc;
        fp_col[index] = fp;
        timestamp_col[index] = timestamp;
        opcode_col[index] = opcode as u16;
        arg0_col[index] = arg0;
        arg1_col[index] = arg1;
        arg2_col[index] = arg2;

        next_pc_col[index] = (B32::new(pc) * B32::MULTIPLICATIVE_GENERATOR).val();
        next_timestamp_col[index] = timestamp + 1u32;

        Ok(())
    }
}

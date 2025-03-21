use std::cell::RefMut;

use binius_core::{constraint_system::channel::ChannelId, oracle::ShiftVariant};
use binius_field::{as_packed_field::PackScalar, BinaryField};
use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableBuilder, TableWitnessIndexSegment, B1, B16, B32,
};
use bytemuck::Pod;

use crate::opcodes::Opcode;

/// A gadget for reading the instruction from the prom and
/// setting the next program counter and timestamp
pub(crate) struct CpuColumns {
    pub(crate) pc: Col<B32>,
    pub(crate) next_pc: Col<B32>, // Virtual
    pub(crate) fp: Col<B32>,
    pub(crate) timestamp: Col<B32>,
    pub(crate) next_timestamp: Col<B32>, // Virtual?
    pub(crate) opcode: Col<B32>,         // Constant
    pub(crate) arg0: Col<B16>,
    pub(crate) args12_unpacked: Col<B1, 32>,
    pub(crate) args12: Col<B16, 2>, // Virtual
    pub(crate) arg1: Col<B16>,      // Virtual
    pub(crate) arg2: Col<B16>,      // Virtual
    options: CpuColumnsOptions,
}

pub(crate) enum NextPc {
    /// Next pc is the current pc * G.
    Increment,
    /// Next pc is the value defined by target.
    Target(Col<B32>),
    /// Next pc is the value defined by arg1, arg2.
    Immediate,
}

pub(crate) struct CpuColumnsOptions {
    pub(crate) next_pc: NextPc,
    pub(crate) next_fp: Option<Col<B32>>,
    // TODO: Maybe add options for reading/writng from/to to the args
}

pub(crate) struct CpuRow {
    pub(crate) index: usize,
    pub(crate) pc: u32,
    // TODO: This is only necessary for ret because when filling it can't be read from target in
    // NextPc::Target(target)
    pub(crate) next_pc: Option<u32>,
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
    pub fn new<const OPCODE: u32>(
        table: &mut TableBuilder,
        state_channel: ChannelId,
        prom_channel: ChannelId,
        options: CpuColumnsOptions,
    ) -> Self {
        let pc = table.add_committed("pc");
        let fp = table.add_committed("fp");
        let timestamp = table.add_committed("timestamp");
        let opcode = table.add_constant("opcode", [B32::new(OPCODE)]); //add_committed("opcode"); //TODO: opcode is a constant
        let arg0 = table.add_committed("arg0");
        let args12_unpacked = table.add_committed("args12_unpacked");
        let args12 = table.add_packed("args12", args12_unpacked);
        let arg1 = table.add_selected("arg1", args12, 0);
        let arg2 = table.add_selected("arg2", args12, 1);

        // TODO: Next timestamp should be either timestamp + 1 or timestamp*G.
        // For now it's unconstrained.
        let next_timestamp = table.add_committed("next_timestamp");

        let next_pc;
        match options.next_pc {
            NextPc::Increment => {
                next_pc =
                    table.add_computed("next_pc", (pc * B32::MULTIPLICATIVE_GENERATOR).into());
            }
            NextPc::Target(target) => {
                next_pc = target;
            }
            NextPc::Immediate => {
                next_pc = table.add_packed("next_pc", args12_unpacked);
            }
        };

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
        table.pull(state_channel, [pc, fp, timestamp]);
        if let Some(next_fp) = options.next_fp {
            table.push(state_channel, [next_pc, next_fp, next_timestamp]);
        } else {
            table.push(state_channel, [next_pc, fp, next_timestamp]);
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
            args12_unpacked,
            args12,
            arg2,
            options,
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
            index: i,
            pc,
            next_pc,
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
        pc_col[i] = pc;
        fp_col[i] = fp;
        timestamp_col[i] = timestamp;
        opcode_col[i] = opcode as u16;
        arg0_col[i] = arg0;
        arg1_col[i] = arg1;
        arg2_col[i] = arg2;

        // println!("next_pc = {:?}", next_pc);
        next_pc_col[i] = match self.options.next_pc {
            NextPc::Increment => (B32::new(pc) * B32::MULTIPLICATIVE_GENERATOR).val(),
            NextPc::Target(target) => next_pc.expect("next_pc must be Some when NextPc::Target"),
            NextPc::Immediate => (arg1 as u32) << 16 | arg2 as u32,
        };
        next_timestamp_col[i] = timestamp + 1u32;
        println!("next_pc = {:?}", next_pc_col[i]);
        println!("next_timestamp = {:?}", next_timestamp_col[i]);

        Ok(())
    }
}

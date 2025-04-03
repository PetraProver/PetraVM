use binius_core::constraint_system::channel::ChannelId;
use binius_field::{as_packed_field::PackScalar, BinaryField};
use binius_m3::builder::{Col, TableBuilder, TableWitnessSegment, B1, B128, B16, B32};
use bytemuck::Pod;

use crate::utils::pack_b16_into_b32;
use crate::utils::{pack_instruction, pack_instruction_u128};

/// A gadget for reading the instruction from the prom and
/// setting the next program counter
pub(crate) struct CpuColumns<const OPCODE: u16> {
    pub(crate) pc: Col<B32>,
    pub(crate) next_pc: Col<B32>, // Virtual
    pub(crate) fp: Col<B32>,
    pub(crate) opcode: Col<B16>, // Constant
    pub(crate) arg0: Col<B16>,
    pub(crate) arg1: Col<B16>,
    // This field will be used for opcodes like SRLI
    #[allow(dead_code)]
    pub(crate) arg2_unpacked: Col<B1, 16>,
    pub(crate) arg2: Col<B16>, // Virtual,
    options: CpuColumnsOptions,
    // Virtual columns for communication with the channels
    prom_pull: Col<B128>, // Virtual
}

#[derive(Default)]
pub(crate) enum NextPc {
    /// `next_pc` is `current_pc * G`.
    #[default]
    Increment,
    /// Next pc is the value defined by target.
    Target(Col<B32>),
    /// Next pc is the value defined by arg1, arg2.
    #[allow(dead_code)]
    Immediate, // This will be necessary for opcodes like BNZ
}

#[derive(Default)]
pub(crate) struct CpuColumnsOptions {
    pub(crate) next_pc: NextPc,
    pub(crate) next_fp: Option<Col<B32>>,
}

#[derive(Default)]
pub(crate) struct CpuEvent {
    pub(crate) pc: u32,
    pub(crate) next_pc: Option<u32>,
    pub(crate) fp: u32,
    pub(crate) arg0: u16,
    pub(crate) arg1: u16,
    pub(crate) arg2: u16,
}

impl<const OPCODE: u16> CpuColumns<OPCODE> {
    pub fn new(
        table: &mut TableBuilder,
        state_channel: ChannelId,
        prom_channel: ChannelId,
        options: CpuColumnsOptions,
    ) -> Self {
        let pc = table.add_committed("pc");
        let fp = table.add_committed("fp");
        let opcode = table.add_constant(format!("opcode_{OPCODE}"), [B16::new(OPCODE)]);
        let arg0 = table.add_committed("arg0");
        let arg1 = table.add_committed("arg1");
        let arg2_unpacked = table.add_committed("arg2"); // This will be necessary for opcodes like SRLI
        let arg2 = table.add_packed("arg2", arg2_unpacked);

        let next_pc = match options.next_pc {
            NextPc::Increment => table.add_computed("next_pc", pc * B32::MULTIPLICATIVE_GENERATOR),
            NextPc::Target(target) => target,
            NextPc::Immediate => {
                table.add_computed("next_pc", pack_b16_into_b32([arg1.into(), arg2.into()]))
            }
        };

        // Pull the current pc and instruction to the prom channel
        let prom_pull = pack_instruction(table, "prom_pull", pc, opcode, [arg0, arg1, arg2]);
        table.pull(prom_channel, [prom_pull]);

        // Pull/Push the current/next pc and fp from from/to the state channel
        table.pull(state_channel, [pc, fp]);
        if let Some(next_fp) = options.next_fp {
            table.push(state_channel, [next_pc, next_fp]);
        } else {
            table.push(state_channel, [next_pc, fp]);
        }
        Self {
            pc,
            next_pc,
            fp,
            opcode,
            arg0,
            arg1,
            arg2_unpacked,
            arg2,
            options,
            prom_pull,
        }
    }

    pub fn populate<U, T>(
        &self,
        index: &mut TableWitnessSegment<U>,
        rows: T,
    ) -> Result<(), anyhow::Error>
    where
        U: Pod + PackScalar<B1>,
        T: Iterator<Item = CpuEvent>,
    {
        let mut pc_col = index.get_mut_as(self.pc)?;
        let mut fp_col = index.get_mut_as(self.fp)?;
        let mut next_pc_col = index.get_mut_as(self.next_pc)?;

        let mut opcode_col = index.get_mut_as(self.opcode)?;

        let mut arg0_col = index.get_mut_as(self.arg0)?;
        let mut arg1_col = index.get_mut_as(self.arg1)?;
        let mut arg2_col = index.get_mut_as(self.arg2)?;

        let mut prom_pull = index.get_mut_as(self.prom_pull)?;

        for (
            i,
            CpuEvent {
                pc,
                next_pc,
                fp,
                arg0,
                arg1,
                arg2,
            },
        ) in rows.enumerate()
        {
            pc_col[i] = pc;
            fp_col[i] = fp;
            opcode_col[i] = OPCODE;
            arg0_col[i] = arg0;
            arg1_col[i] = arg1;
            arg2_col[i] = arg2;

            next_pc_col[i] = match self.options.next_pc {
                NextPc::Increment => (B32::new(pc) * B32::MULTIPLICATIVE_GENERATOR).val(),
                NextPc::Target(_) => next_pc.expect("next_pc must be Some when NextPc::Target"),
                NextPc::Immediate => arg1 as u32 | (arg2 as u32) << 16,
            };

            prom_pull[i] = pack_instruction_u128(pc, OPCODE, arg0, arg1, arg2);
        }

        Ok(())
    }
}

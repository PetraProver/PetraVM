use binius_core::constraint_system::channel::ChannelId;
use binius_field::BinaryField;
use binius_m3::builder::{Col, TableBuilder, TableWitnessSegment, B1, B128, B16, B32};

use crate::{
    types::ProverPackedField,
    utils::{pack_b16_into_b32, pack_instruction_u128, pack_instruction_with_fixed_opcode},
};

/// A gadget for reading an instruction and its operands from the PROM and
/// setting the next program counter.
#[derive(Default)]
pub(crate) struct CpuGadget {
    /// Current program counter
    pub(crate) pc: u32,
    /// Next program counter
    pub(crate) next_pc: Option<u32>,
    /// Current frame pointer
    pub(crate) fp: u32,
    /// First 16-bit operand
    pub(crate) arg0: u16,
    /// Second 16-bit operand
    pub(crate) arg1: u16,
    /// Third 16-bit operand
    pub(crate) arg2: u16,
}

/// Column view for instruction operands.
/// The underlying type is a 16-bit field element.
type OpcodeArg = Col<B16>;
/// Column view for instruction operands in unpacked form,
/// consisting of 16 binary columns.
type OpcodeArgUnpacked = Col<B1, 16>;

/// The columns associated with the CPU gadget.
pub(crate) struct CpuColumns<const OPCODE: u16> {
    pub(crate) pc: Col<B32>,
    // TODO: next pc can be set to anything, so shouldn't be virtual?
    pub(crate) next_pc: Col<B32>, // Virtual
    pub(crate) fp: Col<B32>,
    pub(crate) arg0: OpcodeArg,
    pub(crate) arg1: OpcodeArg,
    // This field will be used for opcodes like SRLI
    pub(crate) arg2_unpacked: OpcodeArgUnpacked,
    pub(crate) arg2: OpcodeArg, // Virtual,

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
    Immediate, // This will be necessary for opcodes like BNZ
}

#[derive(Default)]
pub(crate) struct CpuColumnsOptions {
    pub(crate) next_pc: NextPc,
    pub(crate) next_fp: Option<Col<B32>>,
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
        let arg0 = table.add_committed("arg0");
        let arg1 = table.add_committed("arg1");
        let arg2_unpacked = table.add_committed("arg2"); // This will be necessary for opcodes like SRLI
        let arg2 = table.add_packed("arg2", arg2_unpacked);

        // Pull the current pc and instruction to the prom channel
        let prom_pull =
            pack_instruction_with_fixed_opcode(table, "prom_pull", pc, OPCODE, [arg0, arg1, arg2]);
        table.pull(prom_channel, [prom_pull]);

        // Pull/Push the current/next pc and fp from from/to the state channel
        let next_pc = match options.next_pc {
            NextPc::Increment => table.add_computed("next_pc", pc * B32::MULTIPLICATIVE_GENERATOR),
            NextPc::Target(target) => target,
            NextPc::Immediate => {
                table.add_computed("next_pc", pack_b16_into_b32([arg0.into(), arg1.into()]))
            }
        };
        let next_fp = options.next_fp.unwrap_or(fp);
        table.pull(state_channel, [pc, fp]);
        table.push(state_channel, [next_pc, next_fp]);

        Self {
            pc,
            next_pc,
            fp,
            arg0,
            arg1,
            arg2_unpacked,
            arg2,
            options,
            prom_pull,
        }
    }

    pub fn populate<T>(
        &self,
        index: &mut TableWitnessSegment<ProverPackedField>,
        rows: T,
    ) -> Result<(), anyhow::Error>
    where
        T: Iterator<Item = CpuGadget>,
    {
        // TODO: Replace with `get_scalars_mut`?
        let mut pc_col = index.get_mut_as(self.pc)?;
        let mut fp_col = index.get_mut_as(self.fp)?;
        let mut next_pc_col = index.get_mut_as(self.next_pc)?;

        let mut arg0_col = index.get_mut_as(self.arg0)?;
        let mut arg1_col = index.get_mut_as(self.arg1)?;
        let mut arg2_col = index.get_mut_as(self.arg2)?;

        let mut prom_pull = index.get_mut_as(self.prom_pull)?;

        for (
            i,
            CpuGadget {
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
            arg0_col[i] = arg0;
            arg1_col[i] = arg1;
            arg2_col[i] = arg2;

            next_pc_col[i] = match self.options.next_pc {
                NextPc::Increment => (B32::new(pc) * B32::MULTIPLICATIVE_GENERATOR).val(),
                NextPc::Target(_) => next_pc.expect("next_pc must be Some when NextPc::Target"),
                NextPc::Immediate => arg0 as u32 | (arg1 as u32) << 16,
            };

            prom_pull[i] = pack_instruction_u128(pc, OPCODE, arg0, arg1, arg2);
        }

        Ok(())
    }
}

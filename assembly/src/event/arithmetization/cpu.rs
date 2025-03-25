use std::cell::RefMut;

use binius_core::{constraint_system::channel::ChannelId, oracle::ShiftVariant};
use binius_field::{as_packed_field::PackScalar, BinaryField, ExtensionField};
use binius_m3::builder::{
    upcast_col, upcast_expr, Col, ConstraintSystem, Expr, TableBuilder, TableWitnessIndexSegment,
    B1, B128, B16, B32, B64,
};
use bytemuck::Pod;

use crate::opcodes::Opcode;

/// A gadget for reading the instruction from the prom and
/// setting the next program counter and timestamp
pub(crate) struct CpuColumns {
    pub(crate) pc: Col<B32>,
    pub(crate) next_pc: Col<B32>, // Virtual
    pub(crate) fp: Col<B32>,
    pub(crate) opcode: Col<B16>, // Constant
    pub(crate) arg0: Col<B16>,
    pub(crate) arg1: Col<B16>,
    pub(crate) arg2: Col<B16>,
    options: CpuColumnsOptions,
    // Virtual columns for communication with the channels
    prom_push: Col<B128>,
    state_push: Col<B64>,
    state_pull: Col<B64>,
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
    pub(crate) pc: u32,
    // TODO: This is only necessary for ret because when filling it can't be read from target in
    // NextPc::Target(target)
    pub(crate) next_pc: Option<u32>,
    pub(crate) fp: u32,
    pub(crate) next_fp: Option<u32>,
    pub(crate) instruction: Instruction,
}

#[derive(Default)]
pub(crate) struct Instruction {
    pub(crate) opcode: Opcode,
    pub(crate) arg0: u16,
    pub(crate) arg1: u16,
    pub(crate) arg2: u16,
}

impl CpuColumns {
    pub fn new<const OPCODE: u16>(
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
        let arg2 = table.add_committed("arg2");

        // Pakc pc and instruction into a single value. We should eventually import this
        // from some utility module.
        let b128_basis: [_; 2] = std::array::from_fn(|i| {
            <B128 as ExtensionField<B64>>::basis(i).expect("i in range 0..2; extension degree is 2")
        });
        let b64_16basis: [_; 4] = std::array::from_fn(|i| {
            <B64 as ExtensionField<B16>>::basis(i).expect("i in range 0..4; extension degree is 4")
        });
        let b64_32basis: [_; 2] = std::array::from_fn(|i| {
            <B64 as ExtensionField<B32>>::basis(i).expect("i in range 0..2; extension degree is 2")
        });
        let b32_basis: [_; 2] = std::array::from_fn(|i| {
            <B32 as ExtensionField<B16>>::basis(i).expect("i in range 0..2; extension degree is 2")
        });
        let pack_b16_into_b32 = move |limbs: [Expr<B16, 1>; 2]| {
            limbs
                .into_iter()
                .enumerate()
                .map(|(i, limb)| upcast_expr(limb) * b32_basis[i])
                .reduce(|a, b| a + b)
                .expect("limbs has length 2")
        };
        let pack_b32_into_b64 = move |limbs: [Expr<B32, 1>; 2]| {
            limbs
                .into_iter()
                .enumerate()
                .map(|(i, limb)| upcast_expr(limb) * b64_32basis[i])
                .reduce(|a, b| a + b)
                .expect("limbs has length 2")
        };

        let next_pc = match options.next_pc {
            NextPc::Increment => {
                table.add_computed("next_pc", (pc * B32::MULTIPLICATIVE_GENERATOR).into())
            }
            NextPc::Target(target) => target,
            NextPc::Immediate => {
                table.add_computed("next_pc", pack_b16_into_b32([arg1.into(), arg2.into()]))
            }
        };

        let next_fp = match options.next_fp {
            Some(next_fp) => next_fp,
            None => fp.clone(),
        };

        let pc_instruction = move |pc: Expr<B32, 1>, instruction: [Expr<B16, 1>; 4]| {
            let pc = upcast_expr(pc);
            let instruction = instruction.into_iter().map(upcast_expr).collect::<Vec<_>>();
            let instruction_64 = instruction
                .into_iter()
                .enumerate()
                .map(|(i, limb)| limb * b64_16basis[i])
                .reduce(|a, b| a + b)
                .expect("instruction has length 4");
            pc * b128_basis[1] + upcast_expr(instruction_64) * b128_basis[0]
        };

        // Push the current pc and instruction to the prom channel
        let prom_push = table.add_computed(
            "prom_push",
            pc_instruction(
                pc.into(),
                [opcode.into(), arg0.into(), arg1.into(), arg2.into()],
            ),
        );
        table.push(prom_channel, [prom_push]);

        // Pull/Push the current/next pc and fp from from/to the state channel
        let state_pull =
            table.add_computed("state_pull", pack_b32_into_b64([fp.into(), pc.into()]));
        table.pull(state_channel, [state_pull]);
        let state_push;
        if let Some(next_fp) = options.next_fp {
            state_push = table.add_computed(
                "state_push",
                pack_b32_into_b64([next_fp.into(), next_pc.into()]),
            );
            table.push(state_channel, [state_push]);
        } else {
            state_push =
                table.add_computed("state_push", pack_b32_into_b64([fp.into(), next_pc.into()]));
            table.push(state_channel, [state_push]);
        }
        Self {
            pc,
            next_pc,
            fp,
            opcode,
            arg0,
            arg1,
            arg2,
            options,
            prom_push,
            state_push,
            state_pull,
        }
    }

    pub fn populate<U, T>(
        &self,
        index: &mut TableWitnessIndexSegment<U>,
        rows: T,
    ) -> Result<(), anyhow::Error>
    where
        U: Pod + PackScalar<B1>,
        T: Iterator<Item = CpuRow>,
    {
        let mut pc_col = index.get_mut_as(self.pc)?;
        let mut fp_col = index.get_mut_as(self.fp)?;
        let mut next_pc_col = index.get_mut_as(self.next_pc)?;

        let mut opcode_col = index.get_mut_as(self.opcode)?;

        let mut arg0_col = index.get_mut_as(self.arg0)?;
        let mut arg1_col = index.get_mut_as(self.arg1)?;
        let mut arg2_col = index.get_mut_as(self.arg2)?;

        let mut prom_push = index.get_mut_as(self.prom_push)?;
        let mut state_push = index.get_mut_as(self.state_push)?;
        let mut state_pull = index.get_mut_as(self.state_pull)?;

        for (
            i,
            CpuRow {
                pc,
                next_pc,
                fp,
                next_fp,
                instruction:
                    Instruction {
                        opcode,
                        arg0,
                        arg1,
                        arg2,
                    },
            },
        ) in rows.enumerate()
        {
            pc_col[i] = pc;
            fp_col[i] = fp;
            opcode_col[i] = opcode as u16;
            arg0_col[i] = arg0;
            arg1_col[i] = arg1;
            arg2_col[i] = arg2;

            next_pc_col[i] = match self.options.next_pc {
                NextPc::Increment => (B32::new(pc) * B32::MULTIPLICATIVE_GENERATOR).val(),
                NextPc::Target(target) => {
                    next_pc.expect("next_pc must be Some when NextPc::Target")
                }
                NextPc::Immediate => arg1 as u32 | (arg2 as u32) << 16,
            };

            prom_push[i] = (pc as u128) << 64
                | opcode as u128
                | (arg0 as u128) << 16
                | (arg1 as u128) << 32
                | (arg2 as u128) << 48;

            let next_fp = if let Some(next_fp) = next_fp {
                next_fp
            } else {
                fp
            };
            state_push[i] = (next_pc_col[i] as u64) << 32 | next_fp as u64;
            state_pull[i] = (pc as u64) << 32 | fp as u64;

            println!("next_pc = {:?}, next_fp = {:?}", next_pc, next_fp);

            println!(
                "pc = {:?}, opcode = {:?}, arg0 = {:?}, arg1 = {:?}, arg2 = {:?}",
                pc, opcode, arg0, arg1, arg2
            );
            println!("prom_push = {:#x}", prom_push[i]);
        }

        Ok(())
    }
}

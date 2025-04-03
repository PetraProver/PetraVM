//! Binary field operation tables for the zCrayVM M3 circuit.
//!
//! This module contains tables for binary field arithmetic operations.

use binius_field::BinaryField;
use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B128, B16, B32,
};
use zcrayvm_assembly::{opcodes::Opcode, B32MulEvent};

use crate::{
    channels::Channels,
    types::CommonTableBounds,
    utils::{pack_instruction_b128, pack_instruction_with_fixed_opcode},
};

// Constants for opcodes
const B32_MUL_OPCODE: u16 = Opcode::B32Mul as u16;

/// B32_MUL (Binary Field Multiplication) table.
///
/// This table handles the B32_MUL instruction, which performs multiplication
/// in the binary field GF(2^32).
pub struct B32MulTable {
    /// Table ID
    pub id: TableId,
    /// PC column
    pub pc: Col<B32>,
    /// Frame pointer column
    pub fp: Col<B32>,
    /// Destination offset
    pub dst: Col<B16>,
    /// First source offset
    pub src1: Col<B16>,
    /// Second source offset
    pub src2: Col<B16>,
    /// First source value
    pub src1_val: Col<B32>,
    /// Second source value
    pub src2_val: Col<B32>,
    /// Result value
    pub result_val: Col<B32>,
    /// PROM channel pull value
    pub prom_pull: Col<B128>,
    /// First source absolute address
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
    /// Next PC column
    pub next_pc: Col<B32>,
}

impl B32MulTable {
    /// Create a new B32_MUL table with the given constraint system and
    /// channels.
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("b32_mul");

        // Add columns for PC, FP, and instruction components
        let pc = table.add_committed("pc");
        let fp = table.add_committed("fp");
        let dst = table.add_committed("dst");
        let src1 = table.add_committed("src1");
        let src2 = table.add_committed("src2");
        let src1_val = table.add_committed("src1_val");
        let src2_val = table.add_committed("src2_val");
        let result_val = table.add_committed("result_val");

        // Pull from state channel (get current state)
        table.pull(channels.state_channel, [pc, fp]);

        // Pack instruction for PROM channel pull
        let prom_pull = pack_instruction_with_fixed_opcode(
            &mut table,
            "prom_pull",
            pc,
            B32_MUL_OPCODE,
            [dst, src1, src2],
        );

        // Pull from PROM channel
        table.pull(channels.prom_channel, [prom_pull]);

        // Pull source values from VROM channel
        let src1_abs_addr = table.add_computed("src1_addr", fp + upcast_expr(src1.into()));
        let src2_abs_addr = table.add_computed("src2_addr", fp + upcast_expr(src2.into()));
        table.pull(channels.vrom_channel, [src1_abs_addr, src1_val]);
        table.pull(channels.vrom_channel, [src2_abs_addr, src2_val]);

        // let expected_result = table.add_computed("result", src1_val * src2_val);
        // table.assert_zero("check_b32_mul_result", expected_result - result_val);

        // Push result to VROM channel
        let dst_abs_addr = table.add_computed("dst_addr", fp + upcast_expr(dst.into()));
        table.pull(channels.vrom_channel, [dst_abs_addr, result_val]);

        // Compute next PC
        let next_pc = table.add_computed("next_pc", pc * B32::MULTIPLICATIVE_GENERATOR);

        // Push to state channel
        table.push(channels.state_channel, [next_pc, fp]);

        Self {
            id: table.id(),
            pc,
            fp,
            dst,
            src1,
            src2,
            src1_val,
            src2_val,
            result_val,
            prom_pull,
            src1_abs_addr,
            src2_abs_addr,
            dst_abs_addr,
            next_pc,
        }
    }
}

impl<U> TableFiller<U> for B32MulTable
where
    U: CommonTableBounds,
{
    type Event = B32MulEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessSegment<U>,
    ) -> anyhow::Result<()> {
        let mut pc_col = witness.get_scalars_mut(self.pc)?;
        let mut fp_col = witness.get_scalars_mut(self.fp)?;
        let mut dst_col = witness.get_scalars_mut(self.dst)?;
        let mut src1_col = witness.get_scalars_mut(self.src1)?;
        let mut src2_col = witness.get_scalars_mut(self.src2)?;
        let mut src1_val_col = witness.get_scalars_mut(self.src1_val)?;
        let mut src2_val_col = witness.get_scalars_mut(self.src2_val)?;
        let mut result_val_col = witness.get_scalars_mut(self.result_val)?;
        let mut next_pc_col = witness.get_scalars_mut(self.next_pc)?;
        let mut prom_pull_col = witness.get_scalars_mut(self.prom_pull)?;
        let mut src1_abs_addr_col = witness.get_scalars_mut(self.src1_abs_addr)?;
        let mut src2_abs_addr_col = witness.get_scalars_mut(self.src2_abs_addr)?;
        let mut dst_abs_addr_col = witness.get_scalars_mut(self.dst_abs_addr)?;

        for (i, event) in rows.enumerate() {
            pc_col[i] = event.pc;
            fp_col[i] = B32::new(*event.fp);
            dst_col[i] = B16::new(event.dst);
            src1_col[i] = B16::new(event.src1);
            src2_col[i] = B16::new(event.src2);
            src1_val_col[i] = B32::new(event.src1_val);
            src2_val_col[i] = B32::new(event.src2_val);
            result_val_col[i] = B32::new(event.dst_val);
            src1_abs_addr_col[i] = B32::new(event.fp.addr(event.src1));
            src2_abs_addr_col[i] = B32::new(event.fp.addr(event.src2));
            dst_abs_addr_col[i] = B32::new(event.fp.addr(event.dst));

            next_pc_col[i] = pc_col[i] * B32::MULTIPLICATIVE_GENERATOR;

            prom_pull_col[i] = pack_instruction_b128(
                pc_col[i],
                B16::new(B32_MUL_OPCODE),
                dst_col[i],
                src1_col[i],
                src2_col[i],
            );
            dbg!(
                &pc_col[i],
                &fp_col[i],
                &dst_col[i],
                &src1_col[i],
                &src2_col[i],
                &src1_val_col[i],
                &src2_val_col[i],
                &result_val_col[i],
                &next_pc_col[i],
                &prom_pull_col[i],
                &src1_abs_addr_col[i],
                &src2_abs_addr_col[i],
                &dst_abs_addr_col[i],
            );
        }
        Ok(())
    }
}

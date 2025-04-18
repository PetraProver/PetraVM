//! Binary field operation tables for the zCrayVM M3 circuit.
//!
//! This module contains tables for binary field arithmetic operations.

use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B128, B32,
};
use zcrayvm_assembly::{opcodes::Opcode, B128AddEvent, B32MulEvent};

use crate::{
    channels::Channels,
    gadgets::{
        b128_lookup::{B128LookupColumns, B128LookupGadget},
        cpu::{CpuColumns, CpuColumnsOptions, CpuGadget, NextPc},
    },
    types::ProverPackedField,
};

// Constants for opcodes
const B32_MUL_OPCODE: u16 = Opcode::B32Mul as u16;
const B128_ADD_OPCODE: u16 = Opcode::B128Add as u16;

/// B32_MUL (Binary Field Multiplication) table.
///
/// This table handles the B32_MUL instruction, which performs multiplication
/// in the binary field GF(2^32).
pub struct B32MulTable {
    /// Table ID
    pub id: TableId,
    /// CPU columns
    cpu_cols: CpuColumns<{ B32_MUL_OPCODE }>,
    /// First source value
    pub src1_val: Col<B32>,
    /// Second source value
    pub src2_val: Col<B32>,
    /// Result value
    pub result_val: Col<B32>,
    /// PROM channel pull value
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl B32MulTable {
    /// Create a new B32_MUL table with the given constraint system and
    /// channels.
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("b32_mul");

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let CpuColumns {
            fp,
            arg0: dst,
            arg1: src1,
            arg2: src2,
            ..
        } = cpu_cols;

        let src1_val = table.add_committed("b32_mul_src1_val");
        let src2_val = table.add_committed("b32_mul_src2_val");
        let result_val = table.add_committed("b32_mul_result_val");

        // Pull source values from VROM channel
        let src1_abs_addr = table.add_computed("src1_addr", fp + upcast_expr(src1.into()));
        let src2_abs_addr = table.add_computed("src2_addr", fp + upcast_expr(src2.into()));
        table.pull(channels.vrom_channel, [src1_abs_addr, src1_val]);
        table.pull(channels.vrom_channel, [src2_abs_addr, src2_val]);

        table.assert_zero("check_b32_mul_result", src1_val * src2_val - result_val);

        // Pull result from VROM channel
        let dst_abs_addr = table.add_computed("dst_addr", fp + upcast_expr(dst.into()));
        table.pull(channels.vrom_channel, [dst_abs_addr, result_val]);

        Self {
            id: table.id(),
            cpu_cols,
            src1_val,
            src2_val,
            result_val,
            src1_abs_addr,
            src2_abs_addr,
            dst_abs_addr,
        }
    }
}

impl TableFiller<ProverPackedField> for B32MulTable {
    type Event = B32MulEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut src1_val_col = witness.get_scalars_mut(self.src1_val)?;
            let mut src2_val_col = witness.get_scalars_mut(self.src2_val)?;
            let mut result_val_col = witness.get_scalars_mut(self.result_val)?;
            let mut src1_abs_addr_col = witness.get_scalars_mut(self.src1_abs_addr)?;
            let mut src2_abs_addr_col = witness.get_scalars_mut(self.src2_abs_addr)?;
            let mut dst_abs_addr_col = witness.get_scalars_mut(self.dst_abs_addr)?;

            for (i, event) in rows.clone().enumerate() {
                src1_val_col[i] = B32::new(event.src1_val);
                src2_val_col[i] = B32::new(event.src2_val);
                result_val_col[i] = B32::new(event.dst_val);
                src1_abs_addr_col[i] = B32::new(event.fp.addr(event.src1));
                src2_abs_addr_col[i] = B32::new(event.fp.addr(event.src2));
                dst_abs_addr_col[i] = B32::new(event.fp.addr(event.dst));
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}

/// B128_ADD (Binary Field Addition) table.
///
/// This table handles the B128_ADD instruction, which performs addition
/// in the binary field GF(2^32).
pub struct B128AddTable {
    /// Table ID
    pub id: TableId,
    /// CPU columns
    cpu_cols: CpuColumns<{ B128_ADD_OPCODE }>,
    /// First source value
    pub src1_val: Col<B128>,
    pub src1_val_unpacked: Col<B32, 4>,
    /// Lookup for first source
    src1_lookup: B128LookupColumns,
    /// Second source value
    pub src2_val: Col<B128>,
    pub src2_val_unpacked: Col<B32, 4>,
    /// Lookup for second source
    src2_lookup: B128LookupColumns,
    /// Result value
    pub result_val: Col<B128>,
    pub result_val_unpacked: Col<B32, 4>,
    /// Lookup for result
    result_lookup: B128LookupColumns,
    /// First source absolute address
    pub src1_abs_addr: Col<B32>,
    /// Second source absolute address
    pub src2_abs_addr: Col<B32>,
    /// Destination absolute address
    pub dst_abs_addr: Col<B32>,
}

impl B128AddTable {
    /// Create a new B32_MUL table with the given constraint system and
    /// channels.
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("b128_add");

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let CpuColumns {
            fp,
            arg0: dst,
            arg1: src1,
            arg2: src2,
            ..
        } = cpu_cols;

        let src1_val_unpacked = table.add_committed("b128_add_src1_val_unpacked");
        let src1_val = table.add_packed("b128_add_src1_val", src1_val_unpacked);
        let src2_val_unpacked = table.add_committed("b128_add_src2_val_unpacked");
        let src2_val = table.add_packed("b128_add_src2_val", src2_val_unpacked);
        let result_val_unpacked = table.add_committed("b128_add_result_val_unpacked");
        let result_val = table.add_packed("b128_add_result_val", result_val_unpacked);

        // Pull source values from VROM channel
        let src1_abs_addr = table.add_computed("src1_addr", fp + upcast_expr(src1.into()));
        let src1_lookup = B128LookupColumns::new(
            &mut table,
            channels.vrom_channel,
            src1_abs_addr,
            src1_val_unpacked,
            "b128_add_src1",
        );
        let src2_abs_addr = table.add_computed("src2_addr", fp + upcast_expr(src2.into()));
        let src2_lookup = B128LookupColumns::new(
            &mut table,
            channels.vrom_channel,
            src2_abs_addr,
            src2_val_unpacked,
            "b128_add_src2",
        );
        table.assert_zero("check_b128_add_result", src1_val + src2_val - result_val);

        // Pull result from VROM channel
        let dst_abs_addr = table.add_computed("dst_addr", fp + upcast_expr(dst.into()));
        let result_lookup = B128LookupColumns::new(
            &mut table,
            channels.vrom_channel,
            dst_abs_addr,
            result_val_unpacked,
            "b128_add_dst",
        );

        Self {
            id: table.id(),
            cpu_cols,
            src1_val,
            src1_val_unpacked,
            src1_lookup,
            src2_val,
            src2_val_unpacked,
            src2_lookup,
            result_val,
            result_val_unpacked,
            result_lookup,
            src1_abs_addr,
            src2_abs_addr,
            dst_abs_addr,
        }
    }
}

impl TableFiller<ProverPackedField> for B128AddTable {
    type Event = B128AddEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        let mut cpu_rows = Vec::new();
        let mut src1_rows = Vec::new();
        let mut src2_rows = Vec::new();
        let mut result_rows = Vec::new();

        {
            let mut src1_val_col_unpacked = witness.get_mut_as(self.src1_val_unpacked)?;
            let mut src2_val_col_unpacked = witness.get_mut_as(self.src2_val_unpacked)?;
            let mut result_val_col_unpacked = witness.get_mut_as(self.result_val_unpacked)?;
            let mut src1_abs_addr_col = witness.get_scalars_mut(self.src1_abs_addr)?;
            let mut src2_abs_addr_col = witness.get_scalars_mut(self.src2_abs_addr)?;
            let mut dst_abs_addr_col = witness.get_scalars_mut(self.dst_abs_addr)?;

            for (i, event) in rows.clone().enumerate() {
                src1_val_col_unpacked[i] = B128::new(event.src1_val);
                src2_val_col_unpacked[i] = B128::new(event.src2_val);
                result_val_col_unpacked[i] = B128::new(event.dst_val);
                src1_abs_addr_col[i] = B32::new(event.fp.addr(event.src1));
                src2_abs_addr_col[i] = B32::new(event.fp.addr(event.src2));
                dst_abs_addr_col[i] = B32::new(event.fp.addr(event.dst));

                let src1_row = B128LookupGadget {
                    addr: event.fp.addr(event.src1),
                    val: event.src1_val,
                };
                src1_rows.push(src1_row);
                let src2_row = B128LookupGadget {
                    addr: event.fp.addr(event.src2),
                    val: event.src2_val,
                };
                src2_rows.push(src2_row);
                let result_row = B128LookupGadget {
                    addr: event.fp.addr(event.dst),
                    val: event.dst_val,
                };
                result_rows.push(result_row);

                let cpu_row = CpuGadget {
                    pc: event.pc.val(),
                    next_pc: None,
                    fp: *event.fp,
                    arg0: event.dst,
                    arg1: event.src1,
                    arg2: event.src2,
                };
                cpu_rows.push(cpu_row);
            }
        }

        self.cpu_cols.populate(witness, cpu_rows.into_iter())?;
        self.src1_lookup.populate(witness, src1_rows.into_iter())?;
        self.src2_lookup.populate(witness, src2_rows.into_iter())?;
        self.result_lookup
            .populate(witness, result_rows.into_iter())
    }
}

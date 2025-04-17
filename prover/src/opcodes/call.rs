//! TAILI (Tail Call Immediate) table implementation for the zCrayVM M3 circuit.
//!
//! This module contains the TAILI table which handles tail calls to immediate
//! addresses in the zCrayVM execution.
use std::any::Any;

use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use zcrayvm_assembly::{opcodes::Opcode, TailiEvent};

use crate::gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget, NextPc};
use crate::table::Table;
use crate::{channels::Channels, types::ProverPackedField};

/// TAILI (Tail Call Immediate) table.
///
/// This table handles the Tail Call Immediate instruction, which performs a
/// tail call to a target address given by an immediate.
///
/// Logic:
/// 1. Load the current PC and FP from the state channel
/// 2. Get the instruction from PROM channel
/// 3. Verify this is a TAILI instruction
/// 4. Set up the next frame by preserving return address (FP[0]) and old frame
///    pointer (FP[1])
/// 5. Update PC to the target address
/// 6. Update FP to the next frame pointer
pub struct TailiTable {
    /// Table ID
    pub id: TableId,
    /// CPU columns
    cpu_cols: CpuColumns<{ Opcode::Taili as u16 }>,
    next_fp: Col<B32>,
    next_fp_abs_addr: Col<B32>,
    cur_fp_0_val: Col<B32>,
    cur_fp_1_val: Col<B32>,
    cur_fp_1: Col<B32>,
    next_fp_1: Col<B32>,
}

impl Table for TailiTable {
    type Event = TailiEvent;

    fn name(&self) -> &'static str {
        "TailiTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("taili");
        let next_fp = table.add_committed("next_fp");

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Immediate,
                next_fp: Some(next_fp),
            },
        );

        let CpuColumns {
            fp: cur_fp,
            arg2: next_fp_off,
            ..
        } = cpu_cols;

        // Compute the absolute address for the next frame pointer
        let next_fp_abs_addr =
            table.add_computed("next_fp_abs_addr", cur_fp + upcast_expr(next_fp_off.into()));

        // Pull next_fp from VROM
        table.pull(channels.vrom_channel, [next_fp_abs_addr, next_fp]);

        let cur_fp_0_val = table.add_committed("cur_fp_0_val");
        let cur_fp_1 = table.add_computed("fp_1", cur_fp + B32::new(1));
        let cur_fp_1_val = table.add_committed("cur_fp_1_val");
        table.pull(channels.vrom_channel, [cur_fp, cur_fp_0_val]);
        table.pull(channels.vrom_channel, [cur_fp_1, cur_fp_1_val]);

        let next_fp_1 = table.add_computed("next_fp_1", next_fp + B32::new(1));
        table.pull(channels.vrom_channel, [next_fp, cur_fp_0_val]);
        table.pull(channels.vrom_channel, [next_fp_1, cur_fp_1_val]);

        Self {
            id: table.id(),
            cpu_cols,
            next_fp,
            next_fp_abs_addr,
            cur_fp_0_val,
            cur_fp_1_val,
            cur_fp_1,
            next_fp_1,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for TailiTable {
    type Event = TailiEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut next_fp = witness.get_mut_as(self.next_fp)?;
            let mut next_fp_abs_addr = witness.get_mut_as(self.next_fp_abs_addr)?;
            let mut cur_fp_0_val = witness.get_mut_as(self.cur_fp_0_val)?;
            let mut cur_fp_1_val = witness.get_mut_as(self.cur_fp_1_val)?;
            let mut cur_fp_1 = witness.get_mut_as(self.cur_fp_1)?;
            let mut next_fp_1 = witness.get_mut_as(self.next_fp_1)?;

            for (i, event) in rows.clone().enumerate() {
                next_fp[i] = event.next_fp_val;
                next_fp_abs_addr[i] = event.fp.addr(event.next_fp);
                cur_fp_0_val[i] = event.return_addr;
                cur_fp_1[i] = event.fp.addr(1u32);
                cur_fp_1_val[i] = event.old_fp_val;
                next_fp_1[i] = event.next_fp_val + 1;
            }
        }

        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target),
            fp: *event.fp,
            arg0: (event.target & 0xFFFF) as u16, // target_low
            arg1: ((event.target >> 16) & 0xFFFF) as u16, // target_high
            arg2: event.next_fp,                  // next_fp
        });

        self.cpu_cols.populate(witness, cpu_rows)
    }
}

//! Function call instructions for the zCrayVM M3 circuit.

use std::any::Any;

use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use zcrayvm_assembly::{opcodes::Opcode, TailiEvent};

use crate::gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget, NextPc};
use crate::table::Table;
use crate::{channels::Channels, types::ProverPackedField};

/// TAILI (Tail Call Immediate) table implementation.
pub struct TailiTable {
    /// Table identifier
    pub id: TableId,
    /// CPU-related columns for instruction handling
    cpu_cols: CpuColumns<{ Opcode::Taili as u16 }>,
    /// New frame pointer value
    next_fp: Col<B32>,
    /// Absolute address of the next frame pointer slot (FP + next_fp_off)
    next_fp_abs_addr: Col<B32>,
    /// Value at current frame slot 0 (return address)
    cur_fp_0_val: Col<B32>,
    /// Value at current frame slot 1 (old frame pointer)
    cur_fp_1_val: Col<B32>,
    /// Address of current frame slot 1 (FP + 1)
    cur_fp_1: Col<B32>,
    /// Address of new frame slot 1 (next_fp + 1)
    next_fp_1: Col<B32>,
}

impl Table for TailiTable {
    type Event = TailiEvent;

    fn name(&self) -> &'static str {
        "TailiTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("taili");

        // Column for the new frame pointer value
        let next_fp = table.add_committed("next_fp");

        // Set up CPU columns with immediate PC update and new frame pointer
        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Immediate, // Jump directly to target address
                next_fp: Some(next_fp),     // Update frame pointer
            },
        );

        // Extract relevant instruction arguments
        let CpuColumns {
            fp: cur_fp,
            arg2: next_fp_off,
            ..
        } = cpu_cols;

        // Compute the absolute address for the next frame pointer
        let next_fp_abs_addr =
            table.add_computed("next_fp_abs_addr", cur_fp + upcast_expr(next_fp_off.into()));

        // Read the next frame pointer value from VROM
        table.pull(channels.vrom_channel, [next_fp_abs_addr, next_fp]);

        // Read current frame's return address and old frame pointer
        let cur_fp_0_val = table.add_committed("cur_fp_0_val"); // Return address at slot 0
        let cur_fp_1 = table.add_computed("fp_1", cur_fp + B32::new(1)); // Address of slot 1
        let cur_fp_1_val = table.add_committed("cur_fp_1_val"); // Old frame pointer at slot 1

        // Pull values from current frame
        table.pull(channels.vrom_channel, [cur_fp, cur_fp_0_val]);
        table.pull(channels.vrom_channel, [cur_fp_1, cur_fp_1_val]);

        // Compute address of slot 1 in new frame
        let next_fp_1 = table.add_computed("next_fp_1", next_fp + B32::new(1));

        // Verify that return address and old frame pointer are correctly copied to new
        // frame
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

    /// Fill the table witness with data from TAILI events
    ///
    /// This populates the witness data based on the execution events from
    /// the corresponding assembly TAILI operations.
    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            // Get mutable references to witness columns
            let mut next_fp = witness.get_mut_as(self.next_fp)?;
            let mut next_fp_abs_addr = witness.get_mut_as(self.next_fp_abs_addr)?;
            let mut cur_fp_0_val = witness.get_mut_as(self.cur_fp_0_val)?;
            let mut cur_fp_1_val = witness.get_mut_as(self.cur_fp_1_val)?;
            let mut cur_fp_1 = witness.get_mut_as(self.cur_fp_1)?;
            let mut next_fp_1 = witness.get_mut_as(self.next_fp_1)?;

            // Fill the witness columns with values from each event
            for (i, event) in rows.clone().enumerate() {
                next_fp[i] = event.next_fp_val;
                next_fp_abs_addr[i] = event.fp.addr(event.next_fp);
                cur_fp_0_val[i] = event.return_addr;
                cur_fp_1[i] = event.fp.addr(1u32);
                cur_fp_1_val[i] = event.old_fp_val;
                next_fp_1[i] = event.next_fp_val + 1;
            }
        }

        // Create CPU gadget rows from events
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target), // Jump to target address
            fp: *event.fp,
            arg0: (event.target & 0xFFFF) as u16, // target_low (lower 16 bits)
            arg1: ((event.target >> 16) & 0xFFFF) as u16, // target_high (upper 16 bits)
            arg2: event.next_fp,                  // next_fp address
        });

        // Populate CPU columns with the gadget rows
        self.cpu_cols.populate(witness, cpu_rows)
    }
}

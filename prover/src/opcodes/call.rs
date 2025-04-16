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
    next_fp_off_upcast: Col<B32>,
    next_fp_abs_addr: Col<B32>,
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
            fp,
            arg2: next_fp_off,
            ..
        } = cpu_cols;

        // Compute the absolute address for the next frame pointer
        let next_fp_off_upcast = table.add_computed("next_fp_off", upcast_expr(next_fp_off.into()));
        let next_fp_abs_addr = table.add_computed("next_fp_abs_addr", fp + next_fp_off_upcast);

        // Pull next_fp from VROM
        table.pull(channels.vrom_channel, [next_fp_abs_addr, next_fp]);

        Self {
            id: table.id(),
            cpu_cols,
            next_fp,
            next_fp_off_upcast,
            next_fp_abs_addr,
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
            let mut next_fp_off_upcast = witness.get_mut_as(self.next_fp_off_upcast)?;
            let mut next_fp_abs_addr = witness.get_mut_as(self.next_fp_abs_addr)?;
            
            for (i, event) in rows.clone().enumerate() {
                next_fp[i] = event.next_fp_val;
                next_fp_off_upcast[i] = event.next_fp as u32;
                next_fp_abs_addr[i] = event.fp.addr(event.next_fp);
            }
        }
        
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target),
            fp: *event.fp,
            arg0: (event.target & 0xFFFF) as u16,            // target_low
            arg1: ((event.target >> 16) & 0xFFFF) as u16,    // target_high
            arg2: event.next_fp,                            // next_fp
        });
        
        self.cpu_cols.populate(witness, cpu_rows)
    }
}

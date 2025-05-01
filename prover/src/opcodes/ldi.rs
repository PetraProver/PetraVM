//! LDI (Load Immediate) table implementation for the zCrayVM M3 circuit.
//!
//! This module contains the LDI table which handles loading immediate values
//! into VROM locations in the zCrayVM execution.

use std::any::Any;

use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use zcrayvm_assembly::{opcodes::Opcode, LdiEvent};

use crate::gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget};
use crate::{channels::Channels, table::Table, types::ProverPackedField, utils::pack_b16_into_b32};

/// LDI (Load Immediate) table.
///
/// This table handles the Load Immediate instruction, which loads a 32-bit
/// immediate value into a VROM location.
///
/// Logic:
/// 1. Load the current PC and FP from the state channel
/// 2. Get the instruction from PROM channel
/// 3. Verify this is an LDI instruction
/// 4. Compute the immediate value from the low and high parts
/// 5. Store the immediate value at FP + dst in VROM
/// 6. Update PC to move to the next instruction
pub struct LdiTable {
    /// Table ID
    pub id: TableId,
    /// State columns
    state_cols: StateColumns<{ Opcode::Ldi as u16 }>,
    vrom_abs_addr: Col<B32>, // Virtual
    imm: Col<B32>,           // Virtual
}

impl Table for LdiTable {
    type Event = LdiEvent;

    fn name(&self) -> &'static str {
        "LdiTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("ldi");

        let state_cols = StateColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let StateColumns {
            fp,
            arg0: dst,
            arg1: imm_low,
            arg2: imm_high,
            ..
        } = state_cols;

        let vrom_abs_addr = table.add_computed("abs_addr", fp + upcast_col(dst));

        // Pull value to VROM write table using absolute address
        let imm = table.add_computed("imm", pack_b16_into_b32(imm_low, imm_high));
        table.pull(channels.vrom_channel, [vrom_abs_addr, imm]);

        Self {
            id: table.id(),
            state_cols,
            vrom_abs_addr,
            imm,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for LdiTable {
    type Event = LdiEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut vrom_abs_addr = witness.get_scalars_mut(self.vrom_abs_addr)?;
            let mut imm = witness.get_scalars_mut(self.imm)?;
            for (i, event) in rows.clone().enumerate() {
                vrom_abs_addr[i] = B32::new(event.fp.addr(event.dst));
                imm[i] = B32::new(event.imm);
            }
        }
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.val(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.imm as u16,
            arg2: (event.imm >> 16) as u16,
        });
        self.state_cols.populate(witness, state_rows)
    }
}

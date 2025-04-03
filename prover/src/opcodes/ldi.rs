//! LDI (Load Immediate) table implementation for the zCrayVM M3 circuit.
//!
//! This module contains the LDI table which handles loading immediate values
//! into VROM locations in the zCrayVM execution.

use binius_field::BinaryField;
use binius_m3::builder::{
    upcast_col, upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment,
    B128, B16, B32,
};
use zcrayvm_assembly::{opcodes::Opcode, LDIEvent};

use super::cpu::{CpuColumns, CpuColumnsOptions, CpuEvent, NextPc};
use crate::{
    channels::Channels,
    types::CommonTableBounds,
    utils::{
        pack_b16_into_b32, pack_instruction_with_32bits_imm, pack_instruction_with_32bits_imm_b128,
    },
};

const LDI_OPCODE: u16 = Opcode::Ldi as u16;

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
    /// CPU columns
    cpu_cols: CpuColumns<{ Opcode::Ldi as u16 }>,
    abs_addr: Col<B32>, // Virtual
    imm: Col<B32>,      // Virtual
}

impl LdiTable {
    /// Create a new LDI table with the given constraint system and channels.
    ///
    /// # Arguments
    /// * `cs` - Constraint system to add the table to
    /// * `channels` - Channel IDs for communication with other tables
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("ldi");

        let Channels {
            state_channel,
            prom_channel,
            ..
        } = *channels;

        let cpu_cols = CpuColumns::new(
            &mut table,
            state_channel,
            prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let CpuColumns {
            fp,
            arg0: dst,
            arg1: imm_low,
            arg2: imm_high,
            ..
        } = cpu_cols;

        let abs_addr = table.add_computed("abs_addr", fp + upcast_col(dst));

        // Push value to VROM write table using absolute address
        let imm = table.add_computed("imm", pack_b16_into_b32([imm_low.into(), imm_high.into()]));
        table.pull(channels.vrom_channel, [abs_addr, imm]);

        Self {
            id: table.id(),
            cpu_cols,
            abs_addr,
            imm,
        }
    }
}

impl<U> TableFiller<U> for LdiTable
where
    U: CommonTableBounds,
{
    type Event = LDIEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<U>,
    ) -> anyhow::Result<()> {
        {
            let mut abs_addr = witness.get_mut_as(self.abs_addr)?;
            let mut imm = witness.get_mut_as(self.imm)?;
            for (i, event) in rows.clone().enumerate() {
                abs_addr[i] = *event.fp ^ (event.dst as u32);
                imm[i] = event.imm;
                dbg!("Ldi fill", &abs_addr[i]);
            }
        }
        let cpu_rows = rows.map(|event| CpuEvent {
            pc: event.pc.val(),
            next_pc: None,
            fp: *event.fp,
            next_fp: None,
            arg0: event.dst,
            arg1: event.imm as u16 & 0xFFFF,
            arg2: (event.imm >> 16) as u16,
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}

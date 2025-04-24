use std::any::Any;

use binius_m3::builder::{
    upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32,
};
use zcrayvm_assembly::{JumpiEvent, JumpvEvent, Opcode};

use crate::gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget, NextPc};
use crate::{channels::Channels, table::Table, types::ProverPackedField};

/// Table for JUMPI instruction.
///
/// Implements the unconditional jump to an immediate address.
/// Logic: PC = target
pub struct JumpiTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Jumpi as u16 }>,
}

impl Table for JumpiTable {
    type Event = JumpiEvent;

    fn name(&self) -> &'static str {
        "JumpiTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("jumpi");

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Immediate,
                next_fp: None,
            },
        );

        Self {
            id: table.id(),
            cpu_cols,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for JumpiTable {
    type Event = JumpiEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target.val()),
            fp: *event.fp,
            arg0: event.target.val() as u16,
            arg1: (event.target.val() >> 16) as u16,
            arg2: 0, // Unused for jumpi
        });
        self.cpu_cols.populate(witness, cpu_rows)?;
        Ok(())
    }
}

/// Table for JUMPV instruction.
///
/// Implements the unconditional jump to an address stored in VROM.
/// Logic: PC = FP[offset]
pub struct JumpvTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Jumpv as u16 }>,
    offset_addr: Col<B32>, // Virtual
    target_val: Col<B32>,
}

impl Table for JumpvTable {
    type Event = JumpvEvent;

    fn name(&self) -> &'static str {
        "JumpvTable"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("jumpv");

        let target_val = table.add_committed("target_val");

        let cpu_cols = CpuColumns::new(
            &mut table,
            channels.state_channel,
            channels.prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Target(target_val),
                next_fp: None,
            },
        );

        let offset_addr =
            table.add_computed("offset_addr", cpu_cols.fp + upcast_col(cpu_cols.arg0));

        // Read target_val from VROM
        table.pull(channels.vrom_channel, [offset_addr, target_val]);

        Self {
            id: table.id(),
            cpu_cols,
            offset_addr,
            target_val,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TableFiller<ProverPackedField> for JumpvTable {
    type Event = JumpvEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &'a self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> anyhow::Result<()> {
        {
            let mut offset_addr = witness.get_mut_as(self.offset_addr)?;
            let mut target_val = witness.get_mut_as(self.target_val)?;
            for (i, event) in rows.clone().enumerate() {
                offset_addr[i] = event.fp.addr(event.offset);
                target_val[i] = event.target;
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.val(),
            next_pc: Some(event.target),
            fp: *event.fp,
            arg0: event.offset,
            arg1: 0, // Unused for jumpv
            arg2: 0, // Unused for jumpv
        });
        self.cpu_cols.populate(witness, cpu_rows)?;
        Ok(())
    }
}

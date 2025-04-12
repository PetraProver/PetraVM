use std::ops::Deref;

use binius_m3::{
    builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32,
    },
    gadgets::u32::U32AddFlags,
};
use zcrayvm_assembly::{event::integer_ops::AddiEvent, opcodes::Opcode};

use crate::{
    channels::Channels,
    gadgets::{
        cpu::{CpuColumns, CpuColumnsOptions, CpuGadget, NextPc},
        u32u16::U32U16Add,
    },
    types::ProverPackedField,
};

const ADDI_OPCODE: u16 = Opcode::Addi as u16;

/// ADDI table.
///
/// This table handles the ADDI instruction, which performs integer
/// multiplication between a 32-bit element and a 16-bit immediate.
pub struct AddiTable {
    id: TableId,
    // TODO: Use the cpu gadget
    cpu_cols: CpuColumns<ADDI_OPCODE>,
    dst_abs: Col<B32>, // Virtual
    dst_val_packed: Col<B32>,
    src_abs: Col<B32>, // Virtual
    src_val: Col<B1, 32>,
    src_val_packed: Col<B32>,
    imm_unpacked: Col<B1, 16>,
    add_op: U32U16Add,
}

impl AddiTable {
    pub fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("add");

        let Channels {
            state_channel,
            prom_channel,
            vrom_channel,
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

        // Pull the destination and source values from the VROM channel.
        let dst_abs = table.add_computed("dst", cpu_cols.fp + upcast_col(cpu_cols.arg0));
        let src_abs = table.add_computed("src", cpu_cols.fp + upcast_col(cpu_cols.arg1));
        let src_val = table.add_committed("src_val");
        let src_val_packed = table.add_packed("src_val_packed", src_val);

        let imm_unpacked = table.add_committed("imm_unpacked");

        // Carry out the multiplication.
        let add_op = U32U16Add::new(&mut table, src_val, imm_unpacked, U32AddFlags::default());
        let dst_val_packed = table.add_packed("dst_val_packed", add_op.zout);

        // Read src1
        table.pull(vrom_channel, [src_abs, src_val_packed]);

        // Write dst
        table.pull(vrom_channel, [dst_abs, dst_val_packed]);

        Self {
            id: table.id(),
            cpu_cols,
            dst_abs,
            src_abs,
            src_val,
            src_val_packed,
            imm_unpacked,
            add_op,
            dst_val_packed,
        }
    }
}

impl TableFiller<ProverPackedField> for AddiTable {
    type Event = AddiEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut dst_abs = witness.get_mut_as(self.dst_abs)?;
            let mut src_abs = witness.get_mut_as(self.src_abs)?;
            let mut src_val = witness.get_mut_as(self.src_val)?;
            let mut imm = witness.get_mut_as(self.imm_unpacked)?;

            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = event.fp.addr(event.dst as u32);
                src_abs[i] = event.fp.addr(event.src as u32);
                src_val[i] = event.src_val;
                imm[i] = event.imm;
            }
        }
        let cpu_rows = rows.map(|event| CpuGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp.deref(),
            arg0: event.dst,
            arg1: event.src,
            arg2: event.imm,
        });
        self.cpu_cols.populate(witness, cpu_rows)?;
        self.add_op.populate(witness)
    }
}

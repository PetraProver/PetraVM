use std::ops::Deref;

use binius_m3::{
    builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B1, B32, B64,
    },
    gadgets::u32::U32AddFlags,
};
use zcrayvm_assembly::{event::integer_ops::AddiEvent, opcodes::Opcode};

use super::gadgets::U32U16Add;
use crate::{
    channels::Channels,
    gadgets::cpu::{CpuColumns, CpuColumnsOptions, CpuGadget, NextPc},
    types::ProverPackedField,
    utils::{pack_vrom_entry, pack_vrom_entry_u64},
};

const ADD_OPCODE: u16 = Opcode::Add as u16;

/// ADD table.
///
/// This table handles the ADD instruction, which returns from a function
/// call by loading the return PC and FP from the current frame.
///
/// Logic:
/// 1. Load the current PC and FP from the state channel
/// 2. Get the instruction from PROM channel
/// 3. Verify this is a RET instruction
/// 4. Load the return PC from VROM[fp+0] and return FP from VROM[fp+1]
/// 5. Update the state with the new PC and FP values
pub struct AddiTable {
    id: TableId,
    // TODO: Use the cpu gadget
    cpu_cols: CpuColumns<{ Opcode::Add as u16 }>,
    dst_abs: Col<B32>, // Virtual
    dst_val_packed: Col<B32>,
    src_abs: Col<B32>, // Virtual
    src_val: Col<B1, 32>,
    src_val_packed: Col<B32>,
    imm_unpacked: Col<B1, 16>,
    add_op: U32U16Add,

    vrom_src: Col<B64>,
    vrom_dst: Col<B64>,
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

        let dst_abs = table.add_computed("dst", cpu_cols.fp + upcast_col(cpu_cols.arg0));

        let src_abs = table.add_computed("src", cpu_cols.fp + upcast_col(cpu_cols.arg1));
        let src_val = table.add_committed("src_val");
        let src_val_packed = table.add_packed("src_val_packed", src_val);

        let imm_unpacked = table.add_committed("imm_unpacked");

        let add_op = U32U16Add::new(&mut table, src_val, imm_unpacked, U32AddFlags::default());
        let dst_val_packed = table.add_packed("dst_val_packed", add_op.zout);

        // Read src1
        let vrom_src = table.add_computed("vrom_src", pack_vrom_entry(src_abs, src_val_packed));
        table.pull(vrom_channel, [vrom_src]);

        // Write dst
        let vrom_dst =
            table.add_computed("vrom_dst", pack_vrom_entry(dst_abs, dst_val_packed.into()));
        table.pull(vrom_channel, [vrom_dst]);

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
            vrom_src,
            vrom_dst,
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
            let mut vrom_src = witness.get_mut_as(self.vrom_src)?;
            let mut vrom_dst = witness.get_mut_as(self.vrom_dst)?;
            for (i, event) in rows.clone().enumerate() {
                dst_abs[i] = event.fp.addr(event.dst as u32);
                src_abs[i] = event.fp.addr(event.src as u32);
                src_val[i] = event.src_val;
                imm[i] = event.imm;
                vrom_src[i] = pack_vrom_entry_u64(event.src as u32, event.src_val);
                vrom_dst[i] = pack_vrom_entry_u64(event.dst as u32, event.dst_val);
                dbg!("Add fill", src_val[i], vrom_src[i], vrom_dst[i],);
                println!(
                    "vrom_scr = {:x},  vrom_dst = {:x}",
                    vrom_src[i], vrom_dst[i]
                );
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

use binius_core::constraint_system::channel::ChannelId;
use binius_field::{as_packed_field::PackScalar, underlier::UnderlierType};
use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessIndexSegment, B1,
    B32, B64,
};
use bytemuck::Pod;
use zcrayvm_assembly::{Opcode, XoriEvent};

use crate::{channels::ZkVMChannels, opcodes::{cpu::{CpuColumns, CpuColumnsOptions, CpuEvent}, util::B64_B32_BASIS}};

pub struct XoriTable {
    id: TableId,
    cpu_cols: CpuColumns<{ Opcode::Xori as u16 }>,
    dst_val: Col<B32>, // Virtual
    src_val: Col<B32>,
    vrom_dst: Col<B64>, // Virtual
    vrom_src: Col<B64>, // Virtual
}

impl XoriTable {
    pub fn new(
        cs: &mut ConstraintSystem,
        channels: &ZkVMChannels,
    ) -> Self {
        let mut table = cs.add_table("ret");
        let src_val = table.add_committed("src_val");

        let ZkVMChannels {
            state_channel,
            prom_channel,
            vrom_channel,
            ..
        } = *channels;

        let cpu_cols = CpuColumns::new(
            &mut table,
            state_channel,
            prom_channel,
            CpuColumnsOptions::default(),
        );
        let dst = cpu_cols.arg0;
        let src = cpu_cols.arg1;
        let imm = cpu_cols.arg2;

        let dst_val = table.add_computed("dst_val", src_val + upcast_expr(imm.into()));

        // Read dst_val
        let vrom_dst = table.add_computed(
            "vrom_dst",
            upcast_expr(dst.into()) * B64_B32_BASIS[0] + upcast_expr(dst_val.into()) * B64_B32_BASIS[1],
        );
        table.push(vrom_channel, [vrom_dst]);

        // Read src_val
        let vrom_src = table.add_computed(
            "vrom_src",
            upcast_expr(src.into()) * B64_B32_BASIS[0] + upcast_expr(src_val.into()) * B64_B32_BASIS[1],
        );
        table.push(vrom_channel, [vrom_src]);

        Self {
            id: table.id(),
            cpu_cols,
            dst_val,
            src_val,
            vrom_dst,
            vrom_src,
        }
    }
}

impl<U: UnderlierType> TableFiller<U> for XoriTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = XoriEvent;

    fn id(&self) -> TableId {
        self.id
    }

    // TODO: This implementation might be very similar for all immediate binary
    // operations
    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessIndexSegment<U>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut dst_val = witness.get_mut_as(self.dst_val)?;
            let mut src_val = witness.get_mut_as(self.src_val)?;
            let mut vrom_dst = witness.get_mut_as(self.vrom_dst)?;
            let mut vrom_src = witness.get_mut_as(self.vrom_src)?;
            for (i, event) in rows.clone().enumerate() {
                src_val[i] = event.src_val;
                dst_val[i] = event.dst_val;
                vrom_dst[i] = (event.dst_val as u64) << 32 | event.dst as u64;
                vrom_src[i] = (event.src_val as u64) << 32 | event.src as u64;
            }
        }
        let cpu_rows = rows.map(|event| CpuEvent {
            pc: event.pc.into(),
            next_pc: None,
            fp: event.fp,
            next_fp: None,
            arg0: event.dst,
            arg1: event.src,
            arg2: event.imm,
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}
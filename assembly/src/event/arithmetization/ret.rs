use binius_core::constraint_system::channel::ChannelId;
use binius_field::{as_packed_field::PackScalar, underlier::UnderlierType, ExtensionField, Field};
use binius_m3::builder::{
    upcast_col, upcast_expr, Col, ConstraintSystem, Expr, TableFiller, TableId,
    TableWitnessIndexSegment, B1, B16, B32, B64,
};
use bytemuck::Pod;
use env_logger::Target;

use super::cpu::NextPc;
use crate::{
    event::arithmetization::cpu::{CpuColumns, CpuColumnsOptions, CpuRow, Instruction},
    opcodes::Opcode,
};

pub(crate) struct RetTable {
    id: TableId,
    cpu_cols: CpuColumns,
    fp1: Col<B32>, // Virtual
    fp0_val: Col<B32>,
    fp1_val: Col<B32>,

    vrom_next_pc: Col<B64>,
    vrom_next_fp: Col<B64>,
}

impl RetTable {
    pub fn new(
        cs: &mut ConstraintSystem,
        state_channel: ChannelId,
        vrom_channel: ChannelId,
        prom_channel: ChannelId,
    ) -> Self {
        let mut table = cs.add_table("ret");
        let fp0_val = table.add_committed("fp0_val");
        let fp1_val = table.add_committed("fp1_val");

        let cpu_cols = CpuColumns::new::<{ Opcode::Ret as u16 }>(
            &mut table,
            state_channel,
            prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Target(fp0_val),
                next_fp: Some(fp1_val),
            },
        );

        let fp0 = cpu_cols.fp;
        let fp1 = table.add_computed("fp1", fp0 + B32::ONE);

        // TODO: Load this from some utility module
        let b64_basis: [_; 2] = std::array::from_fn(|i| {
            <B64 as ExtensionField<B32>>::basis(i).expect("i in range 0..2; extension degree is 2")
        });
        let pack_b32_into_b64 = move |limbs: [Expr<B32, 1>; 2]| {
            limbs
                .into_iter()
                .enumerate()
                .map(|(i, limb)| upcast_expr(limb) * b64_basis[i])
                .reduce(|a, b| a + b)
                .expect("limbs has length 2")
        };

        // Read the next_pc
        let vrom_next_pc = table.add_computed(
            "fp0_fp0_val",
            pack_b32_into_b64([fp0_val.into(), fp0.into()]),
        );
        table.push(vrom_channel, [vrom_next_pc]);

        //Read the next_fp
        let vrom_next_fp = table.add_computed(
            "fp1_fp1_val",
            pack_b32_into_b64([fp1_val.into(), fp1.into()]),
        );
        table.push(vrom_channel, [vrom_next_fp]);

        Self {
            id: table.id(),
            cpu_cols,
            fp1,
            fp0_val,
            fp1_val,
            vrom_next_pc,
            vrom_next_fp,
        }
    }
}

impl<U: UnderlierType> TableFiller<U> for RetTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = crate::event::model::ret::RetEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessIndexSegment<U>,
    ) -> Result<(), anyhow::Error> {
        for (i, event) in rows.clone().enumerate() {
            {
                // TODO: Move this outside the loop
                let mut fp1 = witness.get_mut_as(self.fp1)?;
                let mut fp0_val = witness.get_mut_as(self.fp0_val)?;
                let mut fp1_val = witness.get_mut_as(self.fp1_val)?;
                let mut vrom_next_pc = witness.get_mut_as(self.vrom_next_pc)?;
                let mut vrom_next_fp = witness.get_mut_as(self.vrom_next_fp)?;
                fp1[i] = event.fp ^ 1;
                fp0_val[i] = event.fp_0_val;
                fp1_val[i] = event.fp_1_val;
                vrom_next_pc[i] = (event.fp as u64) << 32 | event.fp_0_val as u64;
                vrom_next_fp[i] = (event.fp as u64 ^ 1) << 32 | event.fp_1_val as u64;
            }
            println!("Ret");
        }
        let cpu_rows = rows.map(|event| CpuRow {
            pc: event.pc.into(),
            next_pc: Some(event.fp_0_val),
            fp: event.fp,
            instruction: Instruction {
                opcode: Opcode::Ret,
                ..Default::default()
            },
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}

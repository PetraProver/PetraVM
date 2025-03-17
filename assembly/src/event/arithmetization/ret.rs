
    use binius_core::constraint_system::channel::ChannelId;
    use binius_field::{as_packed_field::PackScalar, underlier::UnderlierType, Field};
    use binius_m3::builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessIndexSegment, B1, B16,
        B32,
    };
    use bytemuck::Pod;

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

            let cpu_cols = CpuColumns::new(
                &mut table,
                state_channel,
                prom_channel,
                CpuColumnsOptions {
                    jumps: Some(fp0_val),
                    next_fp: Some(fp1_val),
                    opcode: Opcode::Ret,
                },
            );

            let fp0 = cpu_cols.fp;
            let fp1 = table.add_linear_combination("fp1", fp0 + B32::ONE);
            let timestamp = cpu_cols.timestamp;

            // Read the next_pc
            table.push(
                vrom_channel,
                [upcast_col(fp0), upcast_col(fp0_val), upcast_col(timestamp)],
            );

            //Read the next_fp
            table.push(
                vrom_channel,
                [upcast_col(fp1), upcast_col(fp1_val), upcast_col(timestamp)],
            );

            Self {
                id: table.id(),
                cpu_cols,
                fp1,
                fp0_val,
                fp1_val,
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
            rows: impl Iterator<Item = &'a Self::Event>,
            witness: &'a mut TableWitnessIndexSegment<U>,
        ) -> Result<(), anyhow::Error> {
            for (i, event) in rows.enumerate() {
                self.cpu_cols.fill_row(
                    witness,
                    CpuRow {
                        index: i,
                        pc: event.pc.val(),
                        fp: event.fp,
                        timestamp: event.timestamp,
                        instruction: Instruction {
                            opcode: Opcode::Ret,
                            arg0: 0,
                            arg1: 0,
                            arg2: 0,
                        },
                    },
                );

                // TODO: Move this outside the loop
                let mut fp1 = witness.get_mut_as(self.fp1)?;
                let mut fp0_val = witness.get_mut_as(self.fp0_val)?;
                let mut fp1_val = witness.get_mut_as(self.fp1_val)?;
                fp1[i] = event.fp ^ 1;
                fp0_val[i] = event.fp_0_val;
                fp1_val[i] = event.fp_1_val;
            }

            Ok(())
        }
    }

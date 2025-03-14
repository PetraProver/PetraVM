use binius_field::BinaryField32b;
use binius_m3::builder::TableFiller;

use super::Event;
use crate::emulator::{Interpreter, InterpreterChannels, InterpreterError, InterpreterTables};

/// Event for RET.
///
/// Performs a return from a function call.
///
/// Logic:
///   1. PC = FP[0]
///   2. FP = FP[1]
#[derive(Debug, PartialEq)]
pub struct RetEvent {
    pub(crate) pc: BinaryField32b,
    pub(crate) fp: u32,
    pub(crate) timestamp: u32,
    pub(crate) fp_0_val: u32,
    pub(crate) fp_1_val: u32,
}

impl RetEvent {
    pub fn new(
        interpreter: &Interpreter,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;
        Ok(Self {
            pc: field_pc,
            fp,
            timestamp: interpreter.timestamp,
            fp_0_val: interpreter.get_vrom_u32(fp)?,
            fp_1_val: interpreter.get_vrom_u32(fp + 1)?,
        })
    }

    pub fn generate_event(
        interpreter: &mut Interpreter,
        field_pc: BinaryField32b,
    ) -> Result<Self, InterpreterError> {
        let fp = interpreter.fp;

        let ret_event = RetEvent::new(interpreter, field_pc);
        interpreter.jump_to(BinaryField32b::new(interpreter.get_vrom_u32(fp)?));
        interpreter.fp = interpreter.get_vrom_u32(fp + 1)?;

        ret_event
    }
}

impl Event for RetEvent {
    fn fire(&self, channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels.state_channel.push((
            BinaryField32b::new(self.fp_0_val),
            self.fp_1_val,
            self.timestamp + 1,
        ));
    }
}

mod arithmetization {
    use binius_core::constraint_system::channel::ChannelId;
    use binius_field::{as_packed_field::PackScalar, underlier::UnderlierType, Field};
    use binius_m3::builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessIndexSegment, B1, B16,
        B32,
    };
    use bytemuck::Pod;

    use crate::{
        event::cpu::{CpuColumns, CpuColumnsOptions, CpuRow, Instruction},
        opcodes::Opcode,
    };

    struct RetTable {
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
        type Event = super::RetEvent;

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
}

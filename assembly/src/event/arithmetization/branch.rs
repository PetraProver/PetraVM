use core::time;

use binius_core::constraint_system::channel::ChannelId;
use binius_field::{
    as_packed_field::PackScalar, underlier::UnderlierType, BinaryField16b, BinaryField32b,
    ExtensionField, Field,
};
use binius_m3::builder::{
    upcast_col, upcast_expr, Col, ConstraintSystem, Expr, TableFiller, TableId,
    TableWitnessIndexSegment, B1, B32, B64,
};
use bytemuck::Pod;
use env_logger::fmt::Timestamp;

use super::cpu::{CpuColumns, CpuColumnsOptions, CpuRow, Instruction, NextPc};
use crate::opcodes::Opcode;

/// Table for BNZ.
///
/// Performs a branching to the target address if the argument is not zero.
///
/// Logic:
///   1. if FP[cond] <> 0, then PC = target
///   2. if FP[cond] == 0, then increment PC
pub(crate) struct BnzTable {
    id: TableId,
    cpu_cols: CpuColumns,
    cond_val: Col<B32>, // Constant
}

impl BnzTable {
    pub fn new(
        cs: &mut ConstraintSystem,
        state_channel: ChannelId,
        vrom_channel: ChannelId,
        prom_channel: ChannelId,
    ) -> Self {
        let mut table = cs.add_table("bnz");
        let cond_val = table.add_committed("cond_val");

        // TODO: Assert cond_val is != 0

        let cpu_cols = CpuColumns::new::<{ Opcode::Bnz as u16 }>(
            &mut table,
            state_channel,
            prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Immediate,
                next_fp: None,
            },
        );

        let cond = cpu_cols.arg0;

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

        let cond_cond_val = table.add_computed(
            "cond_cond_val",
            pack_b32_into_b64([upcast_col(cond).into(), cond_val.into()]),
        );
        // Read cond_val
        table.push(vrom_channel, [cond_cond_val]);

        Self {
            id: table.id(),
            cpu_cols,
            cond_val,
        }
    }
}

impl<U: UnderlierType> TableFiller<U> for BnzTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = crate::event::model::branch::BnzEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessIndexSegment<U>,
    ) -> Result<(), anyhow::Error> {
        for (i, event) in rows.enumerate() {
            let row = CpuRow {
                index: i,
                pc: event.pc.val(),
                next_pc: None,
                fp: event.fp,
                instruction: Instruction {
                    opcode: Opcode::Bnz,
                    arg0: event.cond,
                    arg1: event.target_low.val(),
                    arg2: event.target_high.val(),
                },
            };
            self.cpu_cols.fill_row(witness, row)?;
            let mut cond_val = witness.get_mut_as(self.cond_val)?;
            cond_val[i] = event.cond_val;
        }
        Ok(())
    }
}

pub(crate) struct BzTable {
    id: TableId,
    cpu_cols: CpuColumns,
}

impl BzTable {
    pub fn new(
        cs: &mut ConstraintSystem,
        state_channel: ChannelId,
        vrom_channel: ChannelId,
        prom_channel: ChannelId,
    ) -> Self {
        let mut table = cs.add_table("bz");

        let cpu_cols = CpuColumns::new::<{ Opcode::Bnz as u16 }>(
            &mut table,
            state_channel,
            prom_channel,
            CpuColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        let cond = cpu_cols.arg0;
        // TODO: Should we have a single zero?
        let zero = table.add_constant("zero", [B32::ZERO]);

        // cond_val must be zero
        table.push(prom_channel, [upcast_col(cond), zero]);

        Self {
            id: table.id(),
            cpu_cols,
        }
    }
}

impl<U: UnderlierType> TableFiller<U> for BzTable
where
    U: Pod + PackScalar<B1>,
{
    type Event = crate::event::model::branch::BzEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event>,
        witness: &'a mut TableWitnessIndexSegment<U>,
    ) -> Result<(), anyhow::Error> {
        for (i, event) in rows.enumerate() {
            let row = CpuRow {
                index: i,
                pc: event.pc.val(),
                next_pc: None,
                fp: event.fp,
                instruction: Instruction {
                    opcode: Opcode::Bnz,
                    arg0: 0,
                    arg1: 0,
                    arg2: 0,
                },
            };
            self.cpu_cols.fill_row(witness, row)?;
        }
        Ok(())
    }
}

pub mod test {
    use std::collections::HashMap;

    use binius_field::{arch::OptimalUnderlier128b, BinaryField, Field};
    use binius_m3::builder::{
        Boundary, ConstraintSystem, FlushDirection, Statement, B128, B16, B32,
    };
    use bumpalo::Bump;

    use crate::{
        execution::{emulator_arithmetization::arithmetization::ZCrayTable, trace::BoundaryValues},
        opcodes::Opcode,
        util::code_to_prom,
        Memory, ValueRom, ZCrayTrace,
    };

    #[test]
    fn test_bnz() {
        let mut cs = ConstraintSystem::new();
        let zcray_table = ZCrayTable::new(&mut cs);

        let zero = B16::ZERO;
        // TODO: This is a Ret!!!
        let code = vec![
            [Opcode::Bnz.get_field_elt(), zero, zero, zero],
            [Opcode::Ret.get_field_elt(), zero, zero, zero],
        ];
        let prom = code_to_prom(&code);
        let vrom = ValueRom::new(HashMap::new());

        let mut frames = HashMap::new();
        frames.insert(B32::ONE, 12);

        let memory = Memory::new(prom, ValueRom::new_with_init_vals(&[0, 0]));
        let (trace, boundary_values) =
            ZCrayTrace::generate(memory, frames, HashMap::new()).expect("Ouch!");

        let BoundaryValues {
            final_pc,
            final_fp,
            timestamp: final_timestamp,
        } = boundary_values;

        trace.validate(boundary_values);

        let statement = get_test_bnz_statement(
            &zcray_table,
            final_pc,
            final_fp,
            final_timestamp,
            vec![
                trace.add.len(),
                trace.ret.len(),
                trace.bnz.len(),
                trace.bz.len(),
            ],
        );

        let allocator = Bump::new();
        let mut witness = cs
            .build_witness::<OptimalUnderlier128b>(&allocator, &statement)
            .unwrap();

        zcray_table.populate(trace, &mut witness).unwrap();

        let compiled_cs = cs.compile(&statement).unwrap();
        let witness = witness.into_multilinear_extension_index(&statement);

        binius_core::constraint_system::validate::validate_witness(
            &compiled_cs,
            &statement.boundaries,
            &witness,
        )
        .unwrap();

        const LOG_INV_RATE: usize = 1;
        const SECURITY_BITS: usize = 100;

        // let proof = binius_core::constraint_system::prove::<
        //     _,
        //     CanonicalTowerFamily,
        //     _,
        //     Groestl256,
        //     Groestl256ByteCompression,
        //     HasherChallenger<Groestl256>,
        //     _,
        // >(
        //     &compiled_cs,
        //     LOG_INV_RATE,
        //     SECURITY_BITS,
        //     &statement.boundaries,
        //     witness,
        //     &DefaultEvaluationDomainFactory::default(),
        //     &binius_hal::make_portable_backend(),
        // )
        // .unwrap();

        // binius_core::constraint_system::verify::<
        //     OptimalUnderlier128b,
        //     CanonicalTowerFamily,
        //     Groestl256,
        //     Groestl256ByteCompression,
        //     HasherChallenger<Groestl256>,
        // >(
        //     &compiled_cs,
        //     LOG_INV_RATE,
        //     SECURITY_BITS,
        //     &statement.boundaries,
        //     proof,
        // )
        // .unwrap();
    }

    // Since we still don't have tables implementing the lookups, We balance the
    // prom and vrom channels by just pulling the expected values in the
    // execution.
    fn get_test_bnz_statement(
        zcray_table: &ZCrayTable,
        final_pc: B32,
        final_fp: u32,
        final_timestamp: u32,
        table_sizes: Vec<usize>,
    ) -> Statement {
        Statement {
            boundaries: vec![
                Boundary {
                    values: vec![B128::ONE, B128::new(0), B128::new(0)], /* inital_pc = 0,
                                                                          * inital_fp = 0,
                                                                          * initial_timestamp
                                                                          * = 0 */
                    channel_id: zcray_table.state_channel,
                    direction: FlushDirection::Push,
                    multiplicity: 1,
                },
                Boundary {
                    values: vec![
                        B128::new(final_pc.val() as u128),
                        B128::new(final_fp as u128),
                        B128::new(final_timestamp as u128),
                    ],
                    channel_id: zcray_table.state_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
                // For now we add the prom here
                Boundary {
                    values: vec![
                        B128::ONE,
                        B128::new((Opcode::Add as u16).into()),
                        0.into(),
                        0.into(),
                        0.into(),
                    ],
                    channel_id: zcray_table.prom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
                Boundary {
                    values: vec![
                        B32::MULTIPLICATIVE_GENERATOR.into(),
                        B128::new((Opcode::Ret as u16).into()),
                        0.into(),
                        0.into(),
                        0.into(),
                    ],
                    channel_id: zcray_table.prom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
                // For now we add the vrom here
                // Read src1 and src2 from ADD
                Boundary {
                    values: vec![B128::ZERO, B128::ZERO, B128::ZERO],
                    channel_id: zcray_table.vrom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 2,
                },
                // Write dst from ADD
                // table.push(vrom_channel, [timestamp, upcast_col(dst), dst_val_packed]);
                Boundary {
                    values: vec![B128::ONE, B128::ZERO, B128::ZERO],
                    channel_id: zcray_table.vrom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
                // Read the next_pc from RET
                Boundary {
                    values: vec![B128::ONE, B128::ZERO, B128::ZERO],
                    channel_id: zcray_table.vrom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
                //Read the next_fp
                Boundary {
                    values: vec![B128::ONE, B128::ONE, B128::ZERO],
                    channel_id: zcray_table.vrom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
            ],
            table_sizes, // TODO: What should be here?
        }
    }
}

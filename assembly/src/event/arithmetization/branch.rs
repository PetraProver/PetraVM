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
    cond_val: Col<B32>,  // Constant
    vrom_push: Col<B64>, // Virtual;
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
            vrom_push: cond_cond_val,
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
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessIndexSegment<U>,
    ) -> Result<(), anyhow::Error> {
        {
            let mut cond_val = witness.get_mut_as(self.cond_val)?;
            let mut cond_cond_val = witness.get_mut_as(self.vrom_push)?;
            for (i, event) in rows.clone().enumerate() {
                cond_val[i] = event.cond_val;
                cond_cond_val[i] = (event.cond as u64) << 32 | event.cond_val as u64;
            }
        }
        let cpu_rows = rows.map(|event| CpuRow {
            pc: event.pc.val(),
            next_pc: Some((event.target_high.val() as u32) << 16 | event.target_low.val() as u32),
            fp: event.fp,
            next_fp: None,
            instruction: Instruction {
                opcode: Opcode::Bnz,
                arg0: event.cond,
                arg1: event.target_low.val(),
                arg2: event.target_high.val(),
            },
        });
        self.cpu_cols.populate(witness, cpu_rows)?;
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
        let cpu_rows = rows.map(|event| CpuRow {
            pc: event.pc.val(),
            next_pc: None,
            fp: event.fp,
            next_fp: None,
            instruction: Instruction {
                opcode: Opcode::Bnz,
                ..Default::default()
            },
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}

pub mod test {
    use std::collections::HashMap;

    use binius_field::{arch::OptimalUnderlier128b, BinaryField, ExtensionField, Field};
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

        let generator_low = B16::new((B32::MULTIPLICATIVE_GENERATOR.val() & 0xFFFF) as u16);
        let generator_high = B16::new((B32::MULTIPLICATIVE_GENERATOR.val() >> 16) as u16);
        assert_eq!(
            B32::from_bases([generator_low, generator_high]).unwrap(),
            B32::MULTIPLICATIVE_GENERATOR
        );
        let code = vec![
            // Jumps to the next line because there's a 1 at address 0x1, and the next
            // line is at B32::MULTIPLICATIVE_GENERATOR.
            [
                Opcode::Bnz.get_field_elt(),
                B16::ONE,
                generator_low,
                generator_high,
            ],
            [Opcode::Ret.get_field_elt(), B16::ZERO, B16::ZERO, B16::ZERO],
        ];
        let prom = code_to_prom(&code);
        let vrom = ValueRom::new(HashMap::new());

        let mut frames = HashMap::new();
        frames.insert(B32::ONE, 12);

        // The memory contains two values a addresses 0x0 and 0x1, 0 which is the value
        // of next_pc, and 1 which is the predicate in BNZ as well as next_fp in
        // RET.
        let memory = Memory::new(prom, ValueRom::new_with_init_vals(&[0, 1]));

        let (trace, boundary_values) = ZCrayTrace::generate(
            memory,
            frames,
            [(B32::ONE, 1), (B32::MULTIPLICATIVE_GENERATOR, 2)].into(),
        )
        .expect("Ouch!");

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
                    // first_pc = 1, first_fp = 0
                    //                                       |..pc..||..fp..|
                    values: vec![B128::new(0x00000000000000000000000100000000)],
                    channel_id: zcray_table.state_channel,
                    direction: FlushDirection::Push,
                    multiplicity: 1,
                },
                Boundary {
                    values: vec![B128::new((final_pc.val() as u128) << 32 | final_fp as u128)],
                    channel_id: zcray_table.state_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
                // For now we add the prom here
                    Boundary {
                        values: vec![B128::new(
                            1 << 64
                                | Opcode::Bnz as u128
                                | 1 << 16
                                | (B32::MULTIPLICATIVE_GENERATOR.val() as u128 & 0xFFFF) << 32
                                | (B32::MULTIPLICATIVE_GENERATOR.val() as u128 >> 16) << 48,
                        )],
                        channel_id: zcray_table.prom_channel,
                        direction: FlushDirection::Pull,
                        multiplicity: 1,
                },
                Boundary {
                    values: vec![B128::new(
                        (B32::MULTIPLICATIVE_GENERATOR.val() as u128) << 64 | Opcode::Ret as u128,
                    )],
                    channel_id: zcray_table.prom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
                // For now we add the vrom here
                // Read next_pc in RET
                Boundary {
                    values: vec![B128::ZERO],
                    channel_id: zcray_table.vrom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
                //Read cond_val in BNZ and fp^1 in RET
                Boundary {
                    values: vec![B128::new(1 << 32 | 1)],
                    channel_id: zcray_table.vrom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 2,
                },
            ],
            table_sizes, // TODO: What should be here?
        }
    }
}

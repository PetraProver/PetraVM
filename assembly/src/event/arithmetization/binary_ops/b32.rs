use binius_core::constraint_system::channel::ChannelId;
use binius_field::{as_packed_field::PackScalar, underlier::UnderlierType, ExtensionField};
use binius_m3::builder::{
    upcast_expr, Col, ConstraintSystem, TableFiller, TableId, TableWitnessIndexSegment, B1, B16,
    B32, B64,
};
use bytemuck::Pod;

use crate::{
    event::arithmetization::cpu::{CpuColumns, CpuColumnsOptions, CpuRow, Instruction},
    Opcode,
};

pub struct XoriTable {
    id: TableId,
    cpu_cols: CpuColumns,
    dst_val: Col<B32>, // Virtual
    src_val: Col<B32>,
    vrom_dst: Col<B64>, // Virtual
    vrom_src: Col<B64>, // Virtual
}

impl XoriTable {
    pub fn new(
        cs: &mut ConstraintSystem,
        state_channel: ChannelId,
        vrom_channel: ChannelId,
        prom_channel: ChannelId,
    ) -> Self {
        let mut table = cs.add_table("ret");
        let src_val = table.add_committed("src_val");

        let cpu_cols = CpuColumns::new::<{ Opcode::Xori as u16 }>(
            &mut table,
            state_channel,
            prom_channel,
            CpuColumnsOptions::default(),
        );
        let dst = cpu_cols.arg0;
        let src = cpu_cols.arg1;
        let imm = cpu_cols.arg2;

        // TODO: Load this from some utility module
        let b64_basis: [_; 2] = std::array::from_fn(|i| {
            <B64 as ExtensionField<B32>>::basis(i).expect("i in range 0..2; extension degree is 2")
        });

        let dst_val = table.add_computed("dst_val", src_val + upcast_expr(imm.into()));

        // Read dst_val
        let vrom_dst = table.add_computed(
            "vrom_dst",
            upcast_expr(dst.into()) * b64_basis[0] + upcast_expr(dst_val.into()) * b64_basis[1],
        );
        table.push(vrom_channel, [vrom_dst]);

        // Read src_val
        let vrom_src = table.add_computed(
            "vrom_src",
            upcast_expr(src.into()) * b64_basis[0] + upcast_expr(src_val.into()) * b64_basis[1],
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
    type Event = crate::event::model::b32::XoriEvent;

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
        let cpu_rows = rows.map(|event| CpuRow {
            pc: event.pc.into(),
            next_pc: None,
            fp: event.fp,
            next_fp: None,
            instruction: Instruction {
                opcode: Opcode::Xori,
                arg0: event.dst,
                arg1: event.src,
                arg2: event.imm,
            },
        });
        self.cpu_cols.populate(witness, cpu_rows)
    }
}

pub mod test {
    use std::collections::HashMap;

    use binius_core::{
        fiat_shamir::HasherChallenger, tower::CanonicalTowerFamily,
        witness::MultilinearExtensionIndex,
    };
    use binius_field::{arch::OptimalUnderlier128b, BinaryField, Field};
    use binius_m3::builder::{
        Boundary, ConstraintSystem, FlushDirection, Statement, B128, B16, B32,
    };
    use binius_math::DefaultEvaluationDomainFactory;
    use bumpalo::Bump;
    use groestl_crypto::Groestl256;

    use crate::{
        execution::{
            emulator_arithmetization::arithmetization::ZCrayTable,
            trace::{BoundaryValues, ZCrayTrace},
        },
        opcodes::Opcode,
        util::code_to_prom,
        Memory, ValueRom,
    };

    #[test]
    fn test_xori() {
        let mut cs = ConstraintSystem::new();
        let zcray_table = ZCrayTable::new(&mut cs);

        let src = B16::new(2);
        let dst = B16::new(3);
        // TODO: This is a Ret!!!
        let code = vec![
            [Opcode::Xori.get_field_elt(), dst, src, B16::ONE],
            [Opcode::Ret.get_field_elt(), B16::ZERO, B16::ZERO, B16::ZERO],
        ];
        let prom = code_to_prom(&code);

        let mut frames = HashMap::new();
        frames.insert(B32::ONE, 12);

        let memory = Memory::new(prom, ValueRom::new_with_init_vals(&[0, 0, 0x10]));
        let (trace, boundary_values) =
            ZCrayTrace::generate(memory, frames, HashMap::new()).expect("Ouch!");

        let BoundaryValues {
            final_pc,
            final_fp,
            timestamp: final_timestamp,
        } = boundary_values;

        trace.validate(boundary_values);

        let statement = get_test_xori_statement(
            &zcray_table,
            final_pc,
            final_fp,
            final_timestamp,
            vec![
                trace.add.len(),
                trace.ret.len(),
                trace.bnz.len(),
                trace.bz.len(),
                trace.xori.len(),
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
    fn get_test_xori_statement(
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
                        1 << 64 | Opcode::Xori as u128 | 3 << 16 | 2 << 32 | 1 << 48,
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
                // Read src and from XORI
                Boundary {
                    values: vec![B128::new((0x10_u128) << 32 | 2)],
                    channel_id: zcray_table.vrom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
                // Write dst from XORI
                Boundary {
                    values: vec![B128::new((0x11_u128) << 32 | 3)],
                    channel_id: zcray_table.vrom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
                // Read the next_pc from RET
                Boundary {
                    values: vec![B128::ZERO],
                    channel_id: zcray_table.vrom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
                //Read the next_fp from RET
                Boundary {
                    values: vec![B128::new(1)],
                    channel_id: zcray_table.vrom_channel,
                    direction: FlushDirection::Pull,
                    multiplicity: 1,
                },
            ],
            table_sizes,
        }
    }
}

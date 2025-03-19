pub(crate) mod cpu;
pub(crate) mod integer_ops;
pub(crate) mod ret;

pub mod test {
    use std::collections::HashMap;

    use binius_core::{
        fiat_shamir::HasherChallenger, tower::CanonicalTowerFamily,
        witness::MultilinearExtensionIndex,
    };
    use binius_field::{arch::OptimalUnderlier128b, BinaryField, Field};
    use binius_hash::compress::Groestl256ByteCompression;
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
    fn test_addi() {
        let mut cs = ConstraintSystem::new();
        let zcray_table = ZCrayTable::new(&mut cs);

        let zero = B16::ZERO;
        // TODO: This is a Ret!!!
        let code = vec![
            [Opcode::Add.get_field_elt(), zero, zero, zero],
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

        let statement = get_test_addi_statement(
            &zcray_table,
            final_pc,
            final_fp,
            final_timestamp,
            vec![trace.add.len(), trace.ret.len()],
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
    fn get_test_addi_statement(
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

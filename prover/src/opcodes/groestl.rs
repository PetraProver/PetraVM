use std::{array::from_fn, cell::RefMut};

use binius_field::packed::set_packed_slice;
use binius_field::AESTowerField8b;
use binius_field::Field;
use binius_hash::groestl::GroestlShortImpl;
use binius_hash::groestl::GroestlShortInternal;
use binius_m3::builder::Expr;
use binius_m3::{
    builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32, B64, B8,
    },
    gadgets::hash::groestl::Permutation,
};
use petravm_asm::{Groestl256CompressEvent, Groestl256OutputEvent, Opcode};

use crate::gadgets::transpose::TransposeColumns;
use crate::utils::u64_to_bytes;
use crate::utils::u64_to_u32;
use crate::{
    channels::Channels,
    gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget},
    table::Table,
    types::ProverPackedField,
};

const GROESTL_COMPRESS_OPCODE: u16 = Opcode::Groestl256Compress as u16;
const GROESTL_OUTPUT_OPCODE: u16 = Opcode::Groestl256Output as u16;

/// GROEST256_COMPRESS table.
///
/// This table handles the GROEST256_COMPRESS instruction, which performs the
/// compression function described in the Groestl specs.
pub struct Groestl256CompressTable {
    id: TableId,
    state_cols: StateColumns<GROESTL_COMPRESS_OPCODE>,
    // Base address.
    dst_addresses: [Col<B32>; 8],
    dst_addresses_plus_one: [Col<B32>; 8],
    dst_selected: [Col<B32>; 16],
    // We need to write all 16 words to the VROM channel.
    dst_vals: [Col<B64>; 8],
    // Base address.
    src1_addresses: [Col<B32>; 16],
    // Columns needed for transposing src1.
    src1_transposition: TransposeColumns,
    src1_vals: [Col<B8, 8>; 8],
    src2_addresses: [Col<B32>; 16],
    // Columns needed for transposing src2.
    src2_transposition: TransposeColumns,
    src2_vals: [Col<B8, 8>; 8],
    out: [Col<B8, 8>; 8],
    projected_out: [[Col<B8>; 8]; 8],
    zero_padded_out: [[Col<B8, 8>; 8]; 8],
    interm: [Col<B8, 8>; 8],
    // P permutation.
    p_op: Permutation,
    // Q permutation.
    q_op: Permutation,
}

impl Table for Groestl256CompressTable {
    type Event = Groestl256CompressEvent;

    fn name(&self) -> &'static str {
        "Groestl256Compress"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("Groestl256Compress");

        let Channels {
            state_channel,
            prom_channel,
            vrom_channel,
            ..
        } = *channels;

        let state_cols = StateColumns::new(
            &mut table,
            state_channel,
            prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );
        // Get destination and source values.
        let src1_vals = from_fn(|i| table.add_committed(format!("src1_val_{i}")));
        let src2_vals = from_fn(|i| table.add_committed(format!("src2_val_{i}")));

        // Get the base address for the first and second source values.
        let src1_abs =
            table.add_computed("src1_addr_0", state_cols.fp + upcast_col(state_cols.arg1));
        let src2_abs =
            table.add_computed("src2_addr_0", state_cols.fp + upcast_col(state_cols.arg2));

        // Get all the addresses for the first and second source values.
        let mut src1_addresses = [src1_abs; 16];
        for (i, s1) in src1_addresses.iter_mut().enumerate().skip(1) {
            *s1 = table.add_computed(format!("src1_addr_{i}"), src1_abs + B32::from(i as u32));
        }
        let mut src2_addresses = [src2_abs; 16];
        for (i, s2) in src2_addresses.iter_mut().enumerate().skip(1) {
            *s2 = table.add_computed(format!("src2_addr_{i}"), src2_abs + B32::from(i as u32));
        }

        // Transpose src1 and src2 values to get the correct B32 lookups in the VROM.
        let src1_transposition = TransposeColumns::new(&mut table, src1_vals);
        let src2_transposition = TransposeColumns::new(&mut table, src2_vals);
        let src1_vals_packed = src1_transposition.output;
        let src2_vals_packed = src2_transposition.output;

        // Pull the first and second source values from the VROM channel.
        for i in 0..16 {
            table.pull(vrom_channel, [src1_addresses[i], src1_vals_packed[i]]);
            table.pull(vrom_channel, [src2_addresses[i], src2_vals_packed[i]]);
        }

        // p_state_in = src1_val XOR src2_val.
        let p_state_in: [Col<binius_field::BinaryField8b, 8>; 8] =
            from_fn(|i| table.add_computed("state_in", src1_vals[i] + src2_vals[i]));

        let p_op = Permutation::new(
            &mut table,
            binius_m3::gadgets::hash::groestl::PermutationVariant::P,
            p_state_in,
        );

        // p_out = P(p_state_in)
        let p_out_array = p_op.state_out();

        // Carry out the Q permutation.
        let q_op = Permutation::new(
            &mut table,
            binius_m3::gadgets::hash::groestl::PermutationVariant::Q,
            src2_vals,
        );

        // q_out = Q(src2_val)
        let q_out_array = q_op.state_out();

        // interm = p_out XOR src1_val.
        let interm: [Col<B8, 8>; 8] =
            from_fn(|i| table.add_computed(format!("interm_{i}"), p_out_array[i] + src1_vals[i]));

        // Compute the final output: out = Q(m) XOR P(src1_val XOR src2_val) XOR
        // out = interm XOR q_out.
        let out: [Col<B8, 8>; 8] =
            from_fn(|i| table.add_computed(format!("out_{i}"), interm[i] + q_out_array[i]));

        // TODO: Adapt the gadget to make the output shape more generic.
        // We transpose the output. We do not use the transposition gadget as the output
        // shape is a bit different.
        let projected_out_temp: [[Col<B8>; 8]; 8] = from_fn(|i| {
            from_fn(|j| {
                table.add_selected_block::<_, 8, 1>(
                    format!("compress_projected_out_{i}_{j}"),
                    out[i],
                    j,
                )
            })
        });

        // Then we get the elements in the correct order.
        let projected_out = from_fn(|i| from_fn(|j| projected_out_temp[j][i]));

        let zero_padded_out = from_fn(|i| {
            from_fn(|j| {
                table.add_zero_pad::<_, 1, 8>(
                    format!("compress_zero_padded_out_{i}_{j}"),
                    projected_out[i][j],
                    j,
                )
            })
        });

        // Finally, we sum each array of B8 to get the correct values.
        let transposed_out: [Col<B8, 8>; 8] = zero_padded_out
            .iter()
            .enumerate()
            .map(|(i, cols)| {
                let expr: Expr<B8, 8> = cols
                    .iter()
                    .map(|&col| col.into())
                    .reduce(|acc, item| acc + item)
                    .unwrap();
                table.add_computed(format!("zero_padded_sums_out_{i}"), expr)
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let out_packed: [Col<B32, 2>; 8] =
            from_fn(|i| table.add_packed("dst_val_packed", transposed_out[i]));

        // Get the base address for the destination value.
        let dst_abs = table.add_computed("dst", state_cols.fp + upcast_col(state_cols.arg0));
        let mut dst_addresses = [dst_abs; 8];
        for (i, d) in dst_addresses.iter_mut().enumerate().skip(1) {
            *d = table.add_computed(format!("dst_addr_{i}"), dst_abs + B32::from(2 * i as u32));
        }
        let mut dst_addresses_plus_one = [dst_abs; 8];
        for (i, d_plus_one) in dst_addresses_plus_one.iter_mut().enumerate() {
            *d_plus_one = table.add_computed(
                format!("dst_addr_plus_one_{i}"),
                dst_abs + B32::from(2 * i as u32 + 1),
            );
        }

        // Pull the first source value from the VROM channel.
        let dst_selected: [Col<B32>; 16] = (0..16)
            .map(|i| table.add_selected(format!("dst_val_{i}_selected"), out_packed[i / 2], i % 2))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        for i in 0..8 {
            table.pull(vrom_channel, [dst_addresses[i], dst_selected[2 * i]]);
            table.pull(
                vrom_channel,
                [dst_addresses_plus_one[i], dst_selected[2 * i + 1]],
            );
        }

        let dst_vals = from_fn(|i| table.add_packed("dst_val_packed", out_packed[i]));

        Self {
            id: table.id(),
            state_cols,
            dst_addresses,
            dst_vals,
            dst_addresses_plus_one,
            dst_selected,
            src1_addresses,
            src1_vals,
            src1_transposition,
            src2_addresses,
            src2_vals,
            src2_transposition,
            interm,
            out,
            projected_out,
            zero_padded_out,
            p_op,
            q_op,
        }
    }
}

impl TableFiller<ProverPackedField> for Groestl256CompressTable {
    type Event = Groestl256CompressEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        let p_states = rows
            .clone()
            .map(|event| {
                let mut p_state = [B8::ZERO; 64];
                for (i, p_s) in p_state.iter_mut().enumerate() {
                    *p_s = B8::from(event.src1_val[i]) + B8::from(event.src2_val[i]);
                }
                p_state
            })
            .collect::<Vec<_>>();

        {
            let mut dst_addresses = (0..8)
                .map(|i| witness.get_mut_as(self.dst_addresses[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut dst_addresses_plus_one = (0..8)
                .map(|i| witness.get_mut_as(self.dst_addresses_plus_one[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut dst_selected = (0..16)
                .map(|i| witness.get_mut_as(self.dst_selected[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut dst_vals = (0..8)
                .map(|i| witness.get_mut_as(self.dst_vals[i]))
                .collect::<Result<Vec<RefMut<'_, [u64]>>, _>>()?;

            let mut src1_addresses = (0..16)
                .map(|i| witness.get_mut_as(self.src1_addresses[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut src1_vals = (0..8)
                .map(|i| witness.get_mut(self.src1_vals[i]))
                .collect::<Result<Vec<_>, _>>()?;

            let mut src2_addresses = (0..16)
                .map(|i| witness.get_mut_as(self.src2_addresses[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut src2_vals = (0..8)
                .map(|i| witness.get_mut(self.src2_vals[i]))
                .collect::<Result<Vec<_>, _>>()?;

            let mut interm = (0..8)
                .map(|i| witness.get_mut_as(self.interm[i]))
                .collect::<Result<Vec<RefMut<'_, [[u8; 8]]>>, _>>()?;

            let mut out = (0..8)
                .map(|i| witness.get_mut(self.out[i]))
                .collect::<Result<Vec<_>, _>>()?;
            let mut projected_out = (0..64)
                .map(|i| witness.get_mut_as(self.projected_out[i / 8][i % 8]))
                .collect::<Result<Vec<RefMut<'_, [u8]>>, _>>()?;
            let mut zero_padded_out = (0..64)
                .map(|i| witness.get_mut_as(self.zero_padded_out[i / 8][i % 8]))
                .collect::<Result<Vec<RefMut<'_, [[u8; 8]]>>, _>>()?;

            for (i, event) in rows.clone().enumerate() {
                let dst_base_addr = event.fp.addr(event.dst as u32);
                let src1_base_addr = event.fp.addr(event.src1 as u32);
                let src2_base_addr = event.fp.addr(event.src2 as u32);

                for j in 0..8 {
                    // Fill addresses.
                    dst_addresses[j][i] = dst_base_addr + 2 * j as u32;
                    dst_addresses_plus_one[j][i] = dst_base_addr + 2 * j as u32 + 1;
                    src1_addresses[2 * j][i] = src1_base_addr + 2 * j as u32;
                    src1_addresses[2 * j + 1][i] = src1_base_addr + 2 * j as u32 + 1;
                    src2_addresses[2 * j][i] = src2_base_addr + 2 * j as u32;
                    src2_addresses[2 * j + 1][i] = src2_base_addr + 2 * j as u32 + 1;

                    // Fill source and destination values.
                    let dst_val_u8 = u64_to_bytes(&event.dst_val);
                    dst_vals[j][i] = event.dst_val[j];

                    // The permutation takes the input in row-major order.
                    for k in 0..8 {
                        set_packed_slice(
                            &mut src1_vals[j],
                            i * 8 + k,
                            B8::from(event.src1_val[k * 8 + j]),
                        );
                        set_packed_slice(
                            &mut src2_vals[j],
                            i * 8 + k,
                            B8::from(event.src2_val[k * 8 + j]),
                        );
                        set_packed_slice(&mut out[j], i * 8 + k, B8::from(dst_val_u8[k * 8 + j]));

                        projected_out[j * 8 + k][i] = dst_val_u8[j * 8 + k];
                        zero_padded_out[j * 8 + k][i][k] = projected_out[j * 8 + k][i];
                    }
                }

                let transposed_src1_val = (0..8)
                    .flat_map(|j| (0..8).map(move |k| event.src1_val[k * 8 + j]))
                    .collect::<Vec<_>>();

                let dst_val_u32: [u32; 16] = u64_to_u32(&event.dst_val).try_into().unwrap();

                // Fill the source and destination 32-bit values.
                for j in 0..16 {
                    dst_selected[j][i] = dst_val_u32[j];
                }

                // We want to get the output of the P permutation. For this, we first need to
                // reshape the input and change its basis.
                let p_state_bytes = p_states[i]
                    .iter()
                    .map(|&b8| AESTowerField8b::from(b8).val())
                    .collect::<Vec<_>>();

                let mut p_state_bytes =
                    GroestlShortImpl::state_from_bytes(&p_state_bytes.try_into().unwrap());
                GroestlShortImpl::p_perm(&mut p_state_bytes);

                // Reshape the output to get the expected output of the P permutation.
                let p_out = GroestlShortImpl::state_to_bytes(&p_state_bytes);
                let p_out = p_out.map(|byte| B8::from(AESTowerField8b::new(byte)));
                let p_out = (0..8)
                    .map(|j| (0..8).map(|k| p_out[k * 8 + j]).collect::<Vec<_>>())
                    .collect::<Vec<_>>();

                for j in 0..8 {
                    // Fill interm = p_out XOR src1_val.
                    interm[j][i] = p_out[j]
                        .iter()
                        .zip(transposed_src1_val[j * 8..(j + 1) * 8].iter())
                        .map(|(&a, &b)| a.val() ^ b)
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();
                }
            }
        }

        // Populate the transposition columns.
        let src1_rows = rows.clone().map(|event| event.src1_val);
        let src2_rows = rows.clone().map(|event| event.src2_val);
        self.src1_transposition.populate(witness, src1_rows)?;
        self.src2_transposition.populate(witness, src2_rows)?;

        // Populate the P and Q permutations.
        // First, populate the P permutation state inputs.
        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.state_cols.populate(witness, state_rows)?;

        self.p_op.populate_state_in(witness, p_states.iter())?;
        // Populate the P permutation.
        self.p_op.populate(witness)?;

        // Populate the Q permutation. We don't have to populate the input of the Q
        // permutation, as it is already done by populating src2_vals.
        self.q_op.populate(witness)?;

        Ok(())
    }
}

/// GROEST256_OUTPUT table.
///
/// This table handles the GROEST256_OUTPUT instruction, which returns the
/// 2-to-1 compression output.
pub struct Groestl256OutputTable {
    id: TableId,
    state_cols: StateColumns<GROESTL_OUTPUT_OPCODE>,
    // All addresses where we need to read the values for dst.
    dst_addrs: [Col<B32>; 4],
    dst_addrs_plus_one: [Col<B32>; 4],
    dst_vals: [Col<B64>; 4],
    dst_selected: [Col<B32>; 8],
    // All addresses where we need to read the values for src1.
    src1_addrs: [Col<B32>; 8],
    // First 256 bits of the input state.
    src1_vals: [Col<B8, 8>; 4],
    // All addresses where we need to read the values for src2.
    src2_addrs: [Col<B32>; 8],
    // Last 256 bits of the input state.
    src2_vals: [Col<B8, 8>; 4],
    // We need to write all 4 words to the VROM channel.
    state_in_transposition: TransposeColumns,
    // Output of the P permutation.
    out: [Col<B8, 8>; 8],
    projected_out: [[Col<B8>; 8]; 8],
    zero_padded_out: [[Col<B8, 8>; 8]; 4],
    // P permutation.
    p_op: Permutation,
}

impl Table for Groestl256OutputTable {
    type Event = Groestl256OutputEvent;

    fn name(&self) -> &'static str {
        "Groestl256Output"
    }

    fn new(cs: &mut ConstraintSystem, channels: &Channels) -> Self {
        let mut table = cs.add_table("Groestl256Output");

        let Channels {
            state_channel,
            prom_channel,
            vrom_channel,
            ..
        } = *channels;

        let state_cols = StateColumns::new(
            &mut table,
            state_channel,
            prom_channel,
            StateColumnsOptions {
                next_pc: NextPc::Increment,
                next_fp: None,
            },
        );

        // Get destination and source values.
        let src1_vals = from_fn(|i| table.add_committed(format!("src1_val_{i}")));
        let src2_vals = from_fn(|i| table.add_committed(format!("src2_val_{i}")));

        // Get the base address for the first source value.
        let src1_base_addr = state_cols.fp + upcast_col(state_cols.arg1);
        let src1_addrs = from_fn(|i| {
            table.add_computed(
                format!("src1_addr_{i}"),
                src1_base_addr.clone() + B32::from(i as u32),
            )
        });

        // Get the base address for the second source value.
        let src2_base_addr = state_cols.fp + upcast_col(state_cols.arg2);
        let src2_addrs = from_fn(|i| {
            table.add_computed(
                format!("src2_addr_{i}"),
                src2_base_addr.clone() + B32::from(i as u32),
            )
        });

        let state_in: [Col<B8, 8>; 8] = [src1_vals, src2_vals].concat().try_into().unwrap();

        let state_in_transposition = TransposeColumns::new(&mut table, state_in);
        let state_in_packed = state_in_transposition.output;

        // Pull the first and second source values from the VROM channel.
        for i in 0..8 {
            table.pull(vrom_channel, [src1_addrs[i], state_in_packed[i]]);
            table.pull(vrom_channel, [src2_addrs[i], state_in_packed[8 + i]]);
        }

        // Carry out the P permutation.
        let p_op = Permutation::new(
            &mut table,
            binius_m3::gadgets::hash::groestl::PermutationVariant::P,
            state_in,
        );

        // p_out = P(p_state_in)
        let p_out_array = p_op.state_out();

        // XOR with state_in and only return the lower 256 bits (so the first 32 bytes).
        let out: [Col<B8, 8>; 8] =
            from_fn(|i| table.add_computed(format!("out_{i}"), p_out_array[i] + state_in[i]));

        // TODO: Adapt the gadget to make the output shape more generic.
        // We transpose the output. We do not use the transposition gadget as the output
        // shape is a bit different.
        let projected_out_temp: [[Col<B8>; 8]; 8] = from_fn(|i| {
            from_fn(|j| {
                table.add_selected_block::<_, 8, 1>(
                    format!("output_projected_out_{i}_{j}"),
                    out[i],
                    j,
                )
            })
        });

        // Then we get the elements in the correct order.
        let projected_out = from_fn(|i| from_fn(|j| projected_out_temp[j][i]));

        let zero_padded_out = from_fn(|i| {
            from_fn(|j| {
                table.add_zero_pad::<_, 1, 8>(
                    format!("output_zero_padded_out_{i}_{j}"),
                    projected_out[4 + i][j],
                    j,
                )
            })
        });

        // Finally, we sum each array of B8 to get the correct values.
        let transposed_out: [Col<B8, 8>; 4] = zero_padded_out
            .iter()
            .enumerate()
            .map(|(i, cols)| {
                let expr: Expr<B8, 8> = cols
                    .iter()
                    .map(|&col| col.into())
                    .reduce(|acc, item| acc + item)
                    .unwrap();
                table.add_computed(format!("zero_padded_sums_out_{i}"), expr)
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let out_packed: [Col<B32, 2>; 4] =
            from_fn(|i| table.add_packed("dst_val_packed", transposed_out[i]));

        // Get the base address for the destination value.
        let dst_abs = table.add_computed("dst", state_cols.fp + upcast_col(state_cols.arg0));
        let mut dst_addrs = [dst_abs; 4];
        for (i, d) in dst_addrs.iter_mut().enumerate().skip(1) {
            *d = table.add_computed(format!("dst_addr_{i}"), dst_abs + B32::from(2 * i as u32));
        }
        let mut dst_addrs_plus_one = [dst_abs; 4];
        for (i, d_plus_one) in dst_addrs_plus_one.iter_mut().enumerate() {
            *d_plus_one = table.add_computed(
                format!("dst_addr_plus_one_{i}"),
                dst_abs + B32::from(2 * i as u32 + 1),
            );
        }

        // Pull the first source value from the VROM channel.
        let dst_selected: [Col<B32>; 8] = (0..8)
            .map(|i| table.add_selected(format!("dst_val_{i}_selected"), out_packed[i / 2], i % 2))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        for i in 0..4 {
            table.pull(vrom_channel, [dst_addrs[i], dst_selected[2 * i]]);
            table.pull(
                vrom_channel,
                [dst_addrs_plus_one[i], dst_selected[2 * i + 1]],
            );
        }

        let dst_vals = from_fn(|i| table.add_packed("dst_val_packed", out_packed[i]));

        Self {
            id: table.id(),
            state_cols,
            dst_addrs,
            dst_addrs_plus_one,
            dst_vals,
            dst_selected,
            src1_addrs,
            src1_vals,
            src2_addrs,
            src2_vals,
            state_in_transposition,
            projected_out,
            zero_padded_out,
            p_op,
            out,
        }
    }
}

impl TableFiller<ProverPackedField> for Groestl256OutputTable {
    type Event = Groestl256OutputEvent;

    fn id(&self) -> TableId {
        self.id
    }

    fn fill<'a>(
        &self,
        rows: impl Iterator<Item = &'a Self::Event> + Clone,
        witness: &'a mut TableWitnessSegment<ProverPackedField>,
    ) -> Result<(), anyhow::Error> {
        let p_states = rows
            .clone()
            .map(|event| {
                let mut p_state = [B8::ZERO; 64];
                for i in 0..32 {
                    p_state[i] = B8::from(event.src1_val[i]);
                    p_state[i + 32] = B8::from(event.src2_val[i]);
                }
                p_state
            })
            .collect::<Vec<_>>();
        {
            let mut dst_addrs = (0..4)
                .map(|i| witness.get_mut_as(self.dst_addrs[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut dst_vals = (0..4)
                .map(|i| witness.get_mut_as(self.dst_vals[i]))
                .collect::<Result<Vec<RefMut<'_, [u64]>>, _>>()?;
            let mut dst_addrs_plus_one = (0..4)
                .map(|i| witness.get_mut_as(self.dst_addrs_plus_one[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut dst_selected = (0..8)
                .map(|i| witness.get_mut_as(self.dst_selected[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;

            let mut src1_addrs = (0..8)
                .map(|i| witness.get_mut_as(self.src1_addrs[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut src1_vals = (0..4)
                .map(|i| witness.get_mut(self.src1_vals[i]))
                .collect::<Result<Vec<_>, _>>()?;
            let mut src2_addrs = (0..8)
                .map(|i| witness.get_mut_as(self.src2_addrs[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut src2_vals = (0..4)
                .map(|i| witness.get_mut(self.src2_vals[i]))
                .collect::<Result<Vec<_>, _>>()?;

            let mut out = (0..8)
                .map(|i| witness.get_scalars_mut(self.out[i]))
                .collect::<Result<Vec<_>, _>>()?;
            let mut projected_out = (0..64)
                .map(|i| witness.get_mut_as(self.projected_out[i / 8][i % 8]))
                .collect::<Result<Vec<RefMut<'_, [u8]>>, _>>()?;
            let mut zero_padded_out = (0..32)
                .map(|i| witness.get_mut_as(self.zero_padded_out[i / 8][i % 8]))
                .collect::<Result<Vec<RefMut<'_, [[u8; 8]]>>, _>>()?;

            for (i, event) in rows.clone().enumerate() {
                let dst_base_addr = event.fp.addr(event.dst as u32);
                let src1_base_addr = event.fp.addr(event.src1 as u32);
                let src2_base_addr = event.fp.addr(event.src2 as u32);

                let full_state_in: [u8; 64] = [event.src1_val, event.src2_val]
                    .concat()
                    .try_into()
                    .unwrap();

                for j in 0..4 {
                    dst_addrs[j][i] = dst_base_addr + 2 * j as u32;
                    dst_addrs_plus_one[j][i] = dst_base_addr + 2 * j as u32 + 1;
                    src1_addrs[2 * j][i] = src1_base_addr + 2 * j as u32;
                    src1_addrs[2 * j + 1][i] = src1_base_addr + 2 * j as u32 + 1;
                    src2_addrs[2 * j][i] = src2_base_addr + 2 * j as u32;
                    src2_addrs[2 * j + 1][i] = src2_base_addr + 2 * j as u32 + 1;

                    dst_vals[j][i] = event.dst_val[j];

                    let dst_val_u8 = u64_to_bytes(&event.dst_val);

                    for k in 0..8 {
                        set_packed_slice(
                            &mut src1_vals[j],
                            i * 8 + k,
                            B8::from(event.src1_val[k * 4 + j]),
                        );
                        set_packed_slice(
                            &mut src2_vals[j],
                            i * 8 + k,
                            B8::from(event.src2_val[k * 4 + j]),
                        );
                        set_packed_slice(&mut out[j], i * 8 + k, B8::from(dst_val_u8[k * 4 + j]));

                        projected_out[(j + 4) * 8 + k][i] = dst_val_u8[j * 8 + k];
                        zero_padded_out[j * 8 + k][i][k] = projected_out[(j + 4) * 8 + k][i];
                    }
                }

                let dst_val_u32: [u32; 8] = u64_to_u32(&event.dst_val).try_into().unwrap();

                // Fill the source and destination 32-bit values.
                for j in 0..8 {
                    dst_selected[j][i] = dst_val_u32[j];
                }

                // Compute the output of the P permutation.
                let p_state_bytes = p_states[i]
                    .iter()
                    .map(|&b8| AESTowerField8b::from(b8).val())
                    .collect::<Vec<_>>();
                let p_state_bytes =
                    GroestlShortImpl::state_from_bytes(&p_state_bytes.try_into().unwrap());
                let mut state = p_state_bytes;
                GroestlShortImpl::p_perm(&mut state);

                // Reshape the output to get the expected output.
                let p_out = GroestlShortImpl::state_to_bytes(&state);
                let p_out = p_out.map(|byte| B8::from(AESTowerField8b::new(byte)));
                let p_out = (0..8)
                    .map(|j| (0..8).map(|k| p_out[k * 8 + j]).collect::<Vec<_>>())
                    .collect::<Vec<_>>();

                for j in 0..8 {
                    for k in 0..8 {
                        // Fill interm = p_out XOR src1_val.
                        out[j][i * 8 + k] = p_out[j][k] + B8::from(full_state_in[k * 8 + j]);
                        projected_out[k * 8 + j][i] = out[j][i * 8 + k].val();
                    }
                }
            }
        }

        // Populate the transposition columns.
        let full_state_in_rows = rows.clone().map(|event| {
            [event.src1_val, event.src2_val]
                .concat()
                .try_into()
                .unwrap()
        });
        self.state_in_transposition
            .populate(witness, full_state_in_rows)?;

        // First, we need to populate the P permutation state inputs.
        self.p_op.populate_state_in(witness, p_states.iter())?;
        // Populate the P permutation.
        self.p_op.populate(witness)?;

        let state_rows = rows.map(|event| StateGadget {
            pc: event.pc.into(),
            next_pc: None,
            fp: *event.fp,
            arg0: event.dst,
            arg1: event.src1,
            arg2: event.src2,
        });
        self.state_cols.populate(witness, state_rows)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use petravm_asm::isa::RecursionISA;
    use proptest::prelude::*;

    use crate::{prover::Prover, test_utils::generate_groestl_ret_trace};

    fn test_groestl_with_values(src1_value: [u32; 16], src2_value: [u32; 16]) -> Result<()> {
        let trace = generate_groestl_ret_trace(src1_value, src2_value)?;
        trace.validate()?;
        assert_eq!(trace.groestl_compress_events().len(), 1);
        assert_eq!(trace.groestl_output_events().len(), 1);
        Prover::new(Box::new(RecursionISA)).validate_witness(&trace)
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_groestl(
            src1_value in  any::<[u32; 16]>(),
            src2_value in  any::<[u32; 16]>(),
        ) {
            prop_assert!(test_groestl_with_values(src1_value, src2_value).is_ok());
        }
    }
}

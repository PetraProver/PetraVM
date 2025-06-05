use std::{array::from_fn, cell::RefMut};

use binius_field::packed::set_packed_slice;
use binius_field::AESTowerField8b;
use binius_field::Field;
use binius_hash::groestl::GroestlShortImpl;
use binius_hash::groestl::GroestlShortInternal;
use binius_m3::builder::TableBuilder;
use binius_m3::builder::B1;
use binius_m3::builder::{Expr, B64};
use binius_m3::{
    builder::{
        upcast_col, Col, ConstraintSystem, TableFiller, TableId, TableWitnessSegment, B32, B8,
    },
    gadgets::hash::groestl::Permutation,
};
use petravm_asm::util::u32_to_bytes;
use petravm_asm::{Groestl256CompressEvent, Groestl256OutputEvent, Opcode};

use crate::gadgets::aes_to_bin::AesToBinTransformColumns;
use crate::gadgets::aes_to_bin::BinToAesTransformColumns;
use crate::gadgets::multiple_lookup::{MultipleLookupColumns, MultipleLookupGadget};
use crate::gadgets::transpose::TransposeColumns;
use crate::{
    channels::Channels,
    gadgets::state::{NextPc, StateColumns, StateColumnsOptions, StateGadget},
    table::Table,
    types::ProverPackedField,
};

const GROESTL_COMPRESS_OPCODE: u16 = Opcode::Groestl256Compress as u16;
const GROESTL_OUTPUT_OPCODE: u16 = Opcode::Groestl256Output as u16;

/// GROESTL256_COMPRESS table.
///
/// This table handles the GROESTL256_COMPRESS instruction, which performs the
/// compression function described in the Groestl specs.
/// (see Section 3.2 of <https://www.groestl.info/Groestl.pdf>)
///
/// Note that the P/Q permutation gadgets take the transposed input matrix
/// compared to the actual Groestl specs. The first source value comes from the
/// previous Groestl compression gadged, and is therefore already transposed.
/// But we need to tranpose the second source value.
pub struct Groestl256CompressTable {
    id: TableId,
    state_cols: StateColumns<GROESTL_COMPRESS_OPCODE>,
    /// Base address.
    dst_addresses: [Col<B32>; 8],
    /// Destination values.
    dst_vals: [Col<B64>; 8],
    src1_vals: [Col<B8, 8>; 8],
    /// Base address.
    src1_addresses: [Col<B32>; 8],
    /// Columns to carry out the lookups from the VROM for the first source
    /// values.
    src1_lookups: [MultipleLookupColumns<2>; 8],
    src2_addresses: [Col<B32>; 16],
    /// Columns needed for transposing src2.
    src2_transposition: TransposeColumns,
    /// Columns for lookup up the output values in the VROM. Note that the
    /// output values are transposed compared to the Groestl specs.
    dst_vals_lookup: [MultipleLookupColumns<2>; 8],
    /// P permutation.
    p_op: Permutation,
    /// Q permutation.
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
        // Get source values.
        let src1_vals: [Col<B8, 8>; 8] = from_fn(|i| table.add_committed(format!("src1_val_{i}")));
        let src2_vals = from_fn(|i| table.add_committed(format!("src2_val_{i}")));

        // Get the base address for the first and second source values.
        let src1_base_addr = state_cols.fp + upcast_col(state_cols.arg1);
        let src2_base_addr = state_cols.fp + upcast_col(state_cols.arg2);

        // Get all the addresses for the first and second source values.
        let src1_addresses = get_half_addresses(&mut table, src1_base_addr, "src1_addr");
        let src2_addresses = get_all_addresses(&mut table, src2_base_addr, "src2_addr");

        // Transpose src2 values to get the correct B32 lookups in the VROM.
        let src2_transposition = TransposeColumns::new(&mut table, src2_vals);
        let src2_vals_packed = src2_transposition.output;

        // Pull the second source values from the VROM channel.
        for i in 0..16 {
            table.pull(vrom_channel, [src2_addresses[i], src2_vals_packed[i]]);
        }

        // Lookup the first source values without transposition.
        let src1_vals_packed: [Col<B32, 2>; 8] =
            from_fn(|i| table.add_packed("groestl_output_src1_vals_packed", src1_vals[i]));

        let src1_lookups = from_fn(|i| {
            MultipleLookupColumns::<2>::new(
                &mut table,
                vrom_channel,
                src1_addresses[i],
                src1_vals_packed[i],
                "groestl_output_src1_lookup",
            )
        });

        // p_state_in = src1_val XOR src2_val.
        let p_state_in: [Col<B8, 8>; 8] =
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

        // out = p_out XOR src1_val XOR q_out.
        let out: [Col<B8, 8>; 8] = from_fn(|i| {
            table.add_computed(
                format!("out_{i}"),
                p_out_array[i] + src1_vals[i] + q_out_array[i],
            )
        });

        let dst_vals_lookup_packed: [Col<B32, 2>; 8] =
            from_fn(|i| table.add_packed("packed_out", out[i]));

        // Lookup the output values.
        let dst_base_addr = state_cols.fp + upcast_col(state_cols.arg0);
        let dst_addresses = get_half_addresses(&mut table, dst_base_addr, "dst_addr");

        let dst_vals_lookup = (0..8)
            .flat_map(|i| {
                [MultipleLookupColumns::<2>::new(
                    &mut table,
                    vrom_channel,
                    dst_addresses[i],
                    dst_vals_lookup_packed[i],
                    "dst_vals_lookup",
                )]
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("state_in_lookup has exactly 8 elements");

        // Pack the output values to get the final destination values.
        let dst_vals = from_fn(|i| table.add_packed("dst_vals", dst_vals_lookup_packed[i]));

        Self {
            id: table.id(),
            state_cols,
            dst_addresses,
            dst_vals,
            src1_vals,
            src1_addresses,
            src1_lookups,
            src2_addresses,
            src2_transposition,
            dst_vals_lookup,
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
                let transposed_src1_val: [u8; 64] = (0..8)
                    .flat_map(|j| (0..8).map(move |k| event.src1_val[k * 8 + j]))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap();
                for (i, p_s) in p_state.iter_mut().enumerate() {
                    *p_s = B8::from(transposed_src1_val[i]) + B8::from(event.src2_val[i]);
                }
                p_state
            })
            .collect::<Vec<_>>();

        {
            let mut dst_addresses = (0..8)
                .map(|i| witness.get_mut_as(self.dst_addresses[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut src1_addresses = (0..8)
                .map(|i| witness.get_mut_as(self.src1_addresses[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut src2_addresses = (0..16)
                .map(|i| witness.get_mut_as(self.src2_addresses[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;

            let mut src1_vals = (0..8)
                .map(|i| witness.get_mut(self.src1_vals[i]))
                .collect::<Result<Vec<_>, _>>()?;

            let mut dst_vals = (0..8)
                .map(|i| witness.get_mut_as(self.dst_vals[i]))
                .collect::<Result<Vec<RefMut<'_, [u64]>>, _>>()?;

            for (i, event) in rows.clone().enumerate() {
                let dst_base_addr = event.fp.addr(event.dst as u32);
                let src1_base_addr = event.fp.addr(event.src1 as u32);
                let src2_base_addr = event.fp.addr(event.src2 as u32);

                for j in 0..8 {
                    // Fill addresses.
                    dst_addresses[j][i] = dst_base_addr + 2 * j as u32;
                    src1_addresses[j][i] = src1_base_addr + 2 * j as u32;
                    // src1_addresses[2 * j + 1][i] = src1_base_addr + 2 * j as u32 + 1;
                    src2_addresses[2 * j][i] = src2_base_addr + 2 * j as u32;
                    src2_addresses[2 * j + 1][i] = src2_base_addr + 2 * j as u32 + 1;

                    // Fill out the destination values.
                    dst_vals[j][i] = event.dst_val[j];

                    for k in 0..8 {
                        set_packed_slice(
                            &mut src1_vals[j],
                            i * 8 + k,
                            B8::from(event.src1_val[j * 8 + k]),
                        );
                    }
                }

                // We want to get the output of the P permutation. For this, we first need to
                // reshape the input and change its basis.
                let p_state_bytes = p_states[i]
                    .iter()
                    .map(|&b8| AESTowerField8b::from(b8).val())
                    .collect::<Vec<_>>();

                let mut p_state_bytes = GroestlShortImpl::state_from_bytes(
                    &p_state_bytes
                        .try_into()
                        .expect("p_state_bytes has exactly 64 elements"),
                );
                GroestlShortImpl::p_perm(&mut p_state_bytes);
            }
        }

        let src1_iters = (0..8)
            .map(|i| {
                rows.clone().map(move |ev| {
                    let vals = ev.src1_val[i * 8..(i + 1) * 8]
                        .chunks_exact(4)
                        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();

                    MultipleLookupGadget {
                        addr: ev.fp.addr(ev.src1) + 2 * i as u32,
                        vals,
                    }
                })
            })
            .collect::<Vec<_>>();

        for (i, s_i) in src1_iters.iter().enumerate().take(8) {
            self.src1_lookups[i].populate(witness, s_i.clone())?;
        }

        // Populate the transposition columns.
        let src2_rows = rows.clone().map(|event| event.src2_val);

        let dst_vals_iters = (0..8)
            .map(|i| {
                rows.clone().map(move |event| {
                    let dst_vals: [u32; 2] =
                        [event.dst_val[i] as u32, (event.dst_val[i] >> 32) as u32];
                    MultipleLookupGadget {
                        addr: event.fp.addr(event.dst + 2 * i as u16),
                        vals: dst_vals,
                    }
                })
            })
            .collect::<Vec<_>>();

        for (i, d_i) in dst_vals_iters.iter().enumerate().take(8) {
            self.dst_vals_lookup[i].populate(witness, d_i.clone())?;
        }

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

/// GROESTL256_OUTPUT table.
///
/// This table handles the GROESTL256_OUTPUT instruction, which returns the
/// 2-to-1 output transformation.
/// (see Section 3.3 of <https://www.groestl.info/Groestl.pdf>)
///
/// Note that the P/Q permutation gadgets take the transposed input matrix
/// compared to the actual Groestl specs. It is therefore necessary to transpose
/// the input we pull from the VROM.
///
/// The state we receive is the output of a previous `Groestl256Compress`, and
/// is therefore already transposed compared to the Groestl specs. Furthermore,
/// for the same reason, the transformation from the AES to the binary basis has
/// been applied. So the last step of the gadget is to apply the transformation
/// from bonary to AES basis, to agree with the Groestl specs final output.
pub struct Groestl256OutputTable {
    id: TableId,
    state_cols: StateColumns<GROESTL_OUTPUT_OPCODE>,
    /// All addresses where we need to read the values for dst.
    dst_addrs: [Col<B32>; 8],
    /// Destination values.
    dst_vals: [Col<B32>; 8],
    /// All addresses where we need to read the values for src1.
    src1_addrs: [Col<B32>; 4],
    /// All addresses where we need to read the values for src2.
    src2_addrs: [Col<B32>; 4],
    /// Columns to lookup the values for src1 and src2 from the VROM.
    src1_lookup: [MultipleLookupColumns<2>; 4],
    src2_lookup: [MultipleLookupColumns<2>; 4],
    /// Output of the P permutation, in the binary basis.
    out: [Col<B8, 8>; 8],
    /// Columns to get the output of the P permutation, in the AES basis.
    out_aes_columns: [BinToAesTransformColumns; 8],
    out_bits: [Col<B1, 64>; 8],
    /// `projected_out` and `zero_padded_out` are the columns needed for
    /// transposing the output, and getting the B32 values we can pull from
    /// the VROM.
    projected_out: [Col<B8>; 64],
    zero_padded_out: [Col<B8, 4>; 32],
    /// P permutation.
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
        let src1_vals: [Col<B8, 8>; 4] = from_fn(|i| table.add_committed(format!("src1_val_{i}")));
        let src2_vals = from_fn(|i| table.add_committed(format!("src2_val_{i}")));

        // Get the base address for the first source value.
        let src1_base_addr = state_cols.fp + upcast_col(state_cols.arg1);
        let src1_addrs = get_half_addresses(&mut table, src1_base_addr, "src1_addr");

        // Get the base address for the second source value.
        let src2_base_addr = state_cols.fp + upcast_col(state_cols.arg2);
        let src2_addrs = get_half_addresses(&mut table, src2_base_addr, "src2_addr");

        let state_in: [Col<B8, 8>; 8] = [src1_vals, src2_vals]
            .concat()
            .try_into()
            .expect("src1_vals and src2_vals have exactly 4 elements each");

        // Pack the values so we get the correct B32 lookups in the VROM.
        let state_in_packed: [Col<B32, 2>; 8] =
            from_fn(|i| table.add_packed("state_in_packed", state_in[i]));

        let src1_lookup = from_fn(|i| {
            MultipleLookupColumns::<2>::new(
                &mut table,
                vrom_channel,
                src1_addrs[i],
                state_in_packed[i],
                "src1_lookup",
            )
        });
        let src2_lookup = from_fn(|i| {
            MultipleLookupColumns::<2>::new(
                &mut table,
                vrom_channel,
                src2_addrs[i],
                state_in_packed[4 + i],
                "src2_lookup",
            )
        });

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

        // Go fro the binary basis to the AES basis.
        let out_bits: [Col<B1, 64>; 8] = from_fn(|i| table.add_committed(format!("out_bits_{i}")));
        let out_aes_columns = from_fn(|i| {
            // We need to convert the B8 values into AESTowerField values.
            BinToAesTransformColumns::new(&mut table, out_bits[i], "groestl_output_out_aes")
        });
        let out_aes: [Col<B8, 8>; 8] = out_aes_columns
            .iter()
            .map(|col| col.reshaped_outputs)
            .collect::<Vec<_>>()
            .try_into()
            .expect("out_aes_columns has exactly 8 elements");

        // Check that the bits of the output match the expected values.
        let out_bits_packed: [Col<B8, 8>; 8] =
            from_fn(|i| table.add_packed("out_bits_packed", out_bits[i]));
        out_bits_packed
            .iter()
            .zip(out.iter())
            .for_each(|(&packed, &o)| table.assert_zero("Check out_bits_packed", packed - o));

        // We transpose the output and pack it into B32s so we can read the elements
        // form the VROM. We do not use the transposition gadget since we can truncate
        // the output early on here.
        // First, we project the `Col<B8, 8>` into independent `Col<B8>` columns.
        let projected_out_temp: [[Col<B8>; 8]; 8] = from_fn(|i| {
            from_fn(|j| table.add_selected(format!("output_projected_out_{i}_{j}"), out_aes[i], j))
        });

        // Then we get the elements in the correct order (we transpose the matrix).
        let projected_out = from_fn(|i| projected_out_temp[i % 8][i / 8]);

        // We truncate the output by only getting the final 32 bytes, and we zero-pad
        // from `Col<B8>` into `Col<B8, 4>`.
        let zero_padded_out = from_fn(|i| {
            table.add_zero_pad::<_, 1, 4>(
                format!("output_zero_padded_out_{i}"),
                projected_out[32 + i],
                i % 4,
            )
        });

        // We sum each array of `Col<B8, 4>` to get the correct values.
        let transposed_out: [Col<B8, 4>; 8] = zero_padded_out
            .chunks(4)
            .enumerate()
            .map(|(i, cols)| {
                let expr: Expr<B8, 4> = cols
                    .iter()
                    .map(|&col| col.into())
                    .reduce(|acc, item| acc + item)
                    .expect("The iterator is not empty");
                table.add_computed(format!("zero_padded_sums_out_{i}"), expr)
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("zero_padded_out has exactly 8 chunks of 4 elements");

        // Finally, we pack into `Col<B32>` so we can pull from the VROM.
        let dst_vals: [Col<B32>; 8] =
            from_fn(|i| table.add_packed("dst_val_packed", transposed_out[i]));

        // Get the base address for the destination value.
        let dst_base_addr = state_cols.fp + upcast_col(state_cols.arg0);
        let dst_addrs = get_all_addresses(&mut table, dst_base_addr, "dst_addr");

        for i in 0..8 {
            table.pull(vrom_channel, [dst_addrs[i], dst_vals[i]]);
        }

        Self {
            id: table.id(),
            state_cols,
            dst_addrs,
            dst_vals,
            src1_addrs,
            src2_addrs,
            src1_lookup,
            src2_lookup,
            out,
            out_aes_columns,
            out_bits,
            projected_out,
            zero_padded_out,
            p_op,
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
                    // p_state[i] = B8::from(AESTowerField8b::new(event.src1_val[i]));
                    // p_state[i + 32] = B8::from(AESTowerField8b::new(event.src2_val[i]));
                    p_state[i] = B8::from(event.src1_val[i]);
                    p_state[i + 32] = B8::from(event.src2_val[i]);
                }

                // Since the input of this gadget comes from the output of the
                // `Groestl256Compress` gadget, it is transposed compared to the specs.
                p_state = (0..8)
                    .flat_map(|j| (0..8).map(move |k| p_state[k * 8 + j]))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap();
                p_state
            })
            .collect::<Vec<_>>();
        {
            let mut dst_addrs = (0..8)
                .map(|i| witness.get_mut_as(self.dst_addrs[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut src1_addrs = (0..4)
                .map(|i| witness.get_mut_as(self.src1_addrs[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;
            let mut src2_addrs = (0..4)
                .map(|i| witness.get_mut_as(self.src2_addrs[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;

            let mut dst_vals = (0..8)
                .map(|i| witness.get_mut_as(self.dst_vals[i]))
                .collect::<Result<Vec<RefMut<'_, [u32]>>, _>>()?;

            let mut out = (0..8)
                .map(|i| witness.get_scalars_mut(self.out[i]))
                .collect::<Result<Vec<_>, _>>()?;
            let mut out_bits = (0..8)
                .map(|i| witness.get_mut_as(self.out_bits[i]))
                .collect::<Result<Vec<RefMut<'_, [[u8; 8]]>>, _>>()?;
            let mut projected_out = (0..64)
                .map(|i| witness.get_mut_as(self.projected_out[i]))
                .collect::<Result<Vec<RefMut<'_, [u8]>>, _>>()?;
            let mut zero_padded_out = (0..32)
                .map(|i| witness.get_mut_as(self.zero_padded_out[i]))
                .collect::<Result<Vec<RefMut<'_, [[u8; 4]]>>, _>>()?;

            for (i, event) in rows.clone().enumerate() {
                let dst_base_addr = event.fp.addr(event.dst as u32);
                let src1_base_addr = event.fp.addr(event.src1 as u32);
                let src2_base_addr = event.fp.addr(event.src2 as u32);

                // Get u32 and byte representations of the destination value.
                let dst_val_u8 = u32_to_bytes(&event.dst_val);

                // Compute the output of the P permutation.
                let p_state_bytes = p_states[i]
                    .iter()
                    .map(|&b8| AESTowerField8b::from(b8).val())
                    .collect::<Vec<_>>();
                let mut state = GroestlShortImpl::state_from_bytes(
                    &p_state_bytes
                        .try_into()
                        .expect("p_state_bytes has exactly 64 elements"),
                );
                GroestlShortImpl::p_perm(&mut state);

                // Reshape the output to get the expected output.
                let p_out = GroestlShortImpl::state_to_bytes(&state);
                let p_out_transpose = p_out.map(|byte| B8::from(AESTowerField8b::new(byte)));
                let p_out = (0..8)
                    .map(|j| {
                        (0..8)
                            .map(|k| p_out_transpose[k * 8 + j])
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();

                for j in 0..4 {
                    src1_addrs[j][i] = src1_base_addr + 2 * j as u32;
                    src2_addrs[j][i] = src2_base_addr + 2 * j as u32;
                }
                for j in 0..8 {
                    dst_addrs[j][i] = dst_base_addr + j as u32;

                    for k in 0..8 {
                        // Fill out the destination values.
                        dst_vals[j][i] = event.dst_val[j];

                        // Fill out = p_out XOR src1_val.
                        out[j][i * 8 + k] = p_out[j][k] + B8::from(p_states[i][k * 8 + j]);
                        out_bits[j][i][k] = out[j][i * 8 + k].val();
                        projected_out[k * 8 + j][i] =
                            AESTowerField8b::from(out[j][i * 8 + k]).val();

                        // Fill out the truncated zero-padded columns.
                        if j < 4 {
                            zero_padded_out[j * 8 + k][i][k % 4] = dst_val_u8[j * 8 + k];
                        }
                    }
                }
            }
        }

        // Compute the iterators for src1 and src2 lookups.
        let src1_iters = (0..4)
            .map(|i| {
                rows.clone().map(move |ev| {
                    let vals = ev.src1_val[i * 8..(i + 1) * 8]
                        .chunks_exact(4)
                        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();

                    MultipleLookupGadget {
                        addr: ev.fp.addr(ev.src1) + 2 * i as u32,
                        vals,
                    }
                })
            })
            .collect::<Vec<_>>();
        let src2_iters = (0..4)
            .map(|i| {
                rows.clone().map(move |ev| {
                    let vals = ev.src2_val[i * 8..(i + 1) * 8]
                        .chunks_exact(4)
                        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();

                    MultipleLookupGadget {
                        addr: ev.fp.addr(ev.src2) + 2 * i as u32,
                        vals,
                    }
                })
            })
            .collect::<Vec<_>>();

        // Populate the src1 and src2 lookups.
        for i in 0..4 {
            self.src1_lookup[i].populate(witness, src1_iters[i].clone())?;
            self.src2_lookup[i].populate(witness, src2_iters[i].clone())?;
        }

        {
            // Collect the output data into a Vec so we can drop the immutable borrow before
            // mutably borrowing witness.
            let outs_vec = (0..8)
                .map(|i| {
                    let data = witness.get_as(self.out[i]).unwrap();
                    data.iter().map(|arr| *arr).collect::<Vec<[u8; 8]>>()
                })
                .collect::<Vec<Vec<[u8; 8]>>>();
            let out_iters_bin = (0..8)
                .map(|i| outs_vec[i].clone().into_iter())
                .collect::<Vec<_>>();
            self.out_aes_columns
                .iter()
                .zip(out_iters_bin)
                .for_each(|(aes_to_bin, o)| {
                    aes_to_bin
                        .populate(witness, o)
                        .expect("Failed to populate AesToBinColumns");
                });
        }
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

/// Returns `N` addresses starting at `base_addr`.
fn get_all_addresses<const N: usize>(
    table: &mut TableBuilder,
    base_addr: Expr<B32, 1>,
    label: &str,
) -> [Col<B32>; N] {
    from_fn(|i| {
        table.add_computed(
            format!("{label}_{i}"),
            base_addr.clone() + B32::from(i as u32),
        )
    })
}
/// Returns `N` addresses starting at `base_addr`.
fn get_half_addresses<const N: usize>(
    table: &mut TableBuilder,
    base_addr: Expr<B32, 1>,
    label: &str,
) -> [Col<B32>; N] {
    from_fn(|i| {
        table.add_computed(
            format!("{label}_{i}"),
            base_addr.clone() + B32::from(2 * i as u32),
        )
    })
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

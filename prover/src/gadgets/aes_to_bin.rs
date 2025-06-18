use std::{array::from_fn, cell::RefMut};

use binius_field::{packed::set_packed_slice, AESTowerField8b};
use binius_m3::builder::{Col, TableBuilder, TableWitnessSegment, B1, B8};

use crate::{types::ProverPackedField, utils::aes_bin_transform};

/// This gadget is used to switch between the AES and the binary bases. When
/// `AES_TO_BIN` is set to true, the gadget goes from the AES basis to the
/// binary basis.
pub(crate) struct AesBinTransformColumns<const AES_TO_BIN: bool> {
    /// The selected bits of the input.
    pub(crate) bits: [Col<B1>; 64], // Virtual
    /// The output, which corresponds to the input written in the AES or binary
    /// bases.
    pub(crate) outputs: [Col<B8>; 8], // Virtual
    /// Columns used to reshape the output into a single `Col<B8, 8>`.
    pub(crate) zero_padded_outs: [Col<B8, 8>; 8], // Virtual
    pub(crate) reshaped_outputs: Col<B8, 8>, // Virtual
}

impl<const AES_TO_BIN: bool> AesBinTransformColumns<AES_TO_BIN> {
    pub(crate) fn new(table: &mut TableBuilder, input: Col<B1, 64>, label: &str) -> Self {
        // We need to convert the B8 input into a B32 output in the AES basis.
        // First, select each B1 so that we can apply the basis transformation.
        let bits = from_fn(|i| table.add_selected(format!("{label}_aes_to_bin_bit_{i}"), input, i));
        let output_exprs = bits
            .chunks(8)
            .map(|c| {
                aes_bin_transform(
                    c.try_into().expect("Each chunk is 8 bits long."),
                    AES_TO_BIN,
                )
            })
            .collect::<Vec<_>>();
        let outputs = from_fn(|i| {
            table.add_computed(format!("{label}_aes_output_{i}"), output_exprs[i].clone())
        });

        let zero_padded_outs = from_fn(|i| {
            table.add_zero_pad::<_, 1, 8>(
                format!("{label}_aes_to_bin_zero_padded_{i}"),
                outputs[i],
                i,
            )
        });

        let reshaped_outs_expr = zero_padded_outs
            .iter()
            .map(|&col| col.into())
            .reduce(|acc, item| acc + item)
            .expect("The iterator is not empty");

        let reshaped_outputs = table.add_computed(
            format!("{label}_aes_to_bin_reshaped_outputs"),
            reshaped_outs_expr,
        );

        Self {
            bits,
            outputs,
            zero_padded_outs,
            reshaped_outputs,
        }
    }

    pub fn populate<T>(
        &self,
        index: &mut TableWitnessSegment<ProverPackedField>,
        inputs: T,
    ) -> anyhow::Result<()>
    where
        T: Iterator<Item = [u8; 8]>,
    {
        let mut bits = (0..64)
            .map(|i| index.get_mut(self.bits[i]))
            .collect::<Result<Vec<_>, _>>()?;
        let mut outputs = (0..8)
            .map(|i| index.get_mut_as(self.outputs[i]))
            .collect::<Result<Vec<RefMut<'_, [B8]>>, _>>()?;

        let mut zero_padded_outs = (0..8)
            .map(|i| index.get_mut_as(self.zero_padded_outs[i]))
            .collect::<Result<Vec<RefMut<'_, [[B8; 8]]>>, _>>()?;
        let mut reshaped_outputs: RefMut<'_, [[B8; 8]]> =
            index.get_mut_as(self.reshaped_outputs)?;

        for (i, ev_input) in inputs.enumerate() {
            for j in 0..8 {
                outputs[j][i] = if AES_TO_BIN {
                    B8::from(AESTowerField8b::new(ev_input[j]))
                } else {
                    B8::new(AESTowerField8b::from(B8::new(ev_input[j])).val())
                };
                zero_padded_outs[j][i][j] = outputs[j][i];
                reshaped_outputs[i][j] = outputs[j][i];
                for k in 0..8 {
                    set_packed_slice(&mut bits[j * 8 + k], i, B1::from((ev_input[j] >> k) & 1));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::array::from_fn;

    use binius_compute::cpu::alloc::CpuComputeAllocator;
    use binius_field::arch::OptimalUnderlier128b;
    use binius_field::as_packed_field::PackedType;
    use binius_field::{AESTowerField8b, PackedField};
    use binius_m3::builder::ConstraintSystem;
    use binius_m3::builder::{WitnessIndex, B1, B128, B8};

    use crate::gadgets::aes_to_bin::AesBinTransformColumns;

    #[test]
    fn test_aes_to_bin() {
        let mut cs = ConstraintSystem::new();
        let mut table = cs.add_table("aes_to_bin_test");

        let input = table.add_committed::<B1, 64>("input");
        let aes_to_bin = AesBinTransformColumns::<true>::new(&mut table, input, "test");

        let table_id = table.id();

        let mut allocator = CpuComputeAllocator::new(1 << 12);
        let allocator = allocator.into_bump_allocator();

        let mut witness =
            WitnessIndex::<PackedType<OptimalUnderlier128b, B128>>::new(&cs, &allocator);

        let table_witness = witness.init_table(table_id, 1 << 2).unwrap();

        let inputs: [u8; 8] = from_fn(|_| rand::random::<u8>());

        let expected_outputs = (0..8)
            .map(|i| B8::from(AESTowerField8b::new(inputs[i])))
            .collect::<Vec<_>>();

        let mut segment = table_witness.full_segment();
        aes_to_bin
            .populate(&mut segment, [inputs].into_iter())
            .unwrap();

        {
            let mut table_inputs: std::cell::RefMut<'_, [[u8; 8]]> = segment
                .get_mut_as(input)
                .expect("Column should exist in compiled constraint system");
            let outputs = aes_to_bin.outputs.map(|col| {
                segment
                    .get(col)
                    .expect("Column should exist in compiled constraint system")
            });

            table_inputs[0] = inputs;

            for i in 0..8 {
                assert_eq!(
                    outputs[i][0].get(0),
                    expected_outputs[i],
                    "Expected: {:x?}, but got: {:x?}",
                    expected_outputs[i],
                    outputs[i][0],
                );
            }
        }

        let ccs = cs.compile().unwrap();
        let table_sizes = witness.table_sizes();
        let witness = witness.into_multilinear_extension_index();

        binius_core::constraint_system::validate::validate_witness(
            &ccs,
            &[],
            &table_sizes,
            &witness,
        )
        .unwrap();
    }

    #[test]
    fn test_bin_to_aes() {
        let mut cs = ConstraintSystem::new();
        let mut table = cs.add_table("bin_to_aes_test");

        let input = table.add_committed::<B1, 64>("input");
        let aes_to_bin = AesBinTransformColumns::<false>::new(&mut table, input, "test");

        let table_id = table.id();

        let mut allocator = CpuComputeAllocator::new(1 << 12);
        let allocator = allocator.into_bump_allocator();

        let mut witness =
            WitnessIndex::<PackedType<OptimalUnderlier128b, B128>>::new(&cs, &allocator);

        let table_witness = witness.init_table(table_id, 1 << 2).unwrap();

        let inputs: [u8; 8] = from_fn(|_| rand::random::<u8>());

        let expected_outputs = (0..8)
            .map(|i| AESTowerField8b::from(B8::new(inputs[i])))
            .collect::<Vec<_>>();

        let mut segment = table_witness.full_segment();
        aes_to_bin
            .populate(&mut segment, [inputs].into_iter())
            .unwrap();

        {
            let mut table_inputs: std::cell::RefMut<'_, [[u8; 8]]> = segment
                .get_mut_as(input)
                .expect("Column should exist in compiled constraint system");
            let outputs = aes_to_bin.outputs.map(|col| {
                segment
                    .get(col)
                    .expect("Column should exist in compiled constraint system")
            });

            table_inputs[0] = inputs;

            for i in 0..8 {
                assert_eq!(
                    outputs[i][0].get(0),
                    B8::new(expected_outputs[i].val()),
                    "Expected: {:x?}, but got: {:x?}",
                    expected_outputs[i],
                    outputs[i][0].get(0)
                );
            }
        }

        let ccs = cs.compile().unwrap();
        let table_sizes = witness.table_sizes();
        let witness = witness.into_multilinear_extension_index();

        binius_core::constraint_system::validate::validate_witness(
            &ccs,
            &[],
            &table_sizes,
            &witness,
        )
        .unwrap();
    }
}

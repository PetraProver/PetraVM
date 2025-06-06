use std::{array::from_fn, cell::RefMut};

use binius_field::packed::set_packed_slice;
use binius_m3::builder::{Col, Expr, TableBuilder, TableWitnessSegment, B32, B8};

use crate::types::ProverPackedField;

/// This gadget transposes a matrix of 8x8 B8 columns. Then it reshapes the
/// transposed matrix into a `[Col<B32>; 16]`, so that we can read the values
/// from memory.
pub(crate) struct TransposeColumns {
    /// The input matrix.
    pub(crate) input: [Col<B8, 8>; 8],
    /// The output of the transposition, reshaped so that we can easily pull the
    /// values from the VROM.
    pub(crate) output: [Col<B32>; 16],
    /// The projected values of the input matrix, transposed and flattened.
    pub(crate) projected: [Col<B8>; 64],
    /// The zero-padding of the projected values, so we can sum the elements
    /// into B32s.
    pub(crate) zero_padded: [Col<B8, 4>; 64],
    /// The final values, before packing into B32s.
    pub(crate) transposed: [Col<B8, 4>; 16],
}

impl TransposeColumns {
    pub(crate) fn new(table: &mut TableBuilder, input: [Col<B8, 8>; 8]) -> Self {
        // First, we project the values into independent B8 columns.
        let projected_temp: [[Col<B8>; 8]; 8] =
            from_fn(|i| from_fn(|j| table.add_selected(format!("projected_{i}_{j}"), input[i], j)));
        // We take the projected values into the correct (transposed) order.
        let projected = from_fn(|i| projected_temp[i % 8][i / 8]);

        // Now, we need to construct the B32 elements so we can read from the VROM.
        // We zeropad the projected values to go from `Col<B8>` to `Col<B8, 4>`.
        let zero_padded = from_fn(|i| {
            table.add_zero_pad::<_, 1, 4>(format!("zero_padded_{i}"), projected[i], i % 4)
        });
        // Finally, we sum each array of B8 to get the correct B32 values.
        let transposed: [Col<B8, 4>; 16] = zero_padded
            .chunks(4)
            .enumerate()
            .map(|(i, cols)| {
                let expr: Expr<B8, 4> = cols
                    .iter()
                    .map(|&col| col.into())
                    .reduce(|acc, item| acc + item)
                    .expect("The iterator is not empty");
                table.add_computed(format!("zero_padded_sums_{i}"), expr)
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("zero_padded has exactly 16 chunks of 4 elements");

        let output = (0..16)
            .map(|i| table.add_packed(format!("packed_transpose_{i}"), transposed[i]))
            .collect::<Vec<_>>()
            .try_into()
            .expect("output has exactly 16 elements");

        Self {
            input,
            output,
            projected,
            zero_padded,
            transposed,
        }
    }

    pub fn populate<T>(
        &self,
        index: &mut TableWitnessSegment<ProverPackedField>,
        inputs: T,
    ) -> Result<(), anyhow::Error>
    where
        T: Iterator<Item = [u8; 64]>,
    {
        let mut input = (0..8)
            .map(|i| index.get_mut(self.input[i]))
            .collect::<Result<Vec<_>, _>>()?;
        let mut projected = (0..64)
            .map(|i| index.get_mut_as(self.projected[i]))
            .collect::<Result<Vec<RefMut<'_, [u8]>>, _>>()?;
        let mut zero_padded = (0..64)
            .map(|i| index.get_mut_as(self.zero_padded[i]))
            .collect::<Result<Vec<RefMut<'_, [[u8; 4]]>>, _>>()?;
        let mut transposed = (0..16)
            .map(|i| index.get_mut(self.transposed[i]))
            .collect::<Result<Vec<_>, _>>()?;

        for (i, ev_input) in inputs.enumerate() {
            for j in 0..8 {
                for k in 0..8 {
                    set_packed_slice(&mut input[j], i * 8 + k, B8::from(ev_input[k * 8 + j]));
                    projected[j * 8 + k][i] = ev_input[j * 8 + k];
                }
            }

            for j in 0..16 {
                for k in 0..4 {
                    zero_padded[j * 4 + k][i][k] = projected[j * 4 + k][i];
                    set_packed_slice(&mut transposed[j], i * 8 + k, B8::from(ev_input[j * 4 + k]));
                }
            }
        }

        Ok(())
    }
}

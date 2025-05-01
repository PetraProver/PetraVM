use std::cell::RefMut;

use binius_core::oracle::ShiftVariant;
use binius_field::{packed::set_packed_slice, Field, PackedExtension, PackedFieldIndexable};
use binius_m3::builder::{upcast_col, Col, Expr, TableBuilder, TableWitnessSegment, B1, B128, B32};

/// Maximum number of bits of the shift amount, i.e. 0 < shift_amount < 1 <<
/// SHIFT_MAX_BITS - 1 = 31 where dst_val = src_val >> shift_amount or dst_val =
/// src_val << shift_amount
const MAX_SHIFT_BITS: usize = 5;

/// A gadget for performing arithmetic right shift using logical operations.
///
/// The `ArithmeticShifter` gadget allows for arithmetic right shifts on 32-bit
/// inputs, with a configurable shift amount. It implements arithmetic shift
/// using logical operations with the following algorithm:
/// - For positive numbers: uses logical right shift
/// - For negative numbers: uses invert-shift-invert pattern
pub struct ArithmeticShifter {
    /// The input column representing the 32-bit value to be shifted.
    input: Col<B1, 32>,

    /// The shift amount column representing the 5 of positions to shift,
    /// ignoring the remaining 11.
    shift_amount: Col<B1, 16>,

    /// Virtual columns containing the binary decomposition of the shifted
    /// amount.
    shift_amount_bits: [Col<B1>; MAX_SHIFT_BITS],

    /// The sign bit (bit 31) of the input
    sign_bit: Col<B1>,

    /// Inverted input for negative number path
    inverted_input: Col<B1, 32>,

    /// Logical right shift result for positive numbers
    r_pos: Col<B1, 32>,
    r_pos_packed: Col<B32, 1>,

    /// Inverted result after logical shift for negative numbers
    r_neg: Col<B1, 32>,
    r_neg_packed: Col<B32, 1>,

    // TODO: Try to replace the Vec with an array.
    /// Partial shift virtual columns containing the partial_shift[i - 1]
    /// shifted by 2^i.
    shifted_pos: Vec<Col<B1, 32>>, // For positive number path
    shifted_neg: Vec<Col<B1, 32>>, // For negative number path

    /// Partial shift virtual columns containing either shifted[i] or
    /// partial_shit[i-1], depending on the value of `shift_amount_bits`.
    partial_shift_pos: [Col<B1, 32>; MAX_SHIFT_BITS],
    partial_shift_neg: [Col<B1, 32>; MAX_SHIFT_BITS],

    /// The output column representing the result of the shift operation. This
    /// column is virtual or commited, depending on the flags
    pub output: Col<B1, 32>,
    output_packed: Col<B32, 1>,
}

impl ArithmeticShifter {
    /// Creates a new instance of the `ArithmeticShifter` gadget.
    ///
    /// # Arguments
    ///
    /// * `table` - A mutable reference to the `TableBuilder` used to define the
    ///   gadget.
    /// * `input` - The input column of type `Col<B1, 32>`.
    /// * `shift_amount` - The shift amount column of type `Col<B1, 16>`. The 11
    ///   most significant bits are ignored.
    ///
    /// # Returns
    ///
    /// A new instance of the `ArithmeticShifter` gadget.
    pub fn new(table: &mut TableBuilder, input: Col<B1, 32>, shift_amount: Col<B1, 16>) -> Self {
        let partial_shift_pos =
            core::array::from_fn(|i| table.add_committed(format!("partial_shift_pos_{i}")));
        let partial_shift_neg =
            core::array::from_fn(|i| table.add_committed(format!("partial_shift_neg_{i}")));

        let shift_amount_bits: [_; MAX_SHIFT_BITS] = core::array::from_fn(|i| {
            table.add_selected(format!("shift_amount_bits_{i}"), shift_amount, i)
        });

        // Inverted input for negative number path (!x)
        let inverted_input = table.add_computed("inverted_input", input + B1::ONE);

        let mut shifted_pos = Vec::with_capacity(MAX_SHIFT_BITS);
        let mut shifted_neg = Vec::with_capacity(MAX_SHIFT_BITS);

        let mut current_shift_pos = input;
        let mut current_shift_neg = inverted_input;

        for i in 0..MAX_SHIFT_BITS {
            // For positive number path: logical right shift
            shifted_pos.push(table.add_shifted(
                "shifted_pos",
                current_shift_pos,
                5,
                1 << i,
                ShiftVariant::LogicalRight,
            ));

            // For negative number path: logical right shift of inverted input
            shifted_neg.push(table.add_shifted(
                "shifted_neg",
                current_shift_neg,
                5,
                1 << i,
                ShiftVariant::LogicalRight,
            ));

            // Positive path mux
            let partial_shift_pos_packed: Col<B32> = table.add_packed(
                format!("partial_shift_pos_packed_{i}"),
                partial_shift_pos[i],
            );
            let shifted_pos_packed: Expr<B32, 1> = table
                .add_packed(format!("shifted_pos_packed_{i}"), shifted_pos[i])
                .into();
            let current_shift_pos_packed: Col<B32> =
                table.add_packed(format!("current_shift_pos_packed_{i}"), current_shift_pos);

            table.assert_zero(
                format!("correct_partial_shift_pos_{i}"),
                partial_shift_pos_packed
                    - (shifted_pos_packed * upcast_col(shift_amount_bits[i])
                        + current_shift_pos_packed * (upcast_col(shift_amount_bits[i]) - B32::ONE)),
            );

            // Negative path mux
            let partial_shift_neg_packed: Col<B32> = table.add_packed(
                format!("partial_shift_neg_packed_{i}"),
                partial_shift_neg[i],
            );
            let shifted_neg_packed: Expr<B32, 1> = table
                .add_packed(format!("shifted_neg_packed_{i}"), shifted_neg[i])
                .into();
            let current_shift_neg_packed: Col<B32> =
                table.add_packed(format!("current_shift_neg_packed_{i}"), current_shift_neg);

            table.assert_zero(
                format!("correct_partial_shift_neg_{i}"),
                partial_shift_neg_packed
                    - (shifted_neg_packed * upcast_col(shift_amount_bits[i])
                        + current_shift_neg_packed * (upcast_col(shift_amount_bits[i]) - B32::ONE)),
            );

            current_shift_pos = partial_shift_pos[i];
            current_shift_neg = partial_shift_neg[i];
        }

        // Final results for both paths
        let r_pos = current_shift_pos; // Logical right shift result
        let r_pos_packed: Col<B32, 1> = table.add_packed(format!("r_pos_packed"), r_pos);
        let r_neg = table.add_computed("r_neg", current_shift_neg + B1::ONE); // Inverted result after logical shift
        let r_neg_packed: Col<B32, 1> = table.add_packed(format!("r_neg_packed"), r_neg);

        // Final output based on sign bit
        let output = table.add_committed::<B1, 32>("output");
        let output_packed: Col<B32, 1> = table.add_packed(format!("output_packed"), output);

        // Extract sign bit (bit 31) of input
        let sign_bit = table.add_selected("sign_bit", input, 31);
        table.assert_zero(
            format!("correct_output_packed"),
            output_packed
                - (r_neg_packed * upcast_col(sign_bit)
                    + r_pos_packed * (upcast_col(sign_bit) - B32::ONE)),
        );

        Self {
            input,
            shift_amount,
            shift_amount_bits,
            sign_bit,
            inverted_input,
            r_pos,
            r_pos_packed,
            r_neg,
            r_neg_packed,
            shifted_pos,
            shifted_neg,
            partial_shift_pos,
            partial_shift_neg,
            output,
            output_packed,
        }
    }

    /// Populates the table with witness values for the arithmetic shifter.
    ///
    /// # Arguments
    ///
    /// * `index` - A mutable reference to the `TableWitness` used to populate
    ///   the table.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure.
    pub fn populate<P>(&self, index: &mut TableWitnessSegment<P>) -> Result<(), anyhow::Error>
    where
        P: PackedFieldIndexable<Scalar = B128> + PackedExtension<B1>,
    {
        let input: RefMut<'_, [u32]> = index.get_mut_as(self.input).unwrap();
        let shift_amount: RefMut<'_, [u16]> = index.get_mut_as(self.shift_amount).unwrap();
        let mut sign_bit = index.get_mut(self.sign_bit).unwrap();
        let mut inverted_input = index.get_mut_as(self.inverted_input).unwrap();
        let mut r_neg = index.get_mut_as(self.r_neg).unwrap();
        let mut output = index.get_mut_as(self.output).unwrap();

        let mut partial_shift_pos: [_; MAX_SHIFT_BITS] =
            array_util::try_from_fn(|i| index.get_mut_as(self.partial_shift_pos[i]))?;
        let mut partial_shift_neg: [_; MAX_SHIFT_BITS] =
            array_util::try_from_fn(|i| index.get_mut_as(self.partial_shift_neg[i]))?;
        let mut shifted_pos: [_; MAX_SHIFT_BITS] =
            array_util::try_from_fn(|i| index.get_mut_as(self.shifted_pos[i]))?;
        let mut shifted_neg: [_; MAX_SHIFT_BITS] =
            array_util::try_from_fn(|i| index.get_mut_as(self.shifted_neg[i]))?;
        let mut shift_amount_bits: [_; MAX_SHIFT_BITS] =
            array_util::try_from_fn(|i| index.get_mut(self.shift_amount_bits[i]))?;

        for i in 0..index.size() {
            // Extract sign bit (bit 31)
            let is_negative = (input[i] >> 31) & 1 == 1;
            set_packed_slice(&mut sign_bit, i, B1::from(is_negative));

            // Invert input for negative number path
            inverted_input[i] = !input[i];

            // Process both paths
            let mut current_shift_pos = input[i];
            let mut current_shift_neg = inverted_input[i];

            for j in 0..MAX_SHIFT_BITS {
                let bit = ((shift_amount[i] >> j) & 1) == 1;
                set_packed_slice(&mut shift_amount_bits[j], i, B1::from(bit));

                // Logical right shift for positive path
                shifted_pos[j][i] = current_shift_pos >> (1 << j);

                // Logical right shift for negative path (inverted input)
                shifted_neg[j][i] = current_shift_neg >> (1 << j);

                if bit {
                    current_shift_pos = shifted_pos[j][i];
                    current_shift_neg = shifted_neg[j][i];
                }

                partial_shift_pos[j][i] = current_shift_pos;
                partial_shift_neg[j][i] = current_shift_neg;
            }

            // Invert the result of the negative path to complete the invert-shift-invert
            // pattern
            r_neg[i] = !current_shift_neg;

            // Select based on sign bit: (r_neg & sign_mask) | (r_pos & !sign_mask)
            let sign_mask = if is_negative { 0xFFFFFFFF } else { 0 };
            output[i] = (r_neg[i] & sign_mask) | (current_shift_pos & !sign_mask);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::iter::repeat_with;

    use binius_field::{arch::OptimalUnderlier128b, as_packed_field::PackedType};
    use binius_m3::builder::{ConstraintSystem, Statement, WitnessIndex};
    use bumpalo::Bump;
    use rand::{rngs::StdRng, Rng, SeedableRng};

    use super::*;

    fn test_arithmetic_shifter() {
        let mut cs = ConstraintSystem::new();
        let mut table = cs.add_table("ArithmeticShifterTable");
        let table_id = table.id();
        let allocator = Bump::new();

        let input = table.add_committed::<B1, 32>("input");
        let shift_amount = table.add_committed::<B1, 16>("shift_amount");

        let shifter = ArithmeticShifter::new(&mut table, input, shift_amount);

        let statement = Statement {
            boundaries: vec![],
            table_sizes: vec![1 << 8],
        };
        let mut witness =
            WitnessIndex::<PackedType<OptimalUnderlier128b, B128>>::new(&cs, &allocator);
        let table_witness = witness.init_table(table_id, 1 << 8).unwrap();
        let mut segment = table_witness.full_segment();

        let mut rng = StdRng::seed_from_u64(0x1234);
        // Mix of positive and negative numbers
        let test_inputs = repeat_with(|| rng.random::<i32>() as u32)
            .take(1 << 8)
            .collect::<Vec<u32>>();

        for (i, (input, shift_amount)) in (*segment.get_mut_as(input).unwrap())
            .iter_mut()
            .zip(segment.get_mut_as(shift_amount).unwrap().iter_mut())
            .enumerate()
        {
            *input = test_inputs[i];
            *shift_amount = i as u16; // Only the first 5 bits are used
        }

        shifter.populate(&mut segment).unwrap();

        for (i, &output) in segment
            .get_as::<u32, B1, 32>(shifter.output)
            .unwrap()
            .iter()
            .enumerate()
        {
            // Calculate expected arithmetic right shift result
            let input_i32 = test_inputs[i] as i32;
            let shift = (i % 32) as i32;
            let expected_output = (input_i32 >> shift) as u32;

            assert_eq!(
                output, expected_output,
                "Mismatch for input: {:#x}, shift: {}, got: {:#x}, expected: {:#x}",
                test_inputs[i], shift, output, expected_output
            );
        }

        let ccs = cs.compile(&statement).unwrap();
        let witness = witness.into_multilinear_extension_index();

        binius_core::constraint_system::validate::validate_witness(&ccs, &[], &witness).unwrap();
    }

    #[test]
    fn test_arithmetic_right_shift() {
        test_arithmetic_shifter();
    }
}

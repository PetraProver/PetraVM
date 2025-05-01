use std::cell::RefMut;

use binius_core::oracle::ShiftVariant;
use binius_field::{packed::set_packed_slice, Field, PackedExtension, PackedFieldIndexable};
use binius_m3::builder::{upcast_col, Col, Expr, TableBuilder, TableWitnessSegment, B1, B128, B32};

/// Maximum number of bits of the shift amount.
/// Shifting is limited to 0 < shift_amount < 2^MAX_SHIFT_BITS - 1 = 31
const MAX_SHIFT_BITS: usize = 5;

/// A gadget for performing arithmetic right shift using logical operations.
///
/// The `ArithmeticShifter` gadget allows for arithmetic right shifts on 32-bit
/// inputs, with a configurable shift amount. It implements arithmetic shift
/// using logical operations with the following algorithm:
///
/// - For positive numbers (sign bit = 0):
///   * Simply perform logical right shift
///
/// - For negative numbers (sign bit = 1):
///   * Apply the invert-shift-invert pattern:
///     1. Invert the input (~x)
///     2. Perform logical right shift on the inverted value
///     3. Invert the result to get the final output
///
/// This approach correctly handles the sign bit propagation required for
/// arithmetic right shift operations.
pub struct ArithmeticRightShifter {
    /// The input column representing the 32-bit value to be shifted.
    input: Col<B1, 32>,

    /// The shift amount column representing the number of positions to shift.
    /// Only the 5 least significant bits are used (supporting shifts 0-31).
    shift_amount: Col<B1, 16>,

    /// Binary decomposition of the shift amount (5 bits).
    /// Each bit represents a power of 2 in the shift amount.
    shift_amount_bits: [Col<B1>; MAX_SHIFT_BITS],

    /// The sign bit (bit 31) of the input value.
    /// Controls whether we use the positive or negative number path.
    sign_bit: Col<B1>,

    /// Inverted input value (~input) used for the negative number path.
    inverted_input: Col<B1, 32>,

    /// Logical right shift result for positive numbers.
    /// This is the final output when the input is positive.
    positive_result: Col<B1, 32>,

    /// Inverted result after logical shift for negative numbers.
    /// This is the final output when the input is negative.
    negative_result: Col<B1, 32>,

    /// Intermediate columns containing the value after applying shift by 2^i.
    /// Used in the positive number path calculation.
    shifted_positive: Vec<Col<B1, 32>>,

    /// Intermediate columns containing the value after applying shift by 2^i.
    /// Used in the negative number path calculation.
    shifted_negative: Vec<Col<B1, 32>>,

    /// Intermediate columns containing the partial shift results for each bit
    /// position. For the positive number path.
    partial_shift_positive: [Col<B1, 32>; MAX_SHIFT_BITS],

    /// Intermediate columns containing the partial shift results for each bit
    /// position. For the negative number path.
    partial_shift_negative: [Col<B1, 32>; MAX_SHIFT_BITS],

    /// The output column representing the final result of the arithmetic shift
    /// operation.
    pub output: Col<B1, 32>,
}

impl ArithmeticRightShifter {
    /// Creates a new instance of the `ArithmeticShifter` gadget.
    ///
    /// # Arguments
    ///
    /// * `table` - A mutable reference to the `TableBuilder` used to define the
    ///   constraint system for the gadget.
    /// * `input` - The input column of type `Col<B1, 32>` representing the
    ///   32-bit value to shift.
    /// * `shift_amount` - The shift amount column of type `Col<B1, 16>`
    ///   representing the number of positions to shift. Only the 5 least
    ///   significant bits are used.
    ///
    /// # Returns
    ///
    /// A new instance of the `ArithmeticShifter` gadget with all necessary
    /// constraints defined.
    pub fn new(table: &mut TableBuilder, input: Col<B1, 32>, shift_amount: Col<B1, 16>) -> Self {
        // Create committed columns for partial shift results for both paths
        let partial_shift_positive =
            core::array::from_fn(|i| table.add_committed(format!("partial_shift_positive_{i}")));
        let partial_shift_negative =
            core::array::from_fn(|i| table.add_committed(format!("partial_shift_negative_{i}")));

        // Extract individual bits from the shift amount (only the 5 LSBs matter)
        let shift_amount_bits: [_; MAX_SHIFT_BITS] = core::array::from_fn(|i| {
            table.add_selected(format!("shift_amount_bit_{i}"), shift_amount, i)
        });

        // Generate inverted input for the negative number path
        let inverted_input = table.add_computed("inverted_input", input + B1::ONE);

        // Vectors to store intermediate shifted values
        let mut shifted_positive = Vec::with_capacity(MAX_SHIFT_BITS);
        let mut shifted_negative = Vec::with_capacity(MAX_SHIFT_BITS);

        // Start with the original values for both paths
        let mut current_shift_positive = input;
        let mut current_shift_negative = inverted_input;

        // Build the shift network for each bit position (powers of 2)
        for i in 0..MAX_SHIFT_BITS {
            // For positive number path: logical right shift by 2^i positions
            shifted_positive.push(table.add_shifted(
                format!("shifted_positive_{i}"),
                current_shift_positive,
                5,      // Bound on shift amount (0-31)
                1 << i, // Shift amount: 2^i (1, 2, 4, 8, 16)
                ShiftVariant::LogicalRight,
            ));

            // For negative number path: logical right shift of inverted input by 2^i
            // positions
            shifted_negative.push(table.add_shifted(
                format!("shifted_negative_{i}"),
                current_shift_negative,
                5,      // Bound on shift amount (0-31)
                1 << i, // Shift amount: 2^i (1, 2, 4, 8, 16)
                ShiftVariant::LogicalRight,
            ));

            // Create packed (32-bit) versions of columns for the positive path
            let partial_shift_positive_packed: Col<B32> = table.add_packed(
                format!("partial_shift_positive_packed_{i}"),
                partial_shift_positive[i],
            );
            let shifted_positive_packed: Expr<B32, 1> = table
                .add_packed(format!("shifted_positive_packed_{i}"), shifted_positive[i])
                .into();
            let current_shift_positive_packed: Col<B32> = table.add_packed(
                format!("current_shift_positive_packed_{i}"),
                current_shift_positive,
            );

            // Create constraint for the positive path mux:
            // partial_shift_positive = shift_bit ? shifted_positive :
            //                                      current_shift_positive
            table.assert_zero(
                format!("constraint_partial_shift_positive_{i}"),
                partial_shift_positive_packed
                    - (shifted_positive_packed * upcast_col(shift_amount_bits[i])
                        + current_shift_positive_packed
                            * (upcast_col(shift_amount_bits[i]) - B32::ONE)),
            );

            // Create packed (32-bit) versions of columns for the negative path
            let partial_shift_negative_packed: Col<B32> = table.add_packed(
                format!("partial_shift_negative_packed_{i}"),
                partial_shift_negative[i],
            );
            let shifted_negative_packed: Expr<B32, 1> = table
                .add_packed(format!("shifted_negative_packed_{i}"), shifted_negative[i])
                .into();
            let current_shift_negative_packed: Col<B32> = table.add_packed(
                format!("current_shift_negative_packed_{i}"),
                current_shift_negative,
            );

            // Create constraint for the negative path mux:
            // partial_shift_negative = shift_bit ? shifted_negative :
            //                                      current_shift_negative
            table.assert_zero(
                format!("constraint_partial_shift_negative_{i}"),
                partial_shift_negative_packed
                    - (shifted_negative_packed * upcast_col(shift_amount_bits[i])
                        + current_shift_negative_packed
                            * (upcast_col(shift_amount_bits[i]) - B32::ONE)),
            );

            // Update current shift values for the next iteration
            current_shift_positive = partial_shift_positive[i];
            current_shift_negative = partial_shift_negative[i];
        }

        // Final results for both paths
        let positive_result = current_shift_positive; // Logical right shift result
        let positive_result_packed: Col<B32, 1> =
            table.add_packed("positive_result_packed", positive_result);

        // Invert the negative path result to complete the invert-shift-invert pattern
        let negative_result =
            table.add_computed("negative_result", current_shift_negative + B1::ONE);
        let negative_result_packed: Col<B32, 1> =
            table.add_packed("negative_result_packed", negative_result);

        // Final output column
        let output = table.add_committed::<B1, 32>("output");
        let output_packed: Col<B32, 1> = table.add_packed("output_packed", output);

        // Extract sign bit (bit 31) of input to select the appropriate result
        let sign_bit = table.add_selected("sign_bit", input, 31);

        // Create constraint for the final output mux:
        // output = sign_bit ? negative_result : positive_result
        table.assert_zero(
            "constraint_output_selection",
            output_packed
                - (negative_result_packed * upcast_col(sign_bit)
                    + positive_result_packed * (upcast_col(sign_bit) - B32::ONE)),
        );

        Self {
            input,
            shift_amount,
            shift_amount_bits,
            sign_bit,
            inverted_input,
            positive_result,
            negative_result,
            shifted_positive,
            shifted_negative,
            partial_shift_positive,
            partial_shift_negative,
            output,
        }
    }

    /// Populates the table witness with computed values for the arithmetic
    /// shifter.
    ///
    /// This method calculates all intermediate and final values needed to
    /// satisfy the constraints defined in the `new` method.
    ///
    /// # Arguments
    ///
    /// * `index` - A mutable reference to the `TableWitnessSegment` used to
    ///   populate the table with witness values.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure of the witness population.
    pub fn populate<P>(&self, index: &mut TableWitnessSegment<P>) -> Result<(), anyhow::Error>
    where
        P: PackedFieldIndexable<Scalar = B128> + PackedExtension<B1>,
    {
        // Get mutable references to all witness columns
        let input: RefMut<'_, [u32]> = index.get_mut_as(self.input).unwrap();
        let shift_amount: RefMut<'_, [u16]> = index.get_mut_as(self.shift_amount).unwrap();
        let mut sign_bit = index.get_mut(self.sign_bit).unwrap();
        let mut inverted_input = index.get_mut_as(self.inverted_input).unwrap();
        let mut negative_result = index.get_mut_as(self.negative_result).unwrap();
        let mut output = index.get_mut_as(self.output).unwrap();

        // Get mutable references to all intermediate witness columns
        let mut partial_shift_positive: [_; MAX_SHIFT_BITS] =
            array_util::try_from_fn(|i| index.get_mut_as(self.partial_shift_positive[i]))?;
        let mut partial_shift_negative: [_; MAX_SHIFT_BITS] =
            array_util::try_from_fn(|i| index.get_mut_as(self.partial_shift_negative[i]))?;
        let mut shifted_positive: [_; MAX_SHIFT_BITS] =
            array_util::try_from_fn(|i| index.get_mut_as(self.shifted_positive[i]))?;
        let mut shifted_negative: [_; MAX_SHIFT_BITS] =
            array_util::try_from_fn(|i| index.get_mut_as(self.shifted_negative[i]))?;
        let mut shift_amount_bits: [_; MAX_SHIFT_BITS] =
            array_util::try_from_fn(|i| index.get_mut(self.shift_amount_bits[i]))?;

        // Process each row in the witness table
        for row_idx in 0..index.size() {
            // Extract sign bit (bit 31) of the input
            let is_negative = (input[row_idx] >> 31) & 1 == 1;
            set_packed_slice(&mut sign_bit, row_idx, B1::from(is_negative));

            // Compute inverted input for the negative number path
            inverted_input[row_idx] = !input[row_idx];

            // Initialize the current shift values for both paths
            let mut current_shift_positive = input[row_idx];
            let mut current_shift_negative = inverted_input[row_idx];

            // Process each bit of the shift amount
            for bit_idx in 0..MAX_SHIFT_BITS {
                // Extract the current bit of the shift amount
                let shift_bit = ((shift_amount[row_idx] >> bit_idx) & 1) == 1;
                set_packed_slice(
                    &mut shift_amount_bits[bit_idx],
                    row_idx,
                    B1::from(shift_bit),
                );

                // Compute shifted values for both paths (right shift by 2^bit_idx)
                shifted_positive[bit_idx][row_idx] = current_shift_positive >> (1 << bit_idx);
                shifted_negative[bit_idx][row_idx] = current_shift_negative >> (1 << bit_idx);

                // Select the appropriate value based on the shift bit
                if shift_bit {
                    current_shift_positive = shifted_positive[bit_idx][row_idx];
                    current_shift_negative = shifted_negative[bit_idx][row_idx];
                }

                // Store partial shift results for both paths
                partial_shift_positive[bit_idx][row_idx] = current_shift_positive;
                partial_shift_negative[bit_idx][row_idx] = current_shift_negative;
            }

            // Invert the result of the negative path to complete the invert-shift-invert
            // pattern
            negative_result[row_idx] = !current_shift_negative;

            // Select final output based on the sign bit
            output[row_idx] = if is_negative {
                negative_result[row_idx]
            } else {
                current_shift_positive
            };
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use binius_core::constraint_system::validate::validate_witness;
    use binius_field::{arch::OptimalUnderlier128b, as_packed_field::PackedType};
    use binius_m3::builder::{ConstraintSystem, Statement, WitnessIndex};
    use bumpalo::Bump;
    use proptest::prelude::*;

    use super::*;

    /// Test the arithmetic shifter with given input and shift values.
    fn test_shifter_with_values(input: i32, shift_amount: u16) -> Result<(), anyhow::Error> {
        // Set up the constraint system and table
        let mut cs = ConstraintSystem::new();
        let mut table = cs.add_table("ArithmeticShifterTable");
        let table_id = table.id();
        let allocator = Bump::new();

        // Define input columns
        let input_col = table.add_committed::<B1, 32>("input");
        let shift_amount_col = table.add_committed::<B1, 16>("shift_amount");

        // Create the arithmetic shifter
        let shifter = ArithmeticRightShifter::new(&mut table, input_col, shift_amount_col);

        // Set up witness and statement
        let statement = Statement {
            boundaries: vec![],
            table_sizes: vec![1],
        };
        let mut witness =
            WitnessIndex::<PackedType<OptimalUnderlier128b, B128>>::new(&cs, &allocator);
        let table_witness = witness.init_table(table_id, 1).unwrap();
        let mut segment = table_witness.full_segment();

        // Populate input and shift amount columns
        let mut input_ref = segment.get_mut_as::<u32, B1, 32>(input_col).unwrap();
        input_ref[0] = input as u32;

        let mut shift_ref = segment.get_mut_as::<u16, B1, 16>(shift_amount_col).unwrap();
        shift_ref[0] = shift_amount;

        // Drop the mutable borrows before calling populate
        drop(input_ref);
        drop(shift_ref);

        // Populate the witness
        shifter.populate(&mut segment)?;

        // Verify results against expected outcomes
        let output_val = segment.get_as::<u32, B1, 32>(shifter.output).unwrap()[0];

        // Calculate expected arithmetic right shift result
        let shift_amount = shift_amount & 0x1F;
        let expected_output = (input >> shift_amount) as u32;

        assert_eq!(
            output_val, expected_output,
            "Mismatch for input: {:#x}, shift: {}, got: {:#x}, expected: {:#x}",
            input, shift_amount, output_val, expected_output
        );

        // Validate the witness against the constraint system
        let ccs = cs.compile(&statement)?;

        // Clone the witness before consuming it
        let witness_mle = witness.into_multilinear_extension_index();

        validate_witness(&ccs, &[], &witness_mle)?;

        Ok(())
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(20))]

        #[test]
        fn test_arithmetic_right_shift_gadget(
            input_val in prop_oneof![
                any::<i32>(),                    // Random values
            ],
            // Shift amount test cases
            shift_val in prop_oneof![
                any::<u16>(),                    // Random values within valid range
                Just(0u16),                      // Zero shift
                Just(1u16),                      // Minimal shift
                Just(16u16),                     // Half-word shift
                Just(31u16),                     // Maximum shift for u32
            ]
        ) {
            prop_assert!(test_shifter_with_values(input_val, shift_val).is_ok());
        }
    }
}

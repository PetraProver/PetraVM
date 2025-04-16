use std::cell::RefMut;

use binius_core::oracle::ShiftVariant;
use binius_field::{packed::set_packed_slice, Field, PackedExtension, PackedFieldIndexable};
use binius_m3::builder::{upcast_col, Col, Expr, TableBuilder, TableWitnessSegment, B1, B128, B32};

/// A gadget for performing barrel shift operations (logical shifts and
/// rotations).
///
/// The `BarrelShifter` gadget allows for left shifts, right shifts, and
/// rotations on 32-bit inputs, with a configurable shift amount and direction.

/// Maximum number of bits of the shift amount, i.e. 0 < shift_ammount < 1 <<
/// SHIFT_MAX_BITS - 1 = 31 where dst_val = src_val >> shift_amount or dst_val =
/// src_val << shift_amount
const MAX_SHIFT_BITS: usize = 5;

/// Flags to configure the behavior of the barrel shifter.
pub struct BarrelShifterFlags {
    /// The variant of the shift operation: logical left, logical right or
    /// circular left.
    pub(crate) variant: ShiftVariant,
    /// Whether the output column should be committed or computed.
    pub(crate) commit_output: bool,
}
pub struct BarrelShifter {
    /// The input column representing the 32-bit value to be shifted.
    input: Col<B1, 32>,

    /// The shift amount column representing the 5 of positions to shift,
    /// ignoring the remaining 11.
    shift_amount: Col<B1, 16>,

    /// Binary decomposition of the shifted amount.
    shift_ammount_bits: [Col<B1>; MAX_SHIFT_BITS], // Virtual

    // TODO: Try to replace the Vec with an array.
    /// partial shift columns containing the partia_shift[i - 1]
    /// shifted by 2^i.
    shifted: Vec<Col<B1, 32>>, // Virtual

    /// Partial shift columns containing either shifted[i] or partial_shit[i-1],
    /// depending on the value of `shift_amount_bits`.
    partial_shift: [Col<B1, 32>; MAX_SHIFT_BITS], // Virtual

    /// The output column representing the result of the shift operation.
    pub output: Col<B1, 32>, // Virtual or commited, depending on the flags

    /// Flags to configure the behavior of the barrel shifter (e.g., rotation,
    /// right shift).
    flags: BarrelShifterFlags,
}

impl BarrelShifter {
    /// Creates a new instance of the `BarrelShifter` gadget.
    ///
    /// # Arguments
    ///
    /// * `table` - A mutable reference to the `TableBuilder` used to define the
    ///   gadget.
    /// * `flags` - A `BarrelShifterFlags` struct that configures the behavior
    ///   of the gadget.
    ///
    /// # Returns
    ///
    /// A new instance of the `BarrelShifter` gadget.
    pub fn new(
        table: &mut TableBuilder,
        input: Col<B1, 32>,
        shift_amount: Col<B1, 16>,
        flags: BarrelShifterFlags,
    ) -> Self {
        let partial_shift =
            core::array::from_fn(|i| table.add_committed(format!("partial_shift_{i}")));
        let shift_ammount_bits: [_; MAX_SHIFT_BITS] = core::array::from_fn(|i| {
            table.add_selected(format!("shift_ammount_bits_{i}"), shift_amount, i)
        });
        let mut shifted = Vec::with_capacity(MAX_SHIFT_BITS);
        let mut current_shift = input;
        for i in 0..MAX_SHIFT_BITS {
            shifted.push(table.add_shifted("shifted", current_shift, 5, 1 << i, flags.variant));
            let partial_shift_packed: Col<B32> = table
                .add_packed(format!("partial_shift_packed_{i}"), partial_shift[i])
                .into();
            let shifted_packed: Expr<B32, 1> = table
                .add_packed(format!("shifted_packed_{i}"), shifted[i])
                .into();
            let current_shift_packed: Col<B32> =
                table.add_packed(format!("current_shift_packed_{i}"), current_shift);
            table.assert_zero(
                format!("correct_partial_shift_{i}"),
                partial_shift_packed
                    - (shifted_packed * upcast_col(shift_ammount_bits[i])
                        + current_shift_packed * (upcast_col(shift_ammount_bits[i]) + B32::ONE)),
            );
            current_shift = partial_shift[i];
        }

        // Define the output column (32 bits).
        let output = if flags.commit_output {
            // If the output is committed, add a committed column and enforce constraints.
            let output = table.add_committed::<B1, 32>("output");
            table.assert_zero("output_constraint", current_shift - output);
            output
        } else {
            current_shift
        };

        Self {
            input,
            shift_amount,
            shift_ammount_bits,
            shifted,
            partial_shift,
            output,
            flags,
        }
    }

    /// Populates the table with witness values for the barrel shifter.
    ///
    /// # Arguments
    ///
    /// * `witness` - A mutable reference to the `TableWitness` used to populate
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
        let shift_ammount: RefMut<'_, [u16]> = index.get_mut_as(self.shift_amount).unwrap();
        // TODO: Propagate the errors
        let mut partial_shift: [_; MAX_SHIFT_BITS] =
            core::array::from_fn(|i| index.get_mut_as(self.partial_shift[i]).unwrap());
        let mut shifted: [_; MAX_SHIFT_BITS] =
            core::array::from_fn(|i| index.get_mut_as(self.shifted[i]).unwrap());
        let mut shift_ammount_bits: [_; MAX_SHIFT_BITS] =
            core::array::from_fn(|i| index.get_mut(self.shift_ammount_bits[i]).unwrap());

        for i in 0..index.size() {
            let mut current_shift = input[i];
            for j in 0..MAX_SHIFT_BITS {
                let bit = ((shift_ammount[i] >> j) & 1) == 1;
                set_packed_slice(&mut shift_ammount_bits[j], i, B1::from(bit));
                shifted[j][i] = current_shift >> (1 << j);
                if bit {
                    current_shift = shifted[j][i];
                }
                partial_shift[j][i] = current_shift;
            }
        }
        Ok(())
    }
}

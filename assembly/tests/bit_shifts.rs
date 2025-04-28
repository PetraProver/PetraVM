pub mod common;

use common::test_utils::execute_test_asm;

#[test]
fn test_bit_shifts_integration() {
    // Generate the trace for the `bit_shifts.asm` program
    let mut info = execute_test_asm(include_str!("../../examples/bit_shifts.asm"), &[]);
    let bit_shifts_frame = info.frames.add_frame("bit_shifts");

    // Verify the results of the bit shift operations
    assert_eq!(bit_shifts_frame.get_vrom_expected::<u32>(3), 5, "x = 5");
    assert_eq!(bit_shifts_frame.get_vrom_expected::<u32>(4), 10, "x <<= 1");
    assert_eq!(bit_shifts_frame.get_vrom_expected::<u32>(5), 5, "x >>= 1");
    assert_eq!(bit_shifts_frame.get_vrom_expected::<u32>(6), 0, "x >>= 3");
    assert_eq!(bit_shifts_frame.get_vrom_expected::<u32>(7), 0, "x <<= 3");

    assert_eq!(bit_shifts_frame.get_vrom_expected::<u32>(8), 2, "y = 2");
    assert_eq!(
        bit_shifts_frame.get_vrom_expected::<u32>(9),
        6,
        "shift_amt = 6"
    );
    assert_eq!(
        bit_shifts_frame.get_vrom_expected::<u32>(10),
        128,
        "y <<= shift_amt"
    );
}

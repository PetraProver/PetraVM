pub mod common;

use common::test_utils::execute_test_asm;

#[test]
fn test_mul_integration() {
    // Generate the trace for the `add.asm` program
    let mut info = execute_test_asm(include_str!("../../examples/mul.asm"), &[]);
    let mul_frame = info.frames.add_frame("mul");

    // Verify the result of the addition

    assert_eq!(mul_frame.get_vrom_expected::<u32>(6), 3, "x = 3"); // x = 3 * 7
    assert_eq!(mul_frame.get_vrom_expected::<u64>(2), 21, "3 * 7 = 21"); // Ret 1 = 3 * 7 (2 slots)

    assert_eq!(
        mul_frame.get_vrom_expected::<u32>(7),
        2147483647,
        "x = 2,147,483,647"
    ); // y = 2,147,483,647

    assert_eq!(
        mul_frame.get_vrom_expected::<u64>(4),
        21474836470,
        "x = 2,147,483,647"
    ); // Ret 2 = 2,147,483,647 * 10 (2 slots)
}

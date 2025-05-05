pub mod common;

use common::test_utils::execute_test_asm;

#[test]
fn test_add_integration() {
    // Generate the trace for the `add.asm` program
    let mut info = execute_test_asm(include_str!("../../examples/add.asm"));
    let add_frame = info.frames.add_frame("add");

    // Verify the result of the addition
    assert_eq!(add_frame.get_vrom_expected::<u32>(3), 2, "x = 2");
    assert_eq!(add_frame.get_vrom_expected::<u32>(2), 8, "2 + 6 = 8");
}

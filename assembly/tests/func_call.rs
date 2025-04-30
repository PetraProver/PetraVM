pub mod common;

use common::test_utils::execute_test_asm;

#[test]
fn test_func_call_integration() {
    tracing_subscriber::fmt::init();

    // Generate the trace for the `func_call.asm` program
    let mut info = execute_test_asm(include_str!("../../examples/func_call.asm"));

    let func_call_frame = info.frames.add_frame("func_call");
    let add_two_numbers_frame = info.frames.add_frame("add_two_numbers");

    // Verify the results of the function call
    assert_eq!(
        func_call_frame.get_vrom_expected::<u32>(4),
        12,
        "add_two_numbers(4, 8) = 12"
    );
    assert_eq!(
        func_call_frame.get_vrom_expected::<u32>(2),
        22,
        "return x + 10"
    );

    assert_eq!(
        add_two_numbers_frame.get_vrom_expected::<u32>(2),
        4,
        "a = 4"
    );
    assert_eq!(
        add_two_numbers_frame.get_vrom_expected::<u32>(3),
        8,
        "b = 8"
    );
    assert_eq!(
        add_two_numbers_frame.get_vrom_expected::<u32>(4),
        12,
        "a + b = 12"
    );
}

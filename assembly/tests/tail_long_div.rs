pub mod common;
use common::test_utils::{execute_test_asm, AsmToExecute};

fn run_test(dividend: u32, divisor: u32, expected_quotient: u32, expected_remainder: u32) {
    let mut info = execute_test_asm(
        AsmToExecute::new(include_str!("../../examples/tail_long_div.asm"))
            .init_vals(vec![dividend, divisor]),
    );
    let tail_long_div_frame = info.frames.add_frame("div");

    // Verify the quotient
    assert_eq!(
        tail_long_div_frame.get_vrom_expected::<u32>(4),
        expected_quotient,
        "Quotient mismatch for dividend = {dividend}, divisor = {divisor}"
    );

    // Verify the remainder
    assert_eq!(
        tail_long_div_frame.get_vrom_expected::<u32>(5),
        expected_remainder,
        "Remainder mismatch for dividend = {dividend}, divisor = {divisor}"
    );
}

#[test]
fn test_tail_long_div_integration() {
    // Test cases
    run_test(10, 3, 3, 1); // 10 / 3 = 3 remainder 1
    run_test(20, 4, 5, 0); // 20 / 4 = 5 remainder 0
    run_test(15, 6, 2, 3); // 15 / 6 = 2 remainder 3
    run_test(7, 7, 1, 0); // 7 / 7 = 1 remainder 0
    run_test(9, 2, 4, 1); // 9 / 2 = 4 remainder 1
}

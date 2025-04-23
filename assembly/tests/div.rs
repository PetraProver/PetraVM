use common::test_utils::execute_test_asm;

pub mod common;

fn run_test(a: u32, b: u32, expected_q: i32, expected_r: u32) {
    let mut info = execute_test_asm(include_str!("../../examples/div.asm"), &[a, b]);
    let div_frame = info.frames.add_frame("div");

    // TODO: Replace `u32` with `i32` once `VromValueT` is implemented for `i32`...
    assert_eq!(
        div_frame.get_vrom_expected::<u32>(4) as i32,
        expected_q,
        "Quotient for div({}, {})",
        a,
        b
    );
    assert_eq!(
        div_frame.get_vrom_expected::<u32>(5),
        expected_r,
        "Remainder for div({}, {})",
        a,
        b
    );
}

// TODO: This test won't work until we can support signed integers, so for now
// it's going to just be ignored.
#[test]
#[ignore]
fn test_div_integration() {
    // Test case 1: a < b
    run_test(5, 10, 0, 5);

    // Test case 2: a == b
    run_test(10, 10, 1, 0);

    // Test case 3: a > b
    run_test(20, 6, 3, 2);

    // Test case 4: a = 0
    run_test(0, 7, 0, 0);

    // Test case 5: b = 1
    run_test(15, 1, 15, 0);
}

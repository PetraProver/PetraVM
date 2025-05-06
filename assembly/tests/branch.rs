pub mod common;

use common::test_utils::{execute_test_asm, AsmToExecute};

fn run_test(input: u32, expected: u32, condition: &str) {
    let mut info = execute_test_asm(
        AsmToExecute::new(include_str!("../../examples/branch.asm")).init_vals(vec![input]),
    );
    let branch_frame = info.frames.add_frame("branches");

    assert_eq!(
        branch_frame.get_vrom_expected::<u32>(3),
        expected,
        "Condition: {}",
        condition
    );
}

// Test case 1: Input that causes the branch to evaluate to the "less_than_3"
// path
#[test]
fn test_branch_integration_less_than() {
    run_test(2, 4, "n < 3");
}

// Test case 2: Input that causes the branch to evaluate to the "else" path
#[test]
fn test_branch_integration_greater_or_equal() {
    run_test(3, 2, "n >= 3");
}

pub mod common;

use common::test_utils::{execute_test_asm, AsmToExecute};

#[test]
fn test_collatz_integration() {
    let init_vals = [5, 27, 3999];

    // Execute the program multiple times with different initial values
    for initial_value in init_vals {
        // Load in Collatz binary.
        let mut info = execute_test_asm(
            AsmToExecute::new(include_str!("../../examples/collatz.asm"))
                .init_vals(vec![initial_value]),
        );
        let collatz_frame = info.frames.add_frame("collatz");

        // Verify the final result is 1, as expected for the Collatz conjecture
        assert_eq!(
            collatz_frame.get_vrom_expected::<u32>(4),
            1,
            "Final result should be 1 for initial value {initial_value}"
        );
    }
}

// NOTE: Initial version of this test generated with a model and may not work.

pub mod common;

use common::test_utils::{execute_test_asm, AsmToExecute};

// TODO: Once we can support non-unsized arithmetic update and enable this
// test...
#[test]
fn test_bezout_integration() {
    // Define test cases with (a, b) pairs and their expected gcd
    let test_cases = [
        (56, 15, 1),   // gcd(56, 15) = 1
        (48, 18, 6),   // gcd(48, 18) = 6
        (101, 103, 1), // gcd(101, 103) = 1
        (270, 192, 6), // gcd(270, 192) = 6
    ];

    for &(a, b, expected_gcd) in &test_cases {
        // Execute the `bezout.asm` program with the given arguments
        let mut info = execute_test_asm(
            AsmToExecute::new(include_str!("../../examples/bezout.asm"))
                .add_binary(include_str!("../../examples/div.asm"))
                .init_vals(vec![a, b]),
        );
        let bezout_frame = info.frames.add_frame("bezout");

        // TODO: Replace `u32` with `i32` once `VromValueT` is implemented for `i32`...
        // Verify the gcd result
        assert_eq!(
            bezout_frame.get_vrom_expected::<u32>(4),
            expected_gcd,
            "GCD of {a} and {b} should be {expected_gcd}"
        );

        // TODO: Replace `u32` with `i32` once `VromValueT` is implemented for `i32`...
        // Verify Bezout coefficients satisfy the equation: a*x + b*y = gcd(a, b)
        let x = bezout_frame.get_vrom_expected::<u32>(5) as i32;
        let y = bezout_frame.get_vrom_expected::<u32>(6) as i32;

        assert_eq!(
            a as i32 * x + b as i32 * y,
            expected_gcd as i32,
            "Bezout coefficients do not satisfy the equation for a = {a}, b = {b}"
        );
    }
}

#[test]
fn test_bezout_deterministic_integration() {
    execute_test_asm(
        AsmToExecute::new(include_str!("../../examples/bezout_deterministic.asm"))
            .add_binary(include_str!("../../examples/div.asm")),
    );
}

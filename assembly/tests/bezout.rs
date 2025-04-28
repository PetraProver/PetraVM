// NOTE: Initial version of this test generated with a model and may not work.

pub mod common;

use common::test_utils::execute_test_asm;

// TODO: Once we can support non-unsized arithmetic update and enable this
// test...
#[test]
#[ignore]
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
        let mut info = execute_test_asm(include_str!("../../examples/bezout.asm"), &[a, b]);
        let bezout_frame = info.frames.add_frame("bezout");

        // TODO: Replace `u32` with `i32` once `VromValueT` is implemented for `i32`...
        // Verify the gcd result
        assert_eq!(
            bezout_frame.get_vrom_expected::<u32>(3),
            expected_gcd,
            "GCD of {} and {} should be {}",
            a,
            b,
            expected_gcd
        );

        // TODO: Replace `u32` with `i32` once `VromValueT` is implemented for `i32`...
        // Verify Bezout coefficients satisfy the equation: a*x + b*y = gcd(a, b)
        let x = bezout_frame.get_vrom_expected::<u32>(4);
        let y = bezout_frame.get_vrom_expected::<u32>(5);
        assert_eq!(
            a * x + b * y,
            expected_gcd,
            "Bezout coefficients do not satisfy the equation for a = {}, b = {}",
            a,
            b
        );
    }
}

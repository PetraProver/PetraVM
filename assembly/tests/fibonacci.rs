pub mod common;

use common::test_utils::execute_test_asm;
use num_traits::WrappingAdd;

#[test]
fn test_fibonacci_integration() {
    let mut cur_fibs = [0, 1];

    // Set initial value
    let init_val = 4;

    let mut info = execute_test_asm(include_str!("../../examples/fib.asm"), &[init_val]);

    // Push a frame for `fib_frame_temp`.
    let fib_frame = info.frames.add_frame("fib");

    // Check all intermediary values
    for i in 0..init_val {
        let s = cur_fibs[0].wrapping_add(&cur_fibs[1]);

        // Push a frame for each recursive call to `fib_helper`.
        let fib_helper_frame = info.frames.add_frame("fib_helper");

        // Check current a value
        assert_eq!(
            fib_helper_frame.get_vrom_expected::<u32>(2),
            cur_fibs[0],
            "Incorrect 'a' value at iteration {}",
            i
        );

        // Check current b value
        assert_eq!(
            fib_helper_frame.get_vrom_expected::<u32>(3),
            cur_fibs[1],
            "Incorrect 'b' value at iteration {}",
            i
        );

        // Check a + b value
        assert_eq!(
            fib_helper_frame.get_vrom_expected::<u32>(7),
            s,
            "Incorrect 'a + b' value at iteration {}",
            i
        );

        // Update fibonacci values for next iteration
        cur_fibs[0] = cur_fibs[1];
        cur_fibs[1] = s;
    }

    let final_fib_helper_frame = &info.frames["fib_helper"][init_val as usize - 1];
    let final_fib_ret_val = final_fib_helper_frame.get_vrom_expected::<u32>(5);

    // Check the final return value
    assert_eq!(final_fib_ret_val, cur_fibs[0]);

    // Check that the returned value is propagated correctly to the initial frame
    assert_eq!(final_fib_ret_val, fib_frame.get_vrom_expected::<u32>(3));
}

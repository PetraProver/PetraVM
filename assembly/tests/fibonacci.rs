use binius_field::{BinaryField, BinaryField32b, Field};
use num_traits::WrappingAdd;
use zcrayvm_assembly::{get_full_prom_and_labels, parse_program, Memory, ValueRom, ZCrayTrace};

#[test]
fn test_fibonacci_integration() {
    // Use the multiplicative generator G for calculations
    const G: BinaryField32b = BinaryField32b::MULTIPLICATIVE_GENERATOR;

    // Parse the Fibonacci program
    let instructions = parse_program(include_str!("../../examples/fib.asm")).unwrap();

    // Generate the program ROM and associated data
    let (prom, _, pc_field_to_int, frame_sizes) =
        get_full_prom_and_labels(&instructions).expect("Failed to process instructions");

    // Set initial value
    let init_val = 4;
    let initial_value = G.pow([init_val as u64]).val();

    // Initialize memory with return PC = 0, return FP = 0, and the argument
    let vrom = ValueRom::new_with_init_vals(&[0, 0, initial_value]);
    let memory = Memory::new(prom, vrom);

    // Execute the program and generate the trace
    let (trace, boundary_values) = ZCrayTrace::generate(memory, frame_sizes, pc_field_to_int)
        .expect("Trace generation should not fail");

    // Validate the trace
    trace.validate(boundary_values);

    // Verify Fibonacci computation
    let fib_power_two_frame_size = 16;
    let mut cur_fibs = [0, 1];

    // Check all intermediary values
    for i in 0..init_val {
        let s = cur_fibs[0].wrapping_add(&cur_fibs[1]);

        // Check current a value
        assert_eq!(
            trace
                .get_vrom_u32((i + 1) * fib_power_two_frame_size + 2)
                .unwrap(),
            cur_fibs[0],
            "Incorrect 'a' value at iteration {}",
            i
        );

        // Check current b value
        assert_eq!(
            trace
                .get_vrom_u32((i + 1) * fib_power_two_frame_size + 3)
                .unwrap(),
            cur_fibs[1],
            "Incorrect 'b' value at iteration {}",
            i
        );

        // Check a + b value
        assert_eq!(
            trace
                .get_vrom_u32((i + 1) * fib_power_two_frame_size + 7)
                .unwrap(),
            s,
            "Incorrect 'a + b' value at iteration {}",
            i
        );

        // Update fibonacci values for next iteration
        cur_fibs[0] = cur_fibs[1];
        cur_fibs[1] = s;
    }

    // Check the final return value
    assert_eq!(
        trace
            .get_vrom_u32((init_val + 1) * fib_power_two_frame_size + 5)
            .unwrap(),
        cur_fibs[0],
        "Final return value is incorrect"
    );

    // Check that the returned value is propagated correctly to the initial frame
    assert_eq!(
        trace
            .get_vrom_u32((init_val + 1) * fib_power_two_frame_size + 5)
            .unwrap(),
        trace.get_vrom_u32(3).unwrap(),
        "Return value not properly propagated"
    );
}

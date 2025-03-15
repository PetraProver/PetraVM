use std::collections::HashMap;

use binius_field::{BinaryField32b, Field};
use zcrayvm_assembly::{get_full_prom_and_labels, parse_program, Memory, ValueRom, ZCrayTrace};

#[test]
fn test_collatz_validation() {
    // Parse the Collatz program
    let instructions = parse_program(include_str!("../../examples/collatz.asm")).unwrap();

    // Set up the call procedure hints for the program
    let mut is_call_procedure_hints = vec![false; instructions.len()];
    let indices = vec![9, 10, 11, 15, 16, 17];
    for idx in indices {
        is_call_procedure_hints[idx] = true;
    }

    // Generate the program ROM and associated data
    let (prom, _, pc_field_to_int) =
        get_full_prom_and_labels(&instructions, &is_call_procedure_hints)
            .expect("Failed to process instructions");

    // Test with multiple initial values
    for &initial_value in &[5, 27, 3999] {
        // Set up frame sizes for the program
        let mut frame_sizes = HashMap::new();
        frame_sizes.insert(BinaryField32b::ONE, 9);

        // Initialize the VROM with the initial value
        let vrom = ValueRom::new_with_init_vals(&[0, 0, initial_value]);
        let memory = Memory::new(prom.clone(), vrom);

        // Execute the program and generate the trace
        let (trace, boundary_values) =
            ZCrayTrace::generate(memory, frame_sizes, pc_field_to_int.clone())
                .expect("Trace generation should not fail");

        // Validate the trace - this is the key functionality we're testing
        trace.validate(boundary_values);

        // Verify the final result is 1, as expected for the Collatz conjecture
        // assert_eq!(
        //     trace.get_vrom_u32(3).unwrap(),
        //     1,
        //     "Final result should be 1 for initial value {}",
        //     initial_value
        // );
    }
}

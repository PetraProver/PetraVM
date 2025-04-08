use zcrayvm_assembly::{Assembler, Memory, ValueRom, ZCrayTrace};

#[test]
fn test_collatz_integration() {
    // Parse the Collatz program
    let compiled_program =
        Assembler::from_code(include_str!("../../examples/collatz.asm")).unwrap();

    // Test with multiple initial values
    for &initial_value in &[5, 27, 3999] {
        // Initialize the VROM with the initial value
        let vrom = ValueRom::new_with_init_vals(&[0, 0, initial_value]);
        let memory = Memory::new(compiled_program.prom.clone(), vrom);

        // Execute the program and generate the trace
        let (trace, boundary_values) = ZCrayTrace::generate(
            memory,
            compiled_program.frame_sizes.clone(),
            compiled_program.pc_field_to_int.clone(),
        )
        .expect("Trace generation should not fail");

        // Validate the trace - this is the key functionality we're testing
        trace.validate(boundary_values);

        // Verify the final result is 1, as expected for the Collatz conjecture
        assert_eq!(
            trace.vrom().read::<u32>(3).unwrap(),
            1,
            "Final result should be 1 for initial value {}",
            initial_value
        );
    }
}

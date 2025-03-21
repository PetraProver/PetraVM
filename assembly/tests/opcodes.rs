use zcrayvm_assembly::{Assembler, Memory, ValueRom, ZCrayTrace};

#[test]
fn test_opcodes() {
    let compiled_program =
        Assembler::from_code(include_str!("../../examples/opcodes.asm")).unwrap();

    // Generate the program ROM and associated data
    let vrom = ValueRom::new_with_init_vals(&[0, 0]);
    let memory = Memory::new(compiled_program.prom, vrom);

    // Execute the program and generate the trace
    let (trace, boundary_values) = ZCrayTrace::generate(
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_int,
    )
    .expect("Trace generation should not fail");

    // Validate the trace - this is the key functionality we're testing
    trace.validate(boundary_values);
}

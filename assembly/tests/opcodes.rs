use std::{collections::HashSet, mem};

use zcrayvm_assembly::{Assembler, Memory, Opcode, ValueRom, ZCrayTrace};

#[test]
fn test_opcodes() {
    let compiled_program =
        Assembler::from_code(include_str!("../../examples/opcodes.asm")).unwrap();

    // Ensure all opcodes are present in the program
    let mut seen = HashSet::new();
    for instr in &compiled_program.prom {
        seen.insert(mem::discriminant(&instr.opcode()));
    }
    assert_eq!(seen.len(), Opcode::OP_COUNT);

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

    // Verify the final result is 0
    assert_eq!(
        trace.get_vrom_u32(2).unwrap(),
        0,
        "Final result should be 0"
    );
}

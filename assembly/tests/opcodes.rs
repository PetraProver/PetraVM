use std::{collections::HashSet, mem};

use zcrayvm_assembly::{get_full_prom_and_labels, parse_program, Memory, ValueRom, ZCrayTrace};

#[test]
fn test_opcodes() {
    let instructions = parse_program(include_str!("../../examples/opcodes.asm")).unwrap();

    // Ensure all opcodes are present in the program
    const TOTAL_OPS: usize = 39;
    let mut seen = HashSet::new();
    for instr in &instructions {
        seen.insert(mem::discriminant(instr));
    }
    assert_eq!(seen.len(), TOTAL_OPS);

    // Generate the program ROM and associated data
    let (prom, _, pc_field_to_int, frame_sizes) =
        get_full_prom_and_labels(&instructions).expect("Failed to process instructions");

    let vrom = ValueRom::new_with_init_vals(&[0, 0]);
    let memory = Memory::new(prom, vrom);

    // Execute the program and generate the trace
    let (trace, boundary_values) = ZCrayTrace::generate(memory, frame_sizes, pc_field_to_int)
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

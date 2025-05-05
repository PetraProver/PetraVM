pub mod common;

use std::collections::HashSet;

use common::test_utils::execute_test_asm;
use strum::VariantArray;
use zcrayvm_assembly::Opcode;

#[test]
fn test_opcodes() {
    let mut info = execute_test_asm(include_str!("../../examples/opcodes.asm"));

    // Ensure all opcodes are present in the program
    let mut unseen_types_remaining: HashSet<_> = HashSet::from_iter(Opcode::VARIANTS);
    unseen_types_remaining.remove(&Opcode::Bz); // Bz isn't an actual opcode
    unseen_types_remaining.remove(&Opcode::Invalid); // Invalid is not an opcode.

    for instr in &info.compiled_program.prom {
        unseen_types_remaining.remove(&instr.opcode());
    }

    assert!(
        unseen_types_remaining.is_empty(),
        "Some existing opcodes were not present in the opcode test program: {:#?}",
        unseen_types_remaining
    );

    // Verify the final result is 0
    assert_eq!(
        info.frames.add_frame("_start").get_vrom_expected::<u32>(2),
        0,
        "Final result should be 0"
    );
}

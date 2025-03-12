// TODO: Remove these once stable enough
#![allow(unused)]
#![allow(dead_code)]

// TODO: Add doc

mod emulator;
mod event;
mod instruction_args;
mod instructions_with_labels;
mod opcodes;
mod parser;
mod vrom_allocator;

use std::collections::HashMap;

use binius_field::{BinaryField16b, BinaryField32b, ExtensionField, Field, PackedField};
use emulator::{Instruction, InterpreterInstruction, ProgramRom, ValueRom, ZCrayTrace, G};
use instructions_with_labels::{get_frame_sizes_all_labels, get_full_prom_and_labels};
use opcodes::Opcode;
use parser::parse_program;

#[inline(always)]
pub(crate) const fn get_binary_slot(i: u16) -> BinaryField16b {
    BinaryField16b::new(i)
}

pub(crate) fn code_to_prom(
    code: &[Instruction],
    is_calling_procedure_hints: &[bool],
) -> ProgramRom {
    let mut prom = ProgramRom::new();
    let mut pc = BinaryField32b::ONE; // we start at PC = 1G.
    for (i, &instruction) in code.iter().enumerate() {
        let interp_inst =
            InterpreterInstruction::new(instruction, pc, is_calling_procedure_hints[i]);
        prom.push(interp_inst);
        pc *= G;
    }

    prom
}

fn main() {
    let collatz = BinaryField16b::ONE;
    let case_recurse =
        ExtensionField::<BinaryField16b>::iter_bases(&G.pow(4)).collect::<Vec<BinaryField16b>>();
    let case_odd =
        ExtensionField::<BinaryField16b>::iter_bases(&G.pow(10)).collect::<Vec<BinaryField16b>>();

    let instructions = parse_program(include_str!("../../examples/collatz.asm")).unwrap();

    // Sets the call procedure hints to true for the returned PROM (where
    // instructions are given with the labels).
    let mut is_call_procedure_hints_with_labels = vec![false; instructions.len()];
    let indices_to_set_with_labels = vec![9, 10, 11, 15, 16, 17];
    for idx in indices_to_set_with_labels {
        is_call_procedure_hints_with_labels[idx] = true;
    }
    let (prom, labels, pc_field_to_int) =
        get_full_prom_and_labels(&instructions, &is_call_procedure_hints_with_labels)
            .expect("Instructions were not formatted properly.");

    let zero = BinaryField16b::zero();

    let frame_sizes = get_frame_sizes_all_labels(&prom, labels, &pc_field_to_int);
    println!("frame sizes {:?}", frame_sizes);

    let expected_prom = vec![
        // collatz:
        [
            Opcode::Xori.get_field_elt(),
            get_binary_slot(20),
            get_binary_slot(8),
            get_binary_slot(1),
        ], //  0G: XORI 20 8 1
        [
            Opcode::Bnz.get_field_elt(),
            get_binary_slot(20),
            case_recurse[0],
            case_recurse[1],
        ], //  1G: BNZ 20 case_recurse
        // case_return:
        [
            Opcode::Xori.get_field_elt(),
            get_binary_slot(12),
            get_binary_slot(8),
            zero,
        ], //  2G: XORI 12 8 zero
        [Opcode::Ret.get_field_elt(), zero, zero, zero], //  3G: RET
        // case_recurse:
        [
            Opcode::Andi.get_field_elt(),
            get_binary_slot(24),
            get_binary_slot(8),
            get_binary_slot(1),
        ], // 4G: ANDI 24 8 1
        [
            Opcode::Bnz.get_field_elt(),
            get_binary_slot(24),
            case_odd[0],
            case_odd[1],
        ], //  5G: BNZ 24 case_odd
        // case_even:
        [
            Opcode::Srli.get_field_elt(),
            get_binary_slot(32),
            get_binary_slot(8),
            get_binary_slot(1),
        ], //  6G: SRLI 32 8 1
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(16),
            get_binary_slot(8),
            get_binary_slot(32),
        ], //  7G: MVV.W @16[8], @32
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(16),
            get_binary_slot(12),
            get_binary_slot(12),
        ], //  8G: MVV.W @16[12], @12
        [
            Opcode::Taili.get_field_elt(),
            collatz,
            zero,
            get_binary_slot(16),
        ], // 9G: TAILI collatz 16
        // case_odd:
        [
            Opcode::Muli.get_field_elt(),
            get_binary_slot(28),
            get_binary_slot(8),
            get_binary_slot(3),
        ], //  10G: MULI 28 8 3
        [
            Opcode::Addi.get_field_elt(),
            get_binary_slot(32),
            get_binary_slot(28),
            get_binary_slot(1),
        ], //  11G: ADDI 32 28 1
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(16),
            get_binary_slot(8),
            get_binary_slot(32),
        ], //  12G: MVV.W @16[8], @32
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(16),
            get_binary_slot(12),
            get_binary_slot(12),
        ], //  13G: MVV.W @16[12], @12
        [
            Opcode::Taili.get_field_elt(),
            collatz,
            zero,
            get_binary_slot(16),
        ], //  14G: TAILI collatz 16
    ];

    // Sets the call procedure hints to true for the expected PROM (where
    // instructions are given without the labels).
    let mut is_call_procedure_hints = vec![false; instructions.len()];
    let indices_to_set = vec![7, 8, 9, 12, 13, 14];
    for idx in indices_to_set {
        is_call_procedure_hints[idx] = true;
    }
    let expected_prom = code_to_prom(&expected_prom, &is_call_procedure_hints);

    assert!(
        prom.len() == expected_prom.len(),
        "Not identical number of instructions in PROM ({:?}) and expected PROM ({:?})",
        prom.len(),
        expected_prom.len()
    );

    for (i, inst) in prom.iter().enumerate() {
        let expected_inst = &expected_prom[i];
        assert_eq!(
            *inst, *expected_inst,
            "Value for index {:?} in PROM is {:?} but is {:?} in expected PROM",
            i, inst, expected_inst
        );
    }

    let initial_value = 3999;
    let mut vrom = ValueRom::new_from_vec_u32(vec![0, 0, initial_value]);

    let _ = ZCrayTrace::generate_with_vrom(prom, vrom, frame_sizes, pc_field_to_int)
        .expect("Trace generation should not fail.");
}

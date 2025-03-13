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
mod vrom;
mod vrom_allocator;

use std::collections::HashMap;

use binius_field::{BinaryField16b, BinaryField32b, ExtensionField, Field, PackedField};
use emulator::{Instruction, InterpreterInstruction, ProgramRom, ZCrayTrace, G};
use instructions_with_labels::get_full_prom_and_labels;
use opcodes::Opcode;
use parser::parse_program;
use vrom::ValueRom;

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

    let expected_prom = vec![
        // collatz:
        [
            Opcode::Xori.get_field_elt(),
            get_binary_slot(5),
            get_binary_slot(2),
            get_binary_slot(1),
        ], //  0G: XORI 5 2 1
        [
            Opcode::Bnz.get_field_elt(),
            get_binary_slot(5),
            case_recurse[0],
            case_recurse[1],
        ], //  1G: BNZ 5 case_recurse
        // case_return:
        [
            Opcode::Xori.get_field_elt(),
            get_binary_slot(3),
            get_binary_slot(2),
            zero,
        ], //  2G: XORI 3 2 zero
        [Opcode::Ret.get_field_elt(), zero, zero, zero], //  3G: RET
        // case_recurse:
        [
            Opcode::Andi.get_field_elt(),
            get_binary_slot(6),
            get_binary_slot(2),
            get_binary_slot(1),
        ], // 4G: ANDI 6 2 1
        [
            Opcode::Bnz.get_field_elt(),
            get_binary_slot(6),
            case_odd[0],
            case_odd[1],
        ], //  5G: BNZ 6 case_odd
        // case_even:
        [
            Opcode::Srli.get_field_elt(),
            get_binary_slot(8),
            get_binary_slot(2),
            get_binary_slot(1),
        ], //  6G: SRLI 8 2 1
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(4),
            get_binary_slot(2),
            get_binary_slot(8),
        ], //  7G: MVV.W @4[2], @8
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(4),
            get_binary_slot(3),
            get_binary_slot(3),
        ], //  8G: MVV.W @4[3], @3
        [
            Opcode::Taili.get_field_elt(),
            collatz,
            zero,
            get_binary_slot(4),
        ], // 9G: TAILI collatz 4
        // case_odd:
        [
            Opcode::Muli.get_field_elt(),
            get_binary_slot(7),
            get_binary_slot(2),
            get_binary_slot(3),
        ], //  10G: MULI 7 2 3
        [
            Opcode::Addi.get_field_elt(),
            get_binary_slot(8),
            get_binary_slot(7),
            get_binary_slot(1),
        ], //  11G: ADDI 8 7 1
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(4),
            get_binary_slot(2),
            get_binary_slot(8),
        ], //  12G: MVV.W @4[2], @8
        [
            Opcode::MVVW.get_field_elt(),
            get_binary_slot(4),
            get_binary_slot(3),
            get_binary_slot(3),
        ], //  13G: MVV.W @4[3], @3
        [
            Opcode::Taili.get_field_elt(),
            collatz,
            zero,
            get_binary_slot(4),
        ], //  14G: TAILI collatz 4
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

    let mut frame_sizes = HashMap::new();
    frame_sizes.insert(BinaryField32b::ONE, 9);
    let initial_value = 3999;
    let mut vrom = ValueRom::new_with_init_values(vec![0, 0, initial_value]);

    let _ = ZCrayTrace::generate_with_vrom(prom, vrom, frame_sizes, pc_field_to_int)
        .expect("Trace generation should not fail.");
}

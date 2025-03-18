// TODO: Remove these once stable enough
#![allow(unused)]
#![allow(dead_code)]

// TODO: Add doc

mod event;
mod execution;
mod memory;
mod opcodes;
mod parser;
mod util;

use std::collections::HashMap;

use binius_field::{BinaryField16b, BinaryField32b, ExtensionField, Field, PackedField};
use execution::ZCrayTrace;
use execution::{Instruction, InterpreterInstruction, G};
use memory::{Memory, ProgramRom, ValueRom};
use opcodes::Opcode;
use parser::get_full_prom_and_labels;
use parser::parse_program;
use util::get_binary_slot;

pub(crate) fn code_to_prom(code: &[Instruction]) -> ProgramRom {
    let mut prom = ProgramRom::new();
    // TODO: type-gate field_pc and use some `incr()` method to abstract away `+1` /
    // `*G`.
    let mut pc = BinaryField32b::ONE; // we start at PC = 1G.
    for (i, &instruction) in code.iter().enumerate() {
        let interp_inst = InterpreterInstruction::new(instruction, pc);
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

    let (prom, labels, pc_field_to_int, frame_sizes) =
        get_full_prom_and_labels(&instructions).expect("Instructions were not formatted properly.");

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

    let expected_prom = code_to_prom(&expected_prom);

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
    let vrom = ValueRom::new_with_init_vals(&[0, 0, initial_value]);

    let memory = Memory::new(prom, vrom);

    let _ = ZCrayTrace::generate(memory, frame_sizes, pc_field_to_int)
        .expect("Trace generation should not fail.");
}

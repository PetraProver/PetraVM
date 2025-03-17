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

pub(crate) fn code_to_prom(
    code: &[Instruction],
    is_calling_procedure_hints: &[bool],
) -> ProgramRom {
    let mut prom = ProgramRom::new();
    // TODO: type-gate field_pc and use some `incr()` method to abstract away `+1` /
    // `*G`.
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
    let kernel_files = [
        include_str!("../../examples/bezout.asm"),
        include_str!("../../examples/div.asm"),
    ];
    let instructions = kernel_files
        .into_iter()
        .flat_map(|file| parse_program(file).unwrap())
        .collect::<Vec<_>>();

    // Sets the call procedure hints to true for the returned PROM (where
    // instructions are given with the labels).
    let mut is_call_procedure_hints_with_labels = vec![false; instructions.len()];
    let indices_to_set_with_labels = vec![
        7, 8, 9, 10, 12, 13, 14, 15, 16, // Bezout
        25, 26, 27, 28, // Div
    ];
    for idx in indices_to_set_with_labels {
        is_call_procedure_hints_with_labels[idx] = true;
    }
    let (prom, labels, pc_field_to_int, frame_sizes) =
        get_full_prom_and_labels(&instructions, &is_call_procedure_hints_with_labels)
            .expect("Instructions were not formatted properly.");

    let a = 12;
    let b = 3;
    let mut vrom = ValueRom::new_with_init_vals(&[0, 0, a, b]);

    let mut pc = BinaryField32b::ONE;
    let mut pc_field_to_int = HashMap::new();
    for i in 0..prom.len() {
        pc_field_to_int.insert(pc, i as u32 + 1);
        pc *= G;
    }

    let memory = Memory::new(prom, vrom);
    let (trace, _) = ZCrayTrace::generate(memory, frame_sizes, pc_field_to_int)
        .expect("Trace generation should not fail.");

    // gcd
    assert_eq!(
        trace
            .get_vrom_u32(4)
            .expect("Return value for quotient not set."),
        3
    );
    // a's coefficient
    assert_eq!(
        trace
            .get_vrom_u32(5)
            .expect("Return value for remainder not set."),
        0
    );
    // b's coefficient
    assert_eq!(
        trace
            .get_vrom_u32(6)
            .expect("Return value for remainder not set."),
        1
    );
}
